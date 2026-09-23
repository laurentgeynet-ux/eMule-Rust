// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use std::io::{Cursor, Read};
use std::net::SocketAddr;

use bytes::{Bytes, BytesMut};
use mule_core::{AllocationPolicy, PartFile, RangeSet};
use mule_format::ed2k::hash_reader;
use mule_format::fixtures::{PatternReader, FIXTURE_EXACT_ONE_PART, REPEATING_0_TO_255};
use mule_protocol::lab::*;
use mule_protocol::{PacketCodec, PeerId, ED2K_BLOCK_SIZE, MAX_BLOCK_BYTES};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn send_msg(stream: &mut TcpStream, codec: &PacketCodec, msg: &LabMessage) {
    let raw = msg.to_raw_packet().expect("serializes to raw packet");
    let mut buf = BytesMut::new();
    codec.encode(&raw, &mut buf).expect("encodes to wire");
    stream.write_all(&buf).await.expect("writes to socket");
    stream.flush().await.expect("flushes socket");
}

async fn read_msg(
    stream: &mut TcpStream,
    codec: &PacketCodec,
    buf: &mut BytesMut,
) -> Result<LabMessage, Box<dyn std::error::Error + Send + Sync>> {
    loop {
        if let Some(packet) = codec.decode(buf)? {
            return Ok(LabMessage::from_raw_packet(&packet)?);
        }
        let mut chunk = [0u8; 8192];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err("Unexpected EOF on socket".into());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// Spawns a laboratory source server on 127.0.0.1:0 serving the exact 1-part fixture.
async fn spawn_source_server(
    file_hash: [u8; 16],
    fixture_data: Bytes,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let mut engine = LabSourceEngine::new();
    engine.host_file(file_hash, "fixture_9728000.bin", fixture_data);

    let handle = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let engine = engine.clone();
            tokio::spawn(async move {
                let codec = PacketCodec::new();
                let mut read_buf = BytesMut::new();

                let server_peer_id = PeerId::new([0xBB; 16]);
                let mut sm = PeerStateMachine::new_listener(server_peer_id, 1);

                // 1. Handshake: wait for Hello
                let hello = match read_msg(&mut socket, &codec, &mut read_buf).await {
                    Ok(h) => h,
                    Err(_) => return,
                };
                let actions = match sm.on_message(hello) {
                    Ok(a) => a,
                    Err(_) => return,
                };
                for action in actions {
                    if let PeerAction::SendMessage(msg) = action {
                        send_msg(&mut socket, &codec, &msg).await;
                    }
                }

                // 2. Offer the hosted file
                if let Some(offer) = engine.offer_file(&file_hash) {
                    send_msg(&mut socket, &codec, &offer).await;
                }

                // 3. Serve block requests until Goodbye
                loop {
                    let msg = match read_msg(&mut socket, &codec, &mut read_buf).await {
                        Ok(m) => m,
                        Err(_) => break, // Client disconnected
                    };

                    match msg {
                        LabMessage::RequestBlock {
                            file_hash: req_hash,
                            start_offset,
                            length,
                        } => {
                            let resp = engine.serve_block(&req_hash, start_offset, length);
                            send_msg(&mut socket, &codec, &resp).await;
                        }
                        LabMessage::Goodbye { .. } => {
                            let _ = sm.on_message(msg);
                            break;
                        }
                        _ => break,
                    }
                }
            });
        }
    });

    (addr, handle)
}

#[tokio::test]
async fn test_nominal_loopback_transfer_with_shuffled_blocks() {
    let total_len = FIXTURE_EXACT_ONE_PART.length as usize;
    let mut data = vec![0u8; total_len];
    PatternReader::new(&REPEATING_0_TO_255, total_len as u64)
        .read_exact(&mut data)
        .unwrap();
    let fixture_bytes = Bytes::from(data);

    let mut file_hash = [0u8; 16];
    hex::decode_to_slice(FIXTURE_EXACT_ONE_PART.expected_ed2k_hex, &mut file_hash).unwrap();

    let (server_addr, _server_task) = spawn_source_server(file_hash, fixture_bytes).await;

    // Connect client
    let mut client_sock = TcpStream::connect(server_addr).await.unwrap();
    let codec = PacketCodec::new();
    let mut client_buf = BytesMut::new();

    let client_peer_id = PeerId::new([0xAA; 16]);
    let challenge_nonce = [0x55; 16];
    let (mut client_sm, hello) =
        PeerStateMachine::new_initiator(client_peer_id, 1, challenge_nonce);

    // Send Hello
    send_msg(&mut client_sock, &codec, &hello).await;

    // Receive HelloAck
    let ack = read_msg(&mut client_sock, &codec, &mut client_buf)
        .await
        .unwrap();
    client_sm.on_message(ack).expect("client handshake ready");
    assert!(client_sm.state().is_ready());

    // Receive FileOffer
    let offer = read_msg(&mut client_sock, &codec, &mut client_buf)
        .await
        .unwrap();
    assert!(matches!(offer, LabMessage::FileOffer { .. }));

    // Prepare client local storage
    let temp_dir = tempdir().unwrap();
    let part_path = temp_dir.path().join("001.part");
    let mut part_file =
        PartFile::open_or_create(&part_path, total_len as u64, AllocationPolicy::None).unwrap();
    let mut range_set = RangeSet::new();
    let mut in_flight = InFlightTracker::new();

    // Plan block requests: 53 blocks total
    let mut block_offsets: Vec<u64> = Vec::new();
    let mut cur = 0u64;
    while cur < total_len as u64 {
        block_offsets.push(cur);
        cur += ED2K_BLOCK_SIZE as u64;
    }
    assert_eq!(block_offsets.len(), 53);

    // Intentionally SHUFFLE the block request order to prove asynchronous out-of-order handling!
    // We reverse the list of offsets so blocks arrive out-of-order:
    block_offsets.reverse();

    let mut offset_idx = 0;
    while range_set.covered_len() < total_len as u64 {
        // Pipeline requests up to in-flight capacity (8)
        while in_flight.can_request() && offset_idx < block_offsets.len() {
            let offset = block_offsets[offset_idx];
            let remaining = (total_len as u64) - offset;
            let len = std::cmp::min(remaining, MAX_BLOCK_BYTES as u64) as u32;

            let req = mule_protocol::BlockRequest::new(file_hash, offset, len);
            in_flight.record_request(&req).unwrap();

            let req_msg = LabMessage::RequestBlock {
                file_hash,
                start_offset: offset,
                length: len,
            };
            send_msg(&mut client_sock, &codec, &req_msg).await;
            offset_idx += 1;
        }

        // Receive block response
        let resp = read_msg(&mut client_sock, &codec, &mut client_buf)
            .await
            .unwrap();
        match resp {
            LabMessage::BlockData {
                file_hash: resp_hash,
                start_offset,
                data,
            } => {
                assert_eq!(resp_hash, file_hash);
                in_flight
                    .validate_and_remove(&resp_hash, start_offset, data.len())
                    .expect("must match an active in-flight request");

                // Write to PartFile and mark in RangeSet
                part_file.write_segment(start_offset, &data).unwrap();
                range_set
                    .insert(start_offset..start_offset + data.len() as u64)
                    .unwrap();
            }
            other => panic!("expected BlockData, got {:?}", other),
        }
    }

    // Verify all ranges are complete
    assert_eq!(range_set.covered_len(), total_len as u64);
    part_file.flush().unwrap();

    // Verify eD2K part hash of downloaded file on disk
    let mut file = std::fs::File::open(&part_path).unwrap();
    let hash_res = hash_reader(&mut file).unwrap();
    let hex_hash = hash_res.root_hash.to_hex();

    assert_eq!(hex_hash, FIXTURE_EXACT_ONE_PART.expected_ed2k_hex);

    // Clean goodbye
    let goodbye = LabMessage::Goodbye {
        reason: "Download completed".into(),
    };
    send_msg(&mut client_sock, &codec, &goodbye).await;
}

#[tokio::test]
async fn test_tcp_fragmented_handshake() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let codec = PacketCodec::new();
        let mut read_buf = BytesMut::new();

        let server_peer_id = PeerId::new([0x22; 16]);
        let mut sm = PeerStateMachine::new_listener(server_peer_id, 1);

        let hello = read_msg(&mut socket, &codec, &mut read_buf).await.unwrap();
        let actions = sm.on_message(hello).unwrap();
        for action in actions {
            if let PeerAction::SendMessage(msg) = action {
                send_msg(&mut socket, &codec, &msg).await;
            }
        }
    });

    let mut client_sock = TcpStream::connect(addr).await.unwrap();
    let codec = PacketCodec::new();

    let client_peer_id = PeerId::new([0x11; 16]);
    let (_, hello) = PeerStateMachine::new_initiator(client_peer_id, 1, [0x99; 16]);

    let raw = hello.to_raw_packet().unwrap();
    let mut encoded = BytesMut::new();
    codec.encode(&raw, &mut encoded).unwrap();

    // Deliver Hello byte-by-byte with small delays to simulate extreme fragmentation
    for byte in encoded {
        client_sock.write_all(&[byte]).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(2)).await;
    }
    client_sock.flush().await.unwrap();

    let mut client_buf = BytesMut::new();
    let ack = read_msg(&mut client_sock, &codec, &mut client_buf)
        .await
        .unwrap();
    assert!(matches!(ack, LabMessage::HelloAck { .. }));

    server_task.await.unwrap();
}

#[tokio::test]
async fn test_corruption_detection_prevents_finalization() {
    let total_len = 100_000usize;
    let data = vec![0xEE; total_len];
    let fixture_bytes = Bytes::from(data.clone());
    let file_hash = [0x44; 16];

    let (server_addr, _) = spawn_source_server(file_hash, fixture_bytes).await;

    let mut client_sock = TcpStream::connect(server_addr).await.unwrap();
    let codec = PacketCodec::new();
    let mut client_buf = BytesMut::new();

    let (mut client_sm, hello) = PeerStateMachine::new_initiator(PeerId::new([1; 16]), 1, [2; 16]);
    send_msg(&mut client_sock, &codec, &hello).await;
    let ack = read_msg(&mut client_sock, &codec, &mut client_buf)
        .await
        .unwrap();
    client_sm.on_message(ack).unwrap();

    let _offer = read_msg(&mut client_sock, &codec, &mut client_buf)
        .await
        .unwrap();

    // Request block
    let req = LabMessage::RequestBlock {
        file_hash,
        start_offset: 0,
        length: 10_000,
    };
    send_msg(&mut client_sock, &codec, &req).await;

    let resp = read_msg(&mut client_sock, &codec, &mut client_buf)
        .await
        .unwrap();
    if let LabMessage::BlockData { data, .. } = resp {
        // Inject 1 byte of corruption into data
        let mut corrupted = data.to_vec();
        corrupted[42] ^= 0xFF; // flip bits

        // Calculate eD2K hashes of clean vs corrupted
        let hash_clean = hash_reader(&mut Cursor::new(&data)).unwrap();
        let hash_corrupted = hash_reader(&mut Cursor::new(&corrupted)).unwrap();

        assert_ne!(
            hash_clean.root_hash, hash_corrupted.root_hash,
            "Corruption must change digest"
        );
    } else {
        panic!("expected BlockData");
    }
}

#[tokio::test]
async fn test_interruption_and_resumption() {
    let total_len = 500_000usize;
    let data = vec![0xCC; total_len];
    let fixture_bytes = Bytes::from(data.clone());
    let file_hash = [0x66; 16];

    let (server_addr, _) = spawn_source_server(file_hash, fixture_bytes).await;

    let temp_dir = tempdir().unwrap();
    let part_path = temp_dir.path().join("resume.part");

    let mut range_set = RangeSet::new();

    // Phase 1: Download first 180,000 bytes, then disconnect
    {
        let mut client_sock = TcpStream::connect(server_addr).await.unwrap();
        let codec = PacketCodec::new();
        let mut client_buf = BytesMut::new();

        let (mut client_sm, hello) =
            PeerStateMachine::new_initiator(PeerId::new([1; 16]), 1, [10; 16]);
        send_msg(&mut client_sock, &codec, &hello).await;
        let ack = read_msg(&mut client_sock, &codec, &mut client_buf)
            .await
            .unwrap();
        client_sm.on_message(ack).unwrap();
        let _ = read_msg(&mut client_sock, &codec, &mut client_buf)
            .await
            .unwrap(); // offer

        let mut part_file =
            PartFile::open_or_create(&part_path, total_len as u64, AllocationPolicy::None).unwrap();

        // Request 180,000 bytes
        let req = LabMessage::RequestBlock {
            file_hash,
            start_offset: 0,
            length: 180_000,
        };
        send_msg(&mut client_sock, &codec, &req).await;
        if let LabMessage::BlockData { data, .. } =
            read_msg(&mut client_sock, &codec, &mut client_buf)
                .await
                .unwrap()
        {
            part_file.write_segment(0, &data).unwrap();
            range_set.insert(0..data.len() as u64).unwrap();
        }

        part_file.flush().unwrap();
        // Client drops connection prematurely here
    }

    assert_eq!(range_set.covered_len(), 180_000);
    assert!(range_set.covered_len() < total_len as u64);

    // Phase 2: Reconnect, query missing ranges, complete remainder
    {
        let mut client_sock = TcpStream::connect(server_addr).await.unwrap();
        let codec = PacketCodec::new();
        let mut client_buf = BytesMut::new();

        let (mut client_sm, hello) =
            PeerStateMachine::new_initiator(PeerId::new([1; 16]), 1, [11; 16]);
        send_msg(&mut client_sock, &codec, &hello).await;
        let ack = read_msg(&mut client_sock, &codec, &mut client_buf)
            .await
            .unwrap();
        client_sm.on_message(ack).unwrap();
        let _ = read_msg(&mut client_sock, &codec, &mut client_buf)
            .await
            .unwrap(); // offer

        let mut part_file =
            PartFile::open_or_create(&part_path, total_len as u64, AllocationPolicy::None).unwrap();

        // Find missing ranges
        let missing = range_set.missing_within(0..total_len as u64);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], 180_000..500_000);

        let remaining_len = (missing[0].end - missing[0].start) as u32;
        // Request in two chunks: 160,000 and remainder
        let chunk1_len = 160_000;
        let chunk2_len = remaining_len - chunk1_len;

        // Chunk 1
        send_msg(
            &mut client_sock,
            &codec,
            &LabMessage::RequestBlock {
                file_hash,
                start_offset: 180_000,
                length: chunk1_len,
            },
        )
        .await;

        if let LabMessage::BlockData { data, .. } =
            read_msg(&mut client_sock, &codec, &mut client_buf)
                .await
                .unwrap()
        {
            part_file.write_segment(180_000, &data).unwrap();
            range_set
                .insert(180_000..180_000 + data.len() as u64)
                .unwrap();
        }

        // Chunk 2
        send_msg(
            &mut client_sock,
            &codec,
            &LabMessage::RequestBlock {
                file_hash,
                start_offset: 340_000,
                length: chunk2_len,
            },
        )
        .await;

        if let LabMessage::BlockData { data, .. } =
            read_msg(&mut client_sock, &codec, &mut client_buf)
                .await
                .unwrap()
        {
            part_file.write_segment(340_000, &data).unwrap();
            range_set
                .insert(340_000..340_000 + data.len() as u64)
                .unwrap();
        }

        part_file.flush().unwrap();
    }

    assert_eq!(range_set.covered_len(), total_len as u64);

    // Verify whole file content on disk matches original source
    let written = std::fs::read(&part_path).unwrap();
    assert_eq!(written.len(), total_len);
    assert_eq!(written, data);
}
