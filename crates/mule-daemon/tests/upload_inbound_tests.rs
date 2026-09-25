// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde_json::{json, Value};
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use mule_daemon::rpc::methods::handle_request;
use mule_daemon::{AppState, DaemonConfig, DownloadService, WorkDirMode};
use mule_protocol::ed2k::{Ed2kDecoder, Ed2kEncoder, Ed2kHello, Ed2kRequestParts, PR_ED2K};
use mule_protocol::framing::PacketCodec;
use mule_test_support::{make_req, share_state};

/// Contenu du fichier partagé de test : motif périodique de 256 Ko —
/// le chemin upload ne sert que des octets réellement présents sur disque.
fn shared_bytes() -> Vec<u8> {
    (0..262_144u32).map(|i| (i % 251) as u8).collect()
}

#[tokio::test]
async fn test_inbound_listener_rejected_under_disabled_network() {
    let (state, _dir, _hash) = share_state(
        "test_token_upload",
        "test_shared_archive.iso",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    // Disconnect network via kill switch
    let _ = handle_request(
        &state,
        make_req(1, "network.disconnectAll", None),
        &mut authenticated,
    )
    .await
    .unwrap();

    // Attempt to start listener
    let req = make_req(2, "listener.start", Some(json!({ "port": 0 })));
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_some(),
        "Listener start must fail when network is disabled"
    );
    let err = resp.error.unwrap();
    assert!(err.message.contains("Network is disabled"));
}

#[tokio::test]
async fn test_inbound_handshake_and_request_shared_file_upload_streamed() {
    let (state, _dir, hash) = share_state(
        "test_token_upload",
        "test_shared_archive.iso",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    // Start listener on ephemeral loopback port
    let start_req = make_req(1, "listener.start", Some(json!({ "port": 0 })));
    let start_resp = handle_request(&state, start_req, &mut authenticated)
        .await
        .unwrap();
    assert!(start_resp.error.is_none());
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    // Connect remote mock peer client
    let mut client_socket = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();

    // 1. Send OP_HELLO
    let hello = Ed2kHello {
        user_hash: [0x44u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    // 2. Expect OP_HELLO_ANSWER
    let mut received_hello_ans = false;
    while !received_hello_ans {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                received_hello_ans = true;
                break;
            }
        }
    }

    // 3. Send OP_FILESTATUSREQ for shared file
    let hash_bytes: [u8; 16] = hex::decode(&hash).unwrap().try_into().unwrap();
    let status_req = mule_protocol::ed2k::Ed2kFileStatusReq {
        file_hash: hash_bytes,
    };
    let req_pkt = encoder.encode_file_status_req(&status_req).unwrap();
    out.clear();
    out.push(req_pkt.protocol);
    out.extend_from_slice(&(req_pkt.frame_len() as u32).to_le_bytes());
    out.push(req_pkt.opcode);
    out.extend_from_slice(&req_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    // Expect OP_FILESTATUS with complete availability
    let mut received_status = false;
    while !received_status {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_FILESTATUS {
                received_status = true;
                break;
            }
        }
    }

    // 4. Send OP_REQUESTPARTS for block 0..184320
    let req_parts = Ed2kRequestParts {
        file_hash: hash_bytes,
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange {
            start: 0,
            end: 184320,
        }],
    };
    let rp_pkt = encoder.encode_request_parts(&req_parts).unwrap();
    out.clear();
    out.push(rp_pkt.protocol);
    out.extend_from_slice(&(rp_pkt.frame_len() as u32).to_le_bytes());
    out.push(rp_pkt.opcode);
    out.extend_from_slice(&rp_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    // 5. Expect OP_SENDINGPART
    let mut received_data = false;
    while !received_data {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_SENDINGPART {
                let part = decoder
                    .decode_sending_part(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    .unwrap();
                assert_eq!(part.start_offset, 0);
                assert_eq!(part.end_offset, 184320);
                assert_eq!(part.data.len(), 184320);
                received_data = true;
                break;
            }
        }
    }

    // Verify upload status via RPC
    let status_req = make_req(2, "uploads.status", None);
    let status_resp = handle_request(&state, status_req, &mut authenticated)
        .await
        .unwrap();
    let upload_snap: Value = status_resp.result.unwrap();
    assert_eq!(upload_snap["state"], "uploading");
    assert!(upload_snap["transferredBytes"].as_u64().unwrap() >= 184320);

    // Stop listener
    let stop_req = make_req(3, "listener.stop", None);
    let _ = handle_request(&state, stop_req, &mut authenticated)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_inbound_request_unshared_file_does_not_upload() {
    let (state, _dir, _hash) = share_state(
        "test_token_upload",
        "test_shared_archive.iso",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client_socket = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();

    // Handshake
    let hello = Ed2kHello {
        user_hash: [0x44u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    // Read hello answer
    let mut received_hello_ans = false;
    while !received_hello_ans {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                received_hello_ans = true;
                break;
            }
        }
    }

    // Request UNSHARED file
    let unshared_hash = [0x99u8; 16];
    let req_parts = Ed2kRequestParts {
        file_hash: unshared_hash,
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange {
            start: 0,
            end: 184320,
        }],
    };
    let rp_pkt = encoder.encode_request_parts(&req_parts).unwrap();
    out.clear();
    out.push(rp_pkt.protocol);
    out.extend_from_slice(&(rp_pkt.frame_len() as u32).to_le_bytes());
    out.push(rp_pkt.opcode);
    out.extend_from_slice(&rp_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    // Check active upload remains empty
    tokio::time::sleep(Duration::from_millis(100)).await;
    let status_resp = handle_request(
        &state,
        make_req(2, "uploads.status", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(
        status_resp.result.unwrap().is_null(),
        "Unshared file request must not allocate upload slot"
    );

    let _ = handle_request(
        &state,
        make_req(3, "listener.stop", None),
        &mut authenticated,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_upload_queue_ranking_and_single_slot_concurrency() {
    let (state, _dir, hash) = share_state(
        "test_token_upload",
        "test_shared_archive.iso",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    // Restrict to a single active upload slot so queueing behavior is
    // exercised deterministically (production default is MAX_ACTIVE_UPLOADS).
    state.inbound_listener.upload_manager().set_max_active(1);

    // Start listener
    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();
    let hash_bytes: [u8; 16] = hex::decode(&hash).unwrap().try_into().unwrap();

    // Client 1 connects and takes slot 1
    let mut client1 = TcpStream::connect(&listen_addr_str).await.unwrap();
    let hello1 = Ed2kHello {
        user_hash: [0x11u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let h1_pkt = encoder.encode_hello(&hello1).unwrap();
    let mut out1 = Vec::new();
    out1.push(h1_pkt.protocol);
    out1.extend_from_slice(&(h1_pkt.frame_len() as u32).to_le_bytes());
    out1.push(h1_pkt.opcode);
    out1.extend_from_slice(&h1_pkt.payload);
    client1.write_all(&out1).await.unwrap();

    // Read hello answer for client 1
    let mut raw_buf = [0u8; 8192];
    let mut read_buf1 = bytes::BytesMut::with_capacity(65536);
    let mut h1_ok = false;
    while !h1_ok {
        let n = client1.read(&mut raw_buf).await.unwrap();
        read_buf1.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf1) {
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                h1_ok = true;
                break;
            }
        }
    }

    // Request part on client 1
    let req_parts1 = Ed2kRequestParts {
        file_hash: hash_bytes,
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange {
            start: 0,
            end: 184320,
        }],
    };
    let rp_pkt1 = encoder.encode_request_parts(&req_parts1).unwrap();
    out1.clear();
    out1.push(rp_pkt1.protocol);
    out1.extend_from_slice(&(rp_pkt1.frame_len() as u32).to_le_bytes());
    out1.push(rp_pkt1.opcode);
    out1.extend_from_slice(&rp_pkt1.payload);
    client1.write_all(&out1).await.unwrap();

    // Verify client 1 receives sending part
    let mut c1_got_data = false;
    while !c1_got_data {
        let n = client1.read(&mut raw_buf).await.unwrap();
        read_buf1.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf1) {
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_SENDINGPART {
                c1_got_data = true;
                break;
            }
        }
    }

    // Client 2 connects while Client 1 is active (max active = 1)
    let mut client2 = TcpStream::connect(&listen_addr_str).await.unwrap();
    let hello2 = Ed2kHello {
        user_hash: [0x22u8; 16],
        client_id: 0x0100007F,
        tcp_port: 49663,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let h2_pkt = encoder.encode_hello(&hello2).unwrap();
    let mut out2 = Vec::new();
    out2.push(h2_pkt.protocol);
    out2.extend_from_slice(&(h2_pkt.frame_len() as u32).to_le_bytes());
    out2.push(h2_pkt.opcode);
    out2.extend_from_slice(&h2_pkt.payload);
    client2.write_all(&out2).await.unwrap();

    let mut read_buf2 = bytes::BytesMut::with_capacity(65536);
    let mut h2_ok = false;
    while !h2_ok {
        let n = client2.read(&mut raw_buf).await.unwrap();
        read_buf2.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf2) {
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                h2_ok = true;
                break;
            }
        }
    }

    // Request part on client 2 -> must receive OP_QUEUERANKING with rank 1
    let req_parts2 = Ed2kRequestParts {
        file_hash: hash_bytes,
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange {
            start: 0,
            end: 184320,
        }],
    };
    let rp_pkt2 = encoder.encode_request_parts(&req_parts2).unwrap();
    out2.clear();
    out2.push(rp_pkt2.protocol);
    out2.extend_from_slice(&(rp_pkt2.frame_len() as u32).to_le_bytes());
    out2.push(rp_pkt2.opcode);
    out2.extend_from_slice(&rp_pkt2.payload);
    client2.write_all(&out2).await.unwrap();

    let mut received_queue_ranking = false;
    while !received_queue_ranking {
        let n = client2.read(&mut raw_buf).await.unwrap();
        read_buf2.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf2) {
            if pkt.protocol == mule_protocol::ed2k::PR_EMULE && pkt.opcode == mule_protocol::ed2k::OP_QUEUERANKING {
                let rank = u16::from_le_bytes(pkt.payload[0..2].try_into().unwrap()) as u32;
                assert_eq!(rank, 1, "Client 2 should have queue rank 1");
                received_queue_ranking = true;
                break;
            }
        }
    }

    // Verify uploads.list RPC returns active upload and queued upload
    let list_resp = handle_request(
        &state,
        make_req(2, "uploads.list", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    let uploads_list: Value = list_resp.result.unwrap();
    assert_eq!(uploads_list["active"].as_array().unwrap().len(), 1);
    assert_eq!(uploads_list["queued"].as_array().unwrap().len(), 1);

    // Multi-slot promotion: raising the limit then releasing client 1's slot
    // must promote client 2, which then receives data on its next request.
    state.inbound_listener.upload_manager().set_max_active(2);
    drop(client1);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let rp_pkt2b = encoder.encode_request_parts(&req_parts2).unwrap();
    out2.clear();
    out2.push(rp_pkt2b.protocol);
    out2.extend_from_slice(&(rp_pkt2b.frame_len() as u32).to_le_bytes());
    out2.push(rp_pkt2b.opcode);
    out2.extend_from_slice(&rp_pkt2b.payload);
    client2.write_all(&out2).await.unwrap();

    let mut c2_got_data = false;
    while !c2_got_data {
        let n = client2.read(&mut raw_buf).await.unwrap();
        read_buf2.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf2) {
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_SENDINGPART {
                c2_got_data = true;
                break;
            }
        }
    }

    let _ = handle_request(
        &state,
        make_req(3, "listener.stop", None),
        &mut authenticated,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_kill_switch_aborts_inbound_listener_and_active_uploads() {
    let (state, _dir, _hash) = share_state(
        "test_token_upload",
        "test_shared_archive.iso",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    // Start listener
    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();

    let mut client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let hello = Ed2kHello {
        user_hash: [0x33u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let h_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(h_pkt.protocol);
    out.extend_from_slice(&(h_pkt.frame_len() as u32).to_le_bytes());
    out.push(h_pkt.opcode);
    out.extend_from_slice(&h_pkt.payload);
    client.write_all(&out).await.unwrap();

    let mut raw_buf = [0u8; 8192];
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut h_ok = false;
    while !h_ok {
        let n = client.read(&mut raw_buf).await.unwrap();
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                h_ok = true;
                break;
            }
        }
    }

    // Trigger Kill Switch
    let kill_resp = handle_request(
        &state,
        make_req(2, "network.disconnectAll", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(kill_resp.error.is_none());

    // Verify listener is now stopped
    tokio::time::sleep(Duration::from_millis(50)).await;
    let status_resp = handle_request(
        &state,
        make_req(3, "listener.status", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    let status_val: Value = status_resp.result.unwrap();
    assert_eq!(status_val["isListening"], false);
    assert_eq!(status_val["activeUploads"], 0);

    // Socket should receive EOF on read
    let n = client.read(&mut raw_buf).await.unwrap_or(0);
    assert_eq!(n, 0, "Socket must be closed when kill switch triggers");
}

#[tokio::test]
async fn test_inbound_handshake_returns_real_identity_and_listen_port() {
    let (state, _dir, _hash) = share_state(
        "test_token_upload",
        "test_shared_archive.iso",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    // Start listener
    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();
    let listen_port: u16 = listen_addr_str
        .split(':')
        .next_back()
        .unwrap()
        .parse()
        .unwrap();

    let expected_hash = state.identity.lock().await.user_hash();

    let mut client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();

    let hello = Ed2kHello {
        user_hash: [0x11u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let h_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(h_pkt.protocol);
    out.extend_from_slice(&(h_pkt.frame_len() as u32).to_le_bytes());
    out.push(h_pkt.opcode);
    out.extend_from_slice(&h_pkt.payload);
    client.write_all(&out).await.unwrap();

    let mut raw_buf = [0u8; 8192];
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut ans_opt = None;
    while ans_opt.is_none() {
        let n = client.read(&mut raw_buf).await.unwrap();
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                let ans = decoder
                    .decode_hello_answer(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    .unwrap();
                ans_opt = Some(ans);
                break;
            }
        }
    }

    let ans = ans_opt.unwrap();
    assert_eq!(
        ans.user_hash, expected_hash,
        "Ed2kHelloAnswer must use real identity user_hash"
    );
    assert_eq!(
        ans.tcp_port, listen_port,
        "Ed2kHelloAnswer must use actual listener port"
    );
}

#[tokio::test]
async fn test_inbound_connection_per_ip_limit() {
    let (state, _dir, _hash) = share_state(
        "test_token_upload",
        "test_shared_archive.iso",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    // Start listener
    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    // Connect 5 sockets (MAX_INBOUND_PER_IP)
    let mut clients = Vec::new();
    for _ in 0..5 {
        let s = TcpStream::connect(&listen_addr_str).await.unwrap();
        clients.push(s);
    }

    // Connect 6th socket: should be accepted and then dropped/closed immediately by per-IP limit
    let mut sixth = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut buf = [0u8; 128];
    // Reading on 6th socket should yield EOF
    let res = tokio::time::timeout(Duration::from_millis(500), sixth.read(&mut buf)).await;
    match res {
        Ok(Ok(0)) => {
            // Expected EOF due to drop
        }
        Ok(Ok(n)) => panic!("Expected EOF on 6th socket, but got {n} bytes"),
        Ok(Err(e)) => panic!("Expected EOF on 6th socket, got error: {e}"),
        Err(_) => panic!("Timed out waiting for EOF on 6th connection (not dropped)"),
    }

    // Drop one of the 5 clients
    drop(clients.pop());
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Now a new connection should be accepted and stay open
    let mut new_client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let hello = Ed2kHello {
        user_hash: [0x22u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let encoder = Ed2kEncoder::default();
    let h_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(h_pkt.protocol);
    out.extend_from_slice(&(h_pkt.frame_len() as u32).to_le_bytes());
    out.push(h_pkt.opcode);
    out.extend_from_slice(&h_pkt.payload);
    new_client.write_all(&out).await.unwrap();

    let mut raw_buf = [0u8; 8192];
    let n = new_client.read(&mut raw_buf).await.unwrap();
    assert!(
        n > 0,
        "New connection should be accepted after dropping an existing client"
    );
}

#[tokio::test]
async fn test_inbound_secident_signature_without_challenge_rejected() {
    let (state, _dir, _hash) = share_state(
        "test_token_upload",
        "test_shared_archive.iso",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    // Start listener
    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();

    // 1. Handshake
    let hello = Ed2kHello {
        user_hash: [0x33u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let h_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(h_pkt.protocol);
    out.extend_from_slice(&(h_pkt.frame_len() as u32).to_le_bytes());
    out.push(h_pkt.opcode);
    out.extend_from_slice(&h_pkt.payload);
    client.write_all(&out).await.unwrap();

    let mut raw_buf = [0u8; 8192];
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut h_ok = false;
    while !h_ok {
        let n = client.read(&mut raw_buf).await.unwrap();
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                h_ok = true;
                break;
            }
        }
    }

    // 2. Send OP_SIGNATURE without having sent OP_PUBLICKEY / received challenge
    let sig = mule_protocol::ed2k::Ed2kSignature {
        signature_bytes: vec![0x99u8; 64].into(),
    };
    let sig_pkt = encoder.encode_signature(&sig).unwrap();
    out.clear();
    out.push(sig_pkt.protocol);
    out.extend_from_slice(&(sig_pkt.frame_len() as u32).to_le_bytes());
    out.push(sig_pkt.opcode);
    out.extend_from_slice(&sig_pkt.payload);
    client.write_all(&out).await.unwrap();

    // 3. Expect Ed2kSecIdentState with state == 0 (rejected / unauthenticated)
    let mut state_opt = None;
    while state_opt.is_none() {
        let n = client.read(&mut raw_buf).await.unwrap();
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == mule_protocol::ed2k::PR_EMULE
                && pkt.opcode == mule_protocol::ed2k::OP_SECIDENTSTATE
            {
                let st = decoder
                    .decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    .unwrap();
                state_opt = Some(st);
                break;
            }
        }
    }

    let st = state_opt.unwrap();
    assert_eq!(
        st.state, 0,
        "Unsolicited signature without prior challenge must be rejected with state 0"
    );
}

#[tokio::test]
async fn test_shares_rescan_discovers_new_files_and_preserves_settings() {
    let dir = tempdir().unwrap();
    let download_dir = dir.path().join("downloads");
    std::fs::create_dir_all(&download_dir).unwrap();
    std::fs::write(download_dir.join("rescan_target.bin"), b"rescan-me").unwrap();
    // A download artifact must be skipped.
    std::fs::write(download_dir.join("pending.part"), b"partial").unwrap();

    let token = "test_token_rescan".to_string();
    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable)
        .with_default_download_dir(download_dir.clone());
    let downloads = DownloadService::in_memory(dir.path().join("temp")).unwrap();
    let state = AppState::new(config, downloads);
    let mut authenticated = true;

    // First rescan: the new file is discovered, the .part is skipped.
    let resp = handle_request(
        &state,
        make_req(1, "shares.rescan", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());
    assert_eq!(resp.result.as_ref().unwrap()["added"], 1);

    // List to find the registered share, then change its settings.
    let resp = handle_request(&state, make_req(2, "shares.list", None), &mut authenticated)
        .await
        .unwrap();
    let shares = resp.result.as_ref().unwrap()["sharedFiles"]
        .as_array()
        .unwrap();
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0]["fileName"], "rescan_target.bin");
    let hash = shares[0]["fileHash"].as_str().unwrap().to_string();

    let resp = handle_request(
        &state,
        make_req(
            3,
            "shares.setUploadPriority",
            Some(json!({ "fileHash": hash, "priority": "powershare" })),
        ),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());
    let resp = handle_request(
        &state,
        make_req(
            4,
            "shares.setEnabled",
            Some(json!({ "fileHash": hash, "enabled": false })),
        ),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());

    // Second rescan: nothing new, existing entry keeps its settings.
    let resp = handle_request(
        &state,
        make_req(5, "shares.rescan", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert_eq!(resp.result.as_ref().unwrap()["added"], 0);

    let resp = handle_request(&state, make_req(6, "shares.list", None), &mut authenticated)
        .await
        .unwrap();
    let shares = resp.result.as_ref().unwrap()["sharedFiles"]
        .as_array()
        .unwrap();
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0]["upPriority"], "powershare");
    assert_eq!(shares[0]["enabled"], false);
}

#[tokio::test]
async fn test_shares_rescan_is_recursive() {
    let dir = tempdir().unwrap();
    let download_dir = dir.path().join("downloads");
    let nested = download_dir.join("Album").join("CD1");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("track01.flac"), b"nested-audio").unwrap();
    std::fs::write(download_dir.join("toplevel.bin"), b"top").unwrap();
    // Hidden directories must not be shared.
    let hidden = download_dir.join(".hidden");
    std::fs::create_dir_all(&hidden).unwrap();
    std::fs::write(hidden.join("secret.bin"), b"nope").unwrap();

    let token = "test_token_recursive".to_string();
    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable)
        .with_default_download_dir(download_dir);
    let downloads = DownloadService::in_memory(dir.path().join("temp")).unwrap();
    let state = AppState::new(config, downloads);
    let mut authenticated = true;

    let resp = handle_request(
        &state,
        make_req(1, "shares.rescan", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());
    assert_eq!(resp.result.as_ref().unwrap()["added"], 2);

    let resp = handle_request(&state, make_req(2, "shares.list", None), &mut authenticated)
        .await
        .unwrap();
    let shares = resp.result.as_ref().unwrap()["sharedFiles"]
        .as_array()
        .unwrap();
    let names: Vec<&str> = shares
        .iter()
        .map(|s| s["fileName"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"track01.flac"));
    assert!(names.contains(&"toplevel.bin"));
    assert!(!names.contains(&"secret.bin"));
}

#[tokio::test]
async fn test_shares_rescan_prunes_files_deleted_from_disk() {
    let dir = tempdir().unwrap();
    let download_dir = dir.path().join("downloads");
    std::fs::create_dir_all(&download_dir).unwrap();
    std::fs::write(download_dir.join("keep.bin"), b"keep").unwrap();
    std::fs::write(download_dir.join("gone.bin"), b"gone").unwrap();

    let token = "test_token_prune".to_string();
    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable)
        .with_default_download_dir(download_dir.clone());
    let downloads = DownloadService::in_memory(dir.path().join("temp")).unwrap();
    let state = AppState::new(config, downloads);
    let mut authenticated = true;

    let resp = handle_request(
        &state,
        make_req(1, "shares.rescan", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());
    assert_eq!(resp.result.as_ref().unwrap()["added"], 2);

    // Suppression physique du fichier hors daemon.
    std::fs::remove_file(download_dir.join("gone.bin")).unwrap();

    // La liste ne doit déjà plus annoncer le fichier mort (filtre à la
    // lecture dans `list_all_shared_files`), avant même un nouveau scan.
    let resp = handle_request(&state, make_req(2, "shares.list", None), &mut authenticated)
        .await
        .unwrap();
    let shares = resp.result.as_ref().unwrap()["sharedFiles"]
        .as_array()
        .unwrap();
    let names: Vec<&str> = shares
        .iter()
        .map(|s| s["fileName"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"keep.bin"));
    assert!(!names.contains(&"gone.bin"));

    // Le rescan purge définitivement la ligne `shared_files`.
    let resp = handle_request(
        &state,
        make_req(3, "shares.rescan", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());
    assert_eq!(resp.result.as_ref().unwrap()["removed"], 1);
    assert_eq!(resp.result.as_ref().unwrap()["added"], 0);
}

#[tokio::test]
async fn test_shares_rescan_respects_share_download_dir_optout() {
    let dir = tempdir().unwrap();
    let download_dir = dir.path().join("downloads");
    std::fs::create_dir_all(&download_dir).unwrap();
    std::fs::write(download_dir.join("should_not_share.bin"), b"x").unwrap();

    let token = "test_token_rescan_off".to_string();
    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable)
        .with_default_download_dir(download_dir.clone());
    let downloads = DownloadService::in_memory(dir.path().join("temp")).unwrap();
    let state = AppState::new(config, downloads);
    let mut authenticated = true;

    // Disable Incoming sharing via the settings RPC.
    let resp = handle_request(
        &state,
        make_req(
            1,
            "settings.set",
            Some(json!({ "shareDownloadDir": false })),
        ),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());

    let resp = handle_request(
        &state,
        make_req(2, "shares.rescan", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());
    assert_eq!(resp.result.as_ref().unwrap()["added"], 0);

    let resp = handle_request(&state, make_req(3, "shares.list", None), &mut authenticated)
        .await
        .unwrap();
    assert!(resp.result.as_ref().unwrap()["sharedFiles"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn test_partial_download_is_shared_with_completed_parts_only() {
    let dir = tempdir().unwrap();
    let token = "test_token_partial".to_string();
    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable);
    let downloads = DownloadService::in_memory(dir.path().join("temp")).unwrap();
    let state = AppState::new(config, downloads);
    let mut authenticated = true;

    // Add a download > 1 eD2K part (2 * 9_728_000 bytes).
    let resp = handle_request(
        &state,
        make_req(
            1,
            "downloads.addEd2kLink",
            Some(json!({
                "link": "ed2k://|file|partial.bin|19456000|0123456789ABCDEF0123456789ABCDEF|/"
            })),
        ),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());
    let id = resp.result.as_ref().unwrap()["downloadId"]
        .as_str()
        .unwrap()
        .to_string();

    // No completed part yet: still shared as a partial (eMule
    // `CDownloadQueue::AddPartFilesToShare` — "part files are always
    // shared files"), exposing an empty completed-parts mask.
    {
        let dl = state.downloads.lock().await;
        let sf = dl
            .get_shared_file("0123456789abcdef0123456789abcdef")
            .unwrap()
            .expect("in-progress download must be shared even with 0 complete parts");
        assert!(sf.is_partial);
        assert!(sf.completed_parts.is_empty());
        assert!(sf.file_path.ends_with(".part"));

        let all = dl.list_all_shared_files().unwrap();
        assert!(all.iter().any(|f| f.is_partial
            && f.completed_parts.is_empty()
            && f.file_hash
                .eq_ignore_ascii_case("0123456789abcdef0123456789abcdef")));
    }

    // Complete part 0 (9 728 000 bytes) via inject_range.
    {
        let mut dl = state.downloads.lock().await;
        let uuid = uuid::Uuid::parse_str(&id).unwrap();
        dl.inject_range(&uuid, 0, &vec![0xABu8; 9_728_000], None)
            .unwrap();
    }

    // Now the partial share must resolve with only part 0 completed.
    {
        let dl = state.downloads.lock().await;
        let sf = dl
            .get_shared_file("0123456789abcdef0123456789abcdef")
            .unwrap()
            .expect("partial download with 1 complete part must be shared");
        assert!(sf.is_partial);
        assert_eq!(sf.completed_parts, vec![0]);
        assert!(sf.file_path.ends_with(".part"));

        // And it must appear in shares.list output.
        let all = dl.list_all_shared_files().unwrap();
        assert!(all.iter().any(|f| f.is_partial
            && f.file_hash
                .eq_ignore_ascii_case("0123456789abcdef0123456789abcdef")));
    }
}

#[tokio::test]
async fn test_settings_new_keys_roundtrip_and_live_apply() {
    let dir = tempdir().unwrap();
    let token = "test_token_settings".to_string();
    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable);
    let downloads = DownloadService::in_memory(dir.path().join("temp")).unwrap();
    let state = AppState::new(config, downloads);
    let mut authenticated = true;

    let resp = handle_request(
        &state,
        make_req(
            1,
            "settings.set",
            Some(json!({
                "nickname": "TestNick",
                "obfuscationEnabled": true,
                "obfuscationRequired": true,
                "maxConnections": 700,
                "maxConnPerMin": 120,
                "maxUploadQueue": 800,
                "ussEnabled": true,
                "ed2kPort": 4723,
                "kadPort": 4725,
                "udpEnabled": false,
                "upnpEnabled": false,
                "capacityDownloadKbs": 96,
                "capacityUploadKbs": 100,
                "minUploadKbs": 32,
                "ichEnabled": false,
                "rememberCancelled": false,
                "rememberDownloaded": true,
                "serverReconnect": false,
                "serverAutoconnect": true,
                "serverAutoremoveDead": false,
                "serverAutoupdateList": true,
                "serverAutoupdatePath": "C:/srv/server.met",
                "safeConnect": true,
                "autoPriorityNewDownloads": true,
                "tempDir": dir.path().join("tp").display().to_string(),
            })),
        ),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());

    let resp = handle_request(
        &state,
        make_req(2, "settings.get", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    let r = resp.result.as_ref().unwrap();
    assert_eq!(r["nickname"], "TestNick");
    assert_eq!(r["obfuscationEnabled"], true);
    assert_eq!(r["obfuscationRequired"], true);
    assert_eq!(r["maxConnections"], 700);
    assert_eq!(r["maxConnPerMin"], 120);
    assert_eq!(r["maxUploadQueue"], 800);
    assert_eq!(r["ussEnabled"], true);
    assert_eq!(r["ed2kPort"], 4723);
    assert_eq!(r["kadPort"], 4725);
    assert_eq!(r["udpEnabled"], false);
    assert_eq!(r["capacityDownloadKbs"], 96);
    assert_eq!(r["capacityUploadKbs"], 100);
    assert_eq!(r["minUploadKbs"], 32);
    assert_eq!(r["ichEnabled"], false);
    assert_eq!(r["rememberCancelled"], false);
    assert_eq!(r["rememberDownloaded"], true);
    assert_eq!(r["serverReconnect"], false);
    assert_eq!(r["serverAutoconnect"], true);
    assert_eq!(r["serverAutoremoveDead"], false);
    assert_eq!(r["serverAutoupdateList"], true);
    assert_eq!(r["serverAutoupdatePath"], "C:/srv/server.met");
    assert_eq!(r["safeConnect"], true);
    assert_eq!(r["autoPriorityNewDownloads"], true);
    assert_eq!(
        r["tempDir"],
        dir.path().join("tp").display().to_string().as_str()
    );
    assert_eq!(state.ed2k_port(), 4723);
    assert_eq!(state.kad_port(), 4725);
    assert!(!state.udp_enabled());

    // Live-applied: effective obfuscation + nickname helpers.
    assert_eq!(state.effective_obfuscation().await, (true, true));
    assert_eq!(state.current_nickname().await, "TestNick");

    // Nickname validation: empty rejected.
    let resp = handle_request(
        &state,
        make_req(3, "settings.set", Some(json!({ "nickname": "  " }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_some());
}

/// Regression: completed downloads persisted in the DB but absent from the
/// in-memory `active` map (e.g. after a daemon restart) must still appear
/// in `shares.list` and be resolvable by `get_shared_file` — eMule rehashes
/// finished files into `CSharedFileList` at startup.
#[tokio::test]
async fn test_completed_db_download_is_shared_after_restart() {
    let dir = tempdir().unwrap();
    let token = "test_token_dbrestart".to_string();
    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable);
    let downloads = DownloadService::in_memory(dir.path().join("temp")).unwrap();
    let state = AppState::new(config, downloads);
    let mut authenticated = true;

    // A finished file on disk + a Completed row in the DB, nothing in
    // `active` (simulating a fresh daemon boot).
    let file_path = dir.path().join("downloads").join("finished.bin");
    std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
    std::fs::write(&file_path, b"done").unwrap();
    let hash = mule_format::Ed2kHash([0x42; 16]);
    {
        let mut dl = state.downloads.lock().await;
        dl.db_mut()
            .insert_download(&mule_core::model::Download {
                id: uuid::Uuid::new_v4(),
                display_name: "finished.bin".into(),
                total_size: 4,
                ed2k_hash: hash,
                aich_root: None,
                target_path: file_path.clone(),
                temp_path: dir.path().join("temp").join("x.part"),
                status: mule_core::model::DownloadStatus::Completed,
                created_at: 0,
                updated_at: 0,
                category_id: None,
                priority: "normal".into(),
                max_sources: None,
            })
            .unwrap();
    }

    let resp = handle_request(&state, make_req(1, "shares.list", None), &mut authenticated)
        .await
        .unwrap();
    let shares = resp.result.as_ref().unwrap()["sharedFiles"]
        .as_array()
        .unwrap();
    assert!(
        shares.iter().any(|s| s["fileName"] == "finished.bin"),
        "completed DB download must appear in shares.list: {shares:?}"
    );
    // Peers must be able to resolve it for upload.
    let sf = {
        let dl = state.downloads.lock().await;
        dl.get_shared_file(&hash.to_hex()).unwrap()
    };
    let sf = sf.expect("get_shared_file must resolve completed DB download");
    assert!(!sf.is_partial);
    assert_eq!(sf.file_path, file_path.to_string_lossy());
}

/// eMule ListenSocket.cpp:651-825 -- browsing des partages : le pair
/// envoie OP_ASKSHAREDFILES / OP_ASKSHAREDDIRS / OP_ASKSHAREDFILESDIR sur
/// PR_ED2K et recoit les reponses 0x4B / 0x5F / 0x60 (0x61 si refus).
#[tokio::test]
async fn test_inbound_shared_files_browse() {
    let (state, _dir, hash) = share_state(
        "test_token_browse",
        "browse_shared.bin",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();

    // OP_HELLO puis OP_HELLO_ANSWER.
    let hello = Ed2kHello {
        user_hash: [0x55u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client.write_all(&out).await.unwrap();
    loop {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        let mut got = false;
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                got = true;
            }
        }
        if got {
            break;
        }
    }

    // Envoi d'une trame PR_ED2K brute (opcode + payload).
    async fn send_raw(client: &mut TcpStream, opcode: u8, payload: &[u8]) {
        let mut out = Vec::with_capacity(6 + payload.len());
        out.push(PR_ED2K);
        out.extend_from_slice(&((payload.len() as u32 + 1).to_le_bytes()));
        out.push(opcode);
        out.extend_from_slice(payload);
        client.write_all(&out).await.unwrap();
    }
    async fn recv_opcode(
        client: &mut TcpStream,
        read_buf: &mut bytes::BytesMut,
        codec: &PacketCodec,
        want: u8,
    ) -> mule_protocol::framing::RawPacket {
        let mut raw = [0u8; 16384];
        loop {
            let n = client.read(&mut raw).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw[..n]);
            while let Ok(Some(pkt)) = codec.decode(read_buf) {
                if pkt.protocol == PR_ED2K && pkt.opcode == want {
                    return pkt;
                }
            }
        }
    }

    // 1. OP_ASKSHAREDFILES (0x4A, vide) -> 0x4B avec notre fichier.
    send_raw(&mut client, mule_protocol::ed2k::OP_ASKSHAREDFILES, &[]).await;
    let pkt = recv_opcode(
        &mut client,
        &mut read_buf,
        &codec,
        mule_protocol::ed2k::OP_ASKSHAREDFILESANSWER,
    )
    .await;
    let count = u32::from_le_bytes(pkt.payload[..4].try_into().unwrap());
    assert_eq!(count, 1, "one shared file expected in answer");
    assert_eq!(&pkt.payload[4..20], &hex::decode(&hash).unwrap()[..]);

    // 2. OP_ASKSHAREDDIRS (0x5D) -> 0x5F avec le pseudo-repertoire parent.
    send_raw(&mut client, mule_protocol::ed2k::OP_ASKSHAREDDIRS, &[]).await;
    let pkt = recv_opcode(
        &mut client,
        &mut read_buf,
        &codec,
        mule_protocol::ed2k::OP_ASKSHAREDDIRSANS,
    )
    .await;
    let dir_count = u32::from_le_bytes(pkt.payload[..4].try_into().unwrap());
    assert!(dir_count >= 1, "at least one pseudo dir expected");

    // 3. OP_ASKSHAREDFILESDIR (0x5E) sur '!Incomplete Files' -> 0x60
    //    avec 0 fichier (aucun .part actif).
    let pseudo = b"!Incomplete Files";
    let mut payload = Vec::new();
    payload.extend_from_slice(&(pseudo.len() as u16).to_le_bytes());
    payload.extend_from_slice(pseudo);
    send_raw(
        &mut client,
        mule_protocol::ed2k::OP_ASKSHAREDFILESDIR,
        &payload,
    )
    .await;
    let pkt = recv_opcode(
        &mut client,
        &mut read_buf,
        &codec,
        mule_protocol::ed2k::OP_ASKSHAREDFILESDIRANS,
    )
    .await;
    let echoed_len = u16::from_le_bytes([pkt.payload[0], pkt.payload[1]]) as usize;
    assert_eq!(&pkt.payload[2..2 + echoed_len], pseudo);
    assert_eq!(
        u32::from_le_bytes(
            pkt.payload[2 + echoed_len..2 + echoed_len + 4]
                .try_into()
                .unwrap()
        ),
        0,
        "no active part files -> empty list"
    );

    // 4. viewSharedFiles=nobody -> OP_ASKSHAREDDIRS refuse (0x61 vide).
    let set_resp = handle_request(
        &state,
        make_req(
            2,
            "settings.set",
            Some(json!({ "viewSharedFiles": "nobody" })),
        ),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(set_resp.error.is_none());
    send_raw(&mut client, mule_protocol::ed2k::OP_ASKSHAREDDIRS, &[]).await;
    let pkt = recv_opcode(
        &mut client,
        &mut read_buf,
        &codec,
        mule_protocol::ed2k::OP_ASKSHAREDDENIEDANS,
    )
    .await;
    assert!(pkt.payload.is_empty(), "denied answer is empty");

    let _ = handle_request(
        &state,
        make_req(3, "listener.stop", None),
        &mut authenticated,
    )
    .await;
}

/// eMule `ListenSocket.cpp:1623` — `OP_KAD_FWTCPCHECK_ACK` (0xA8, PR_EMULE,
/// vide) arrivant sur une connexion entrante : c'est le connect-back d'un
/// contact à qui on a demandé un test pare-feu TCP. Gate
/// `IsKadFirewallCheckIP` → `IncFirewalled`; non sollicité → ignoré.
#[tokio::test]
async fn test_inbound_kad_fw_tcpcheck_ack() {
    let (state, _dir, _hash) = share_state(
        "test_token_fwack",
        "fwack_shared.bin",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    // Enregistre 127.0.0.1 comme IP sollicitée (AddKadFirewallRequest).
    {
        let tracker = state.kad.udp().firewall_tracker();
        tracker
            .lock()
            .await
            .add_tcp_check_ip(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    }

    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();

    let hello = Ed2kHello {
        user_hash: [0x66u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client.write_all(&out).await.unwrap();
    loop {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        let mut got = false;
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                got = true;
            }
        }
        if got {
            break;
        }
    }

    // 0xA8 sollicité (PR_EMULE, payload vide) -> inc_tcp_acks.
    let mut frame = Vec::with_capacity(6);
    frame.push(mule_protocol::ed2k::PR_EMULE);
    frame.extend_from_slice(&1u32.to_le_bytes());
    frame.push(mule_protocol::ed2k::OP_KAD_FWTCPCHECK_ACK);
    client.write_all(&frame).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    {
        let tracker = state.kad.udp().firewall_tracker();
        assert_eq!(tracker.lock().await.tcp_acks, 1, "solicited 0xA8 counted");
    }

    // Unrequested 0xA8 from an unregistered IP would be ignored — here the
    // same socket stays registered, so a second ACK still counts (eMule keeps
    // counting while the pending entry lives; verdict flips at 2).
    client.write_all(&frame).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    {
        let tracker = state.kad.udp().firewall_tracker();
        let fw = tracker.lock().await;
        assert_eq!(fw.tcp_acks, 2);
        assert!(!fw.tcp_is_firewalled(), "2 ACKs -> TCP open verdict");
    }

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await;
}

/// `OP_PUBLICIP_REQ` (0x97, PR_EMULE) — eMule `ListenSocket.cpp:1297` :
/// répondre `OP_PUBLICIP_ANSWER` (0x98) avec l'IP du demandeur telle que
/// vue depuis notre socket (octets réseau bruts).
#[tokio::test]
async fn test_inbound_publicip_req_echoes_requester_ip() {
    let (state, _dir, _hash) = share_state(
        "test_token_pubip",
        "pubip_shared.bin",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();

    let hello = Ed2kHello {
        user_hash: [0x77u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client.write_all(&out).await.unwrap();
    loop {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        let mut got = false;
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                got = true;
            }
        }
        if got {
            break;
        }
    }

    // PUBLICIP_REQ (payload vide) → PUBLICIP_ANSWER [u32 requester_ip].
    let mut frame = Vec::with_capacity(6);
    frame.push(mule_protocol::ed2k::PR_EMULE);
    frame.extend_from_slice(&1u32.to_le_bytes());
    frame.push(mule_protocol::ed2k::OP_PUBLICIP_REQ);
    client.write_all(&frame).await.unwrap();

    let answer = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let n = client.read(&mut raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == mule_protocol::ed2k::PR_EMULE
                    && pkt.opcode == mule_protocol::ed2k::OP_PUBLICIP_ANSWER
                {
                    return pkt.payload;
                }
            }
        }
    })
    .await
    .expect("PUBLICIP_ANSWER attendu");
    assert_eq!(answer.len(), 4);
    assert_eq!(&answer[..], &[127, 0, 0, 1], "requester IP echoed verbatim");

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await;
}

/// `OP_PACKEDPROT` (0xD4) — `ListenSocket.cpp:1744` + `Packet::UnPackPacket` :
/// un paquet pair zlib-compressé doit être inflaté puis dispatché comme un
/// `PR_EMULE` normal. On pack un `OP_PUBLICIP_REQ` (payload ignoré par le
/// bras) et on attend `OP_PUBLICIP_ANSWER`.
#[tokio::test]
async fn test_inbound_packed_packet_is_unpacked_and_dispatched() {
    let (state, _dir, _hash) = share_state(
        "test_token_packed",
        "packed_shared.bin",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();

    let hello = Ed2kHello {
        user_hash: [0x88u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client.write_all(&out).await.unwrap();
    loop {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        let mut got = false;
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                got = true;
            }
        }
        if got {
            break;
        }
    }

    // Frame packed : [0xD4][u32 len = 1 + compressed][0x97][zlib].
    let compressed = {
        use std::io::Write;
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        enc.write_all(&[0u8; 128]).unwrap();
        enc.finish().unwrap()
    };
    let mut frame = Vec::with_capacity(6 + compressed.len());
    frame.push(mule_protocol::ed2k::PR_EMULE_COMPRESSED);
    frame.extend_from_slice(&((compressed.len() + 1) as u32).to_le_bytes());
    frame.push(mule_protocol::ed2k::OP_PUBLICIP_REQ);
    frame.extend_from_slice(&compressed);
    client.write_all(&frame).await.unwrap();

    let answer = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let n = client.read(&mut raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == mule_protocol::ed2k::PR_EMULE
                    && pkt.opcode == mule_protocol::ed2k::OP_PUBLICIP_ANSWER
                {
                    return pkt.payload;
                }
            }
        }
    })
    .await
    .expect("PUBLICIP_ANSWER attendu pour le paquet packé");
    assert_eq!(&answer[..], &[127, 0, 0, 1]);

    // Un flux zlib corrompu est droppé sans casser la session : un REQ
    // clair envoyé ensuite doit toujours répondre.
    let mut bad = Vec::with_capacity(10);
    bad.push(mule_protocol::ed2k::PR_EMULE_COMPRESSED);
    bad.extend_from_slice(&5u32.to_le_bytes());
    bad.push(mule_protocol::ed2k::OP_PUBLICIP_REQ);
    bad.extend_from_slice(b"junk");
    client.write_all(&bad).await.unwrap();

    let mut frame = Vec::with_capacity(6);
    frame.push(mule_protocol::ed2k::PR_EMULE);
    frame.extend_from_slice(&1u32.to_le_bytes());
    frame.push(mule_protocol::ed2k::OP_PUBLICIP_REQ);
    client.write_all(&frame).await.unwrap();

    let answer2 = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let n = client.read(&mut raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.opcode == mule_protocol::ed2k::OP_PUBLICIP_ANSWER {
                    return pkt.payload;
                }
            }
        }
    })
    .await
    .expect("session saine après paquet packé corrompu");
    assert_eq!(&answer2[..], &[127, 0, 0, 1]);

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await;
}

/// `OP_REASKCALLBACKTCP` (0x9A, PR_EMULE) — eMule `ListenSocket.cpp:1391` :
/// un re-ask LowID relayé par le lien buddy. La réponse part en UDP chez le
/// demandeur (`FILENOTFOUND` si le hash n'est pas partagé). Gate : le pair
/// TCP doit être notre buddy actif.
#[tokio::test]
async fn test_inbound_reaskcallbacktcp_from_buddy_relays_udp_reply() {
    let (state, _dir, _hash) = share_state(
        "test_token_reaskcb",
        "reaskcb_shared.bin",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;

    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let client_local = client.local_addr().unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();

    let hello = Ed2kHello {
        user_hash: [0x55u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client.write_all(&out).await.unwrap();
    loop {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        let mut got = false;
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                got = true;
            }
        }
        if got {
            break;
        }
    }

    // Socket UDP factice = « demandeur » LowID : on attend son datagramme.
    let udp_dest = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let dest_port = udp_dest.local_addr().unwrap().port();

    let mk_frame = |payload: &[u8]| {
        let mut f = Vec::with_capacity(6 + payload.len());
        f.push(mule_protocol::ed2k::PR_EMULE);
        f.extend_from_slice(&((payload.len() as u32 + 1).to_le_bytes()));
        f.push(mule_protocol::ed2k::OP_REASKCALLBACKTCP);
        f.extend_from_slice(payload);
        f
    };
    // [destip 4][destport 2][reqhash 16] — hash inconnu → FILENOTFOUND.
    let mut payload = Vec::with_capacity(22);
    payload.extend_from_slice(&[127, 0, 0, 1]);
    payload.extend_from_slice(&dest_port.to_le_bytes());
    payload.extend_from_slice(&[0xAB; 16]);

    // 1) Sans buddy enregistré → ignoré (pas de datagramme).
    client.write_all(&mk_frame(&payload)).await.unwrap();
    let early =
        tokio::time::timeout(Duration::from_millis(300), udp_dest.recv_from(&mut raw_buf)).await;
    assert!(early.is_err(), "non-buddy REASKCALLBACKTCP must be ignored");

    // 2) Buddy actif dont remote_addr = notre socket → relais. Le canal
    // UDP client a besoin d'un socket réel pour émettre la réponse.
    let client_udp_sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    state
        .client_udp
        .set_socket(std::sync::Arc::new(client_udp_sock))
        .await;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    {
        let reg = state.kad.udp().buddy_registry();
        let mut reg = reg.lock().await;
        assert!(
            reg.set_active(mule_daemon::services::kad_udp::types::ActiveBuddy {
                buddy_id: [0x66; 16],
                user_id: [0x77; 16],
                remote_addr: client_local,
                tx,
            })
        );
    }
    client.write_all(&mk_frame(&payload)).await.unwrap();
    let mut udp_buf = [0u8; 64];
    let (n, _from) = tokio::time::timeout(Duration::from_secs(3), udp_dest.recv_from(&mut udp_buf))
        .await
        .expect("FILENOTFOUND attendu via relais buddy")
        .unwrap();
    assert_eq!(&udp_buf[..n], &[mule_protocol::ed2k::PR_EMULE, 0x92]);

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await;
}

/// `OP_CALLBACK` (0x99, PR_EMULE) — eMule `ListenSocket.cpp:1326` : relais
/// buddy du `KADEMLIA_CALLBACK_REQ` quand nous sommes LowID.
/// `[~kad_id][fileid][ip][tcp]` → le demandeur devient source Kad du
/// fichier (équivalent `TryToConnect`).
#[tokio::test]
async fn test_inbound_callback_registers_requester_as_source() {
    let file_hash = [0x42u8; 16];
    let hex_hash = hex::encode(file_hash);
    let (state, _dir, _hash) = share_state(
        "test_token_callback",
        "callback_shared.bin",
        18874368,
        &shared_bytes(),
        &hex_hash,
    )
    .await;
    let mut authenticated = true;

    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0x0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();

    let hello = Ed2kHello {
        user_hash: [0x44u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client.write_all(&out).await.unwrap();
    loop {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        let mut got = false;
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                got = true;
            }
        }
        if got {
            break;
        }
    }

    // Mauvais check d'abord → ignoré (IP parasite ne doit pas apparaître).
    let mut bad_payload = Vec::with_capacity(38);
    bad_payload.extend_from_slice(&[0u8; 16]);
    bad_payload.extend_from_slice(&file_hash);
    bad_payload.extend_from_slice(&[127, 0, 0, 9]);
    bad_payload.extend_from_slice(&49779u16.to_le_bytes());
    let mut bad = Vec::with_capacity(6 + bad_payload.len());
    bad.push(mule_protocol::ed2k::PR_EMULE);
    bad.extend_from_slice(&((bad_payload.len() as u32 + 1).to_le_bytes()));
    bad.push(mule_protocol::ed2k::OP_CALLBACK);
    bad.extend_from_slice(&bad_payload);
    client.write_all(&bad).await.unwrap();

    // check = ~notre_kad_id (le buddyID publié chez eMule est déjà inversé).
    let our_kad = state.kad.local_id().0;
    let check: Vec<u8> = our_kad.iter().map(|b| !b).collect();
    let mut payload = Vec::with_capacity(38);
    payload.extend_from_slice(&check);
    payload.extend_from_slice(&file_hash);
    payload.extend_from_slice(&[127, 0, 0, 1]);
    payload.extend_from_slice(&49777u16.to_le_bytes());
    let mut frame = Vec::with_capacity(6 + payload.len());
    frame.push(mule_protocol::ed2k::PR_EMULE);
    frame.extend_from_slice(&((payload.len() as u32 + 1).to_le_bytes()));
    frame.push(mule_protocol::ed2k::OP_CALLBACK);
    frame.extend_from_slice(&payload);
    client.write_all(&frame).await.unwrap();

    tokio::time::sleep(Duration::from_millis(400)).await;
    let sources = {
        let dl = state.downloads.lock().await;
        dl.list_peer_sources(&hex_hash).unwrap_or_default()
    };
    assert!(
        sources.iter().any(|s| s.address == "127.0.0.1:49777"),
        "requester inscrit comme source Kad du fichier"
    );
    assert!(
        !sources.iter().any(|s| s.address == "127.0.0.9:49779"),
        "mauvais check doit être ignoré"
    );

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await;
}

/// eMule `SendHashsetPacket(bFileIdentifiers=true)` (`UploadClient.cpp:510`) :
/// `OP_HASHSETREQUEST2` (0xB1, PR_EMULE) `<FileIdentifier><opts u8>` recu ->
/// `OP_HASHSETANSWER2` (0xB2) repletant l'identifiant puis le hashset MD4
/// (`WriteHashSetsToPacket`, `FileIdentifier.cpp:249`). Le bit AICH est omis
/// quand le set par part n'est pas calculable (ici le .part enregistre est
/// tronque vs la taille declaree).
#[tokio::test]
async fn test_inbound_hashsetrequest2_serves_md4_set() {
    let (state, _dir, hash) = share_state(
        "test_token_hs2",
        "hs2_shared.bin",
        18874368,
        &shared_bytes(),
        "31d6cfe0d16ae931b73c59d7e0c089c0",
    )
    .await;
    let mut authenticated = true;
    let file_hash = hex::decode(&hash).unwrap();
    let mut fh = [0u8; 16];
    fh.copy_from_slice(&file_hash);

    // Hashset MD4 en cache -> servi sans rehash disque.
    let part_hashes: Vec<[u8; 16]> = vec![[0x11; 16], [0x22; 16]];
    {
        let mut dl = state.downloads.lock().await;
        dl.store_part_hashes_by_hash(&hash, &part_hashes).unwrap();
    }

    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();

    let hello = Ed2kHello {
        user_hash: [0x66u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client.write_all(&out).await.unwrap();
    loop {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        let mut got = false;
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.opcode == mule_protocol::ed2k::OP_HELLO_ANSWER {
                got = true;
            }
        }
        if got {
            break;
        }
    }

    // OP_HASHSETREQUEST2 : FileIdentifier {md4, taille} + opts MD4|AICH.
    let req = mule_protocol::ed2k::Ed2kHashSetReq2 {
        identifier: mule_protocol::ed2k::Ed2kFileIdentifier::new(fh, 18874368),
        want_md4: true,
        want_aich: true,
    };
    let req_pkt = encoder.encode_hashset_request2(&req).unwrap();
    assert_eq!(req_pkt.protocol, mule_protocol::ed2k::PR_EMULE);
    let mut out = Vec::new();
    out.push(req_pkt.protocol);
    out.extend_from_slice(&(req_pkt.frame_len() as u32).to_le_bytes());
    out.push(req_pkt.opcode);
    out.extend_from_slice(&req_pkt.payload);
    client.write_all(&out).await.unwrap();

    // Attendre OP_HASHSETANSWER2 (0xB2, PR_EMULE).
    let mut answer = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while answer.is_none() && tokio::time::Instant::now() < deadline {
        let n = tokio::time::timeout(Duration::from_secs(5), client.read(&mut raw_buf))
            .await
            .expect("timeout waiting for OP_HASHSETANSWER2")
            .unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == mule_protocol::ed2k::PR_EMULE
                && pkt.opcode == mule_protocol::ed2k::OP_HASHSETANSWER2
            {
                answer = Some(pkt);
            }
        }
    }
    let answer = answer.expect("OP_HASHSETANSWER2 attendu");
    let ans = decoder
        .decode_hashset_answer2(answer.protocol, answer.opcode, &answer.payload, 0)
        .unwrap();
    assert_eq!(ans.identifier.md4_hash, Some(fh));
    assert_eq!(ans.identifier.file_size, Some(18874368));
    let md4_set = ans.md4_set.expect("md4 hashset attendu");
    assert_eq!(md4_set.file_hash, fh);
    assert_eq!(md4_set.part_hashes, part_hashes);
    // L'arbre AICH n'est pas calculable ici -> bit omis comme eMule.
    assert!(ans.aich_set.is_none());

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await;
}
