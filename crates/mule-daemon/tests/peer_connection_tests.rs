// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use bytes::Bytes;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use mule_daemon::rpc::methods::handle_request;
use mule_protocol::ed2k::{
    Ed2kEncoder, Ed2kFileStatus, Ed2kHelloAnswer, Ed2kSendingPart, PartAvailabilityMask,
    OP_ACCEPTUPLOADREQ, OP_FILESTATUSREQ, OP_HELLO, OP_REQUESTPARTS, OP_REQUESTPARTS_EMULE,
    OP_STARTUPLOADREQ, PR_ED2K,
};
use mule_protocol::framing::PacketCodec;
use mule_test_support::{download_state, make_req};

#[tokio::test]
async fn test_sources_list_and_add_manual() {
    let (state, _dir, hash) = download_state("test_token_peer", "test_payload.iso", 9728001, "31d6cfe0d16ae931b73c59d7e0c089c0").await;
    let mut authenticated = true;

    // 1. Initial list should be empty
    let req = make_req(1, "sources.list", Some(json!({ "fileHash": hash })));
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let list: Vec<Value> = serde_json::from_value(resp.result.unwrap()).unwrap();
    assert_eq!(list.len(), 0);

    // 2. Add manual source
    let req = make_req(
        2,
        "sources.addManual",
        Some(json!({ "fileHash": hash, "address": "127.0.0.1:39735" })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let added: Value = resp.result.unwrap();
    assert_eq!(added["address"], "127.0.0.1:39735");
    assert_eq!(added["origin"], "manual");

    // 3. Verify in list
    let req = make_req(3, "sources.list", Some(json!({ "fileHash": hash })));
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let list: Vec<Value> = serde_json::from_value(resp.result.unwrap()).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["address"], "127.0.0.1:39735");
}

#[tokio::test]
async fn test_peer_connect_refused_when_ip_filtered() {
    let (state, _dir, hash) = download_state("test_token_peer", "test_payload.iso", 9728001, "31d6cfe0d16ae931b73c59d7e0c089c0").await;
    let mut authenticated = true;

    // Load an IP filter rule blocking 127.0.0.1
    let filter = state.network_policy.ip_filter();
    let report = filter
        .load_from_str("127.0.0.1 - 127.0.0.1 , 000 , Blocked Loopback")
        .unwrap();
    assert_eq!(report.valid_entries_parsed, 1);

    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": "127.0.0.1:39735" })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_some(), "Must be blocked by IP filter");
    let err = resp.error.unwrap();
    assert!(err.message.contains("blocked by IP filter"));

    // Check that source status in registry is Filtered
    let req = make_req(2, "sources.list", Some(json!({ "fileHash": hash })));
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let list: Vec<Value> = serde_json::from_value(resp.result.unwrap()).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["status"], "filtered");
}

#[tokio::test]
async fn test_peer_connect_refused_under_local_lab_profile() {
    let (state, _dir, hash) = download_state("test_token_peer", "test_payload.iso", 9728001, "31d6cfe0d16ae931b73c59d7e0c089c0").await;
    let mut authenticated = true;

    // Default profile is LocalLab. Connecting to non-loopback IP must be rejected.
    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": "91.121.162.155:39735" })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_some(),
        "Must be rejected under LocalLab profile"
    );
    let err = resp.error.unwrap();
    assert!(err.message.contains("rejected by network policy"));
}

#[tokio::test]
async fn test_peer_connect_nominal_handshake_and_transfer() {
    let (state, _dir, hash) = download_state("test_token_peer", "test_payload.iso", 9728001, "31d6cfe0d16ae931b73c59d7e0c089c0").await;
    let mut authenticated = true;

    // Start mock eD2K peer TCP server on 127.0.0.1
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();

    let hash_clone = hash.clone();
    let mock_peer = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut read_buf = bytes::BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let encoder = Ed2kEncoder::default();

        // 1. Read OP_HELLO
        let mut received_hello = false;
        while !received_hello {
            let n = socket.read(&mut raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO {
                    received_hello = true;
                    break;
                }
            }
        }

        // Send OP_HELLO_ANSWER
        let hello_answer = Ed2kHelloAnswer {
            user_hash: [0x55u8; 16],
            client_id: 0x0100007F,
            tcp_port: peer_addr.port(),
            tags: vec![],
            unknown_tags: vec![],
            server_ip: 0,
            server_port: 0,
        };
        let ans_pkt = encoder.encode_hello_answer(&hello_answer).unwrap();
        let mut out = Vec::new();
        out.push(ans_pkt.protocol);
        out.extend_from_slice(&(ans_pkt.frame_len() as u32).to_le_bytes());
        out.push(ans_pkt.opcode);
        out.extend_from_slice(&ans_pkt.payload);
        socket.write_all(&out).await.unwrap();

        // 2. Read OP_FILESTATUSREQ
        let mut received_status_req = false;
        while !received_status_req {
            let n = socket.read(&mut raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && (pkt.opcode == OP_FILESTATUSREQ || pkt.opcode == 0x4F)
                {
                    received_status_req = true;
                    break;
                }
            }
        }

        // Send OP_FILESTATUS (part_count = 2, all parts available [0, 1])
        let hash_bytes: [u8; 16] = hex::decode(&hash_clone).unwrap().try_into().unwrap();
        let file_status = Ed2kFileStatus {
            file_hash: hash_bytes,
            part_count: 2,
            availability_mask: PartAvailabilityMask::from_parts(&[0, 1], 2),
        };
        let status_pkt = encoder.encode_file_status(&file_status).unwrap();
        out.clear();
        out.push(status_pkt.protocol);
        out.extend_from_slice(&(status_pkt.frame_len() as u32).to_le_bytes());
        out.push(status_pkt.opcode);
        out.extend_from_slice(&status_pkt.payload);
        socket.write_all(&out).await.unwrap();

        // 2b. Read OP_STARTUPLOADREQ and reply with OP_ACCEPTUPLOADREQ
        let mut received_start_upload = false;
        while !received_start_upload {
            let n = socket.read(&mut raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && pkt.opcode == OP_STARTUPLOADREQ {
                    received_start_upload = true;
                    break;
                }
            }
        }

        let accept_pkt =
            mule_protocol::framing::RawPacket::new(PR_ED2K, OP_ACCEPTUPLOADREQ, Bytes::new());
        out.clear();
        out.push(accept_pkt.protocol);
        out.extend_from_slice(&(accept_pkt.frame_len() as u32).to_le_bytes());
        out.push(accept_pkt.opcode);
        let _ = socket.write_all(&out).await;

        // 3. Read OP_REQUESTPARTS
        let mut received_req_parts = false;
        while !received_req_parts {
            let n = socket.read(&mut raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K
                    && (pkt.opcode == OP_REQUESTPARTS || pkt.opcode == OP_REQUESTPARTS_EMULE)
                {
                    received_req_parts = true;
                    break;
                }
            }
        }

        // Send OP_SENDINGPART (1024 dummy bytes)
        let dummy_chunk = vec![0xABu8; 1024];
        let sending_part = Ed2kSendingPart::new(hash_bytes, 0, 1024, Bytes::from(dummy_chunk));
        let send_pkt = encoder.encode_sending_part(&sending_part).unwrap();
        out.clear();
        out.push(send_pkt.protocol);
        out.extend_from_slice(&(send_pkt.frame_len() as u32).to_le_bytes());
        out.push(send_pkt.opcode);
        out.extend_from_slice(&send_pkt.payload);
        socket.write_all(&out).await.unwrap();

        // Keep connection open so the client is in transferring state
        tokio::time::sleep(Duration::from_millis(500)).await;
    });

    // Connect via RPC
    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none(), "RPC sources.connect should succeed");

    // Wait a short moment for transfer & status update
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check status via RPC
    let req = make_req(2, "sources.status", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let status_obj: Value = resp.result.unwrap();
    assert!(
        status_obj["status"] == "transferring"
            || status_obj["transferredBytes"].as_u64().unwrap_or(0) > 0,
        "Status should reflect active or completed block transfer: {:?}",
        status_obj
    );

    // Disconnect explicitly
    let req = make_req(3, "sources.disconnect", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let disconnected_obj: Value = resp.result.unwrap();
    assert_eq!(disconnected_obj["status"], "disconnected");

    mock_peer.await.unwrap();
}

/// Socket-level A4AF (eMule `SwapToAnotherFile`/`DoSwap`) : quand le pair
/// répond `OP_FILEREQANSNOFILE` pour le fichier demandé, la session ne doit
/// PAS se fermer — le même socket re-émet la séquence de requête fichier
/// pour un autre download dont ce pair est source.
#[tokio::test]
async fn test_peer_connect_a4af_socket_swap_on_fnf() {
    let hash_a = "11111111111111111111111111111111";
    let hash_b = "22222222222222222222222222222222";
    let (state, _dir, _) =
        download_state("test_token_a4af", "file_a.iso", 9728001, hash_a).await;
    let mut authenticated = true;

    // Second download = swap candidate.
    let link_b = format!("ed2k://|file|file_b.iso|9728001|{hash_b}|/");
    let req = make_req(0, "downloads.addEd2kLink", Some(json!({ "link": link_b })));
    let resp = handle_request(&state, req, &mut authenticated).await.unwrap();
    assert!(resp.error.is_none());

    // Mock eD2K peer; registered as a source for file B only.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();
    let req = make_req(
        1,
        "sources.addManual",
        Some(json!({ "fileHash": hash_b, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated).await.unwrap();
    assert!(resp.error.is_none());

    let ha_bytes: [u8; 16] = hex::decode(hash_a).unwrap().try_into().unwrap();
    let hb_bytes: [u8; 16] = hex::decode(hash_b).unwrap().try_into().unwrap();
    let swapped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let swapped_mock = swapped.clone();

    let mock_peer = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut read_buf = bytes::BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let encoder = Ed2kEncoder::default();
        let mut sent_fnf_a = false;
        let mut sent_status_b = false;
        let mut sent_part = false;

        async fn send_pkt(socket: &mut tokio::net::TcpStream, pkt: mule_protocol::framing::RawPacket) {
            let mut out = Vec::with_capacity(pkt.payload.len() + 6);
            out.push(pkt.protocol);
            out.extend_from_slice(&(pkt.frame_len() as u32).to_le_bytes());
            out.push(pkt.opcode);
            out.extend_from_slice(&pkt.payload);
            socket.write_all(&out).await.unwrap();
        }

        // 1. OP_HELLO → OP_HELLOANSWER
        loop {
            let n = socket.read(&mut raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            let mut got_hello = false;
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO {
                    got_hello = true;
                }
            }
            if got_hello {
                break;
            }
        }
        let hello_answer = Ed2kHelloAnswer {
            user_hash: [0x55u8; 16],
            client_id: 0x0100007F,
            tcp_port: peer_addr.port(),
            tags: vec![],
            unknown_tags: vec![],
            server_ip: 0,
            server_port: 0,
        };
        send_pkt(&mut socket, encoder.encode_hello_answer(&hello_answer).unwrap()).await;

        // 2. Request flow: FNF for A, full service for B on the same socket.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol != PR_ED2K {
                    continue;
                }
                match pkt.opcode {
                    mule_protocol::ed2k::OP_REQFILENAME
                    | mule_protocol::ed2k::OP_SETREQFILEID_EMULE => {
                        if pkt.payload.len() < 16 {
                            continue;
                        }
                        if pkt.payload[..16] == ha_bytes[..] {
                            if pkt.opcode == mule_protocol::ed2k::OP_REQFILENAME && !sent_fnf_a {
                                send_pkt(
                                    &mut socket,
                                    mule_protocol::framing::RawPacket::new(
                                        PR_ED2K,
                                        mule_protocol::ed2k::OP_FILEREQANSNOFILE,
                                        Bytes::copy_from_slice(&ha_bytes),
                                    ),
                                )
                                .await;
                                sent_fnf_a = true;
                            }
                        } else if pkt.payload[..16] == hb_bytes[..] {
                            if pkt.opcode == mule_protocol::ed2k::OP_REQFILENAME {
                                swapped_mock
                                    .store(true, std::sync::atomic::Ordering::SeqCst);
                            } else if !sent_status_b {
                                let fs = Ed2kFileStatus {
                                    file_hash: hb_bytes,
                                    part_count: 2,
                                    availability_mask: PartAvailabilityMask::from_parts(
                                        &[0, 1], 2,
                                    ),
                                };
                                send_pkt(&mut socket, encoder.encode_file_status(&fs).unwrap())
                                    .await;
                                sent_status_b = true;
                            }
                        }
                    }
                    OP_STARTUPLOADREQ => {
                        send_pkt(
                            &mut socket,
                            mule_protocol::framing::RawPacket::new(
                                PR_ED2K,
                                OP_ACCEPTUPLOADREQ,
                                Bytes::new(),
                            ),
                        )
                        .await;
                    }
                    OP_REQUESTPARTS | OP_REQUESTPARTS_EMULE
                        if !sent_part =>
                    {
                        let part = Ed2kSendingPart::new(
                            hb_bytes,
                            0,
                            1024,
                            Bytes::from(vec![0xCDu8; 1024]),
                        );
                        send_pkt(&mut socket, encoder.encode_sending_part(&part).unwrap())
                            .await;
                        sent_part = true;
                    }
                    _ => {}
                }
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            let n = match tokio::time::timeout(
                deadline - tokio::time::Instant::now(),
                socket.read(&mut raw_buf),
            )
            .await
            {
                Ok(Ok(0)) => break, // session closed
                Ok(Ok(n)) => n,
                _ => break,
            };
            read_buf.extend_from_slice(&raw_buf[..n]);
        }
    });

    // Connect for file A — peer FNF's it; the session must swap to file B.
    let req = make_req(
        2,
        "sources.connect",
        Some(json!({ "fileHash": hash_a, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none(), "sources.connect failed: {:?}", resp.error);

    // Wait for the swap + transfer to complete (mock ends by deadline).
    tokio::time::timeout(Duration::from_secs(15), mock_peer)
        .await
        .expect("mock peer timed out")
        .unwrap();

    assert!(
        swapped.load(std::sync::atomic::Ordering::SeqCst),
        "client must re-request file B on the same socket (A4AF swap)"
    );

    // File A source is deadlisted (FNF → Rejected) — the session outcome was
    // written on file B, so A's bookkeeping must survive.
    let req = make_req(3, "sources.list", Some(json!({ "fileHash": hash_a })));
    let resp = handle_request(&state, req, &mut authenticated).await.unwrap();
    let list_a: Vec<Value> = serde_json::from_value(resp.result.unwrap()).unwrap();
    let src_a = list_a
        .iter()
        .find(|s| s["address"] == peer_addr.to_string())
        .expect("source for file A must exist");
    assert_eq!(src_a["status"], "rejected", "FNF must deadlist the source on file A");

    // File B source was actively served on the swapped socket.
    let req = make_req(4, "sources.list", Some(json!({ "fileHash": hash_b })));
    let resp = handle_request(&state, req, &mut authenticated).await.unwrap();
    let list_b: Vec<Value> = serde_json::from_value(resp.result.unwrap()).unwrap();
    assert!(
        list_b.iter().any(|s| s["address"] == peer_addr.to_string()),
        "peer must remain a source for file B"
    );

    // Real bytes flowed on the swapped socket and landed in file B.
    let req = make_req(5, "downloads.list", None);
    let resp = handle_request(&state, req, &mut authenticated).await.unwrap();
    let result = resp.result.unwrap();
    let dl_b = result["downloads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["ed2kHash"].as_str().unwrap_or("") == hash_b)
        .cloned()
        .expect("file B download must exist");
    assert!(
        dl_b["receivedBytes"].as_u64().unwrap_or(0) >= 1024,
        "post-swap transfer must have delivered bytes into file B: {:?}",
        dl_b
    );
}

#[tokio::test]
async fn test_peer_connect_swarm_peer_limit() {
    let (state, _dir, hash) = download_state("test_token_peer", "test_payload.iso", 9728001, "31d6cfe0d16ae931b73c59d7e0c089c0").await;
    let mut authenticated = true;
    state.peer_session.set_swarm_limits(2, 4).await;

    // Start 2 mock listeners that accept and hold open
    let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr1 = listener1.local_addr().unwrap();

    let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr2 = listener2.local_addr().unwrap();

    let _mock_peer1 = tokio::spawn(async move {
        let (mut socket, _) = listener1.accept().await.unwrap();
        let mut buf = [0u8; 128];
        let _ = socket.read(&mut buf).await;
        tokio::time::sleep(Duration::from_secs(5)).await;
    });

    let _mock_peer2 = tokio::spawn(async move {
        let (mut socket, _) = listener2.accept().await.unwrap();
        let mut buf = [0u8; 128];
        let _ = socket.read(&mut buf).await;
        tokio::time::sleep(Duration::from_secs(5)).await;
    });

    // 1. Connect first peer
    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr1.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    // 2. Connect second peer -> Must succeed (limit is 2)
    let req = make_req(
        2,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr2.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    // 3. Try to connect third peer while 2 are active -> Must fail with limit reached error
    let req = make_req(
        3,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": "127.0.0.1:9999" })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_some(), "3rd peer connection must be rejected");
    let err = resp.error.unwrap();
    assert!(err
        .message
        .contains("Maximum concurrent peer connections reached for this download"));

    // 4. Disconnect all
    let req = make_req(4, "sources.disconnect", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
}

#[tokio::test]
async fn test_peer_connect_aborted_by_kill_switch() {
    let (state, _dir, hash) = download_state("test_token_peer", "test_payload.iso", 9728001, "31d6cfe0d16ae931b73c59d7e0c089c0").await;
    let mut authenticated = true;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();

    let _mock_peer = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 128];
        let _ = socket.read(&mut buf).await;
        tokio::time::sleep(Duration::from_secs(5)).await;
    });

    // 1. Connect peer
    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    // 2. Trigger Kill Switch: network.disconnectAll
    let req = make_req(2, "network.disconnectAll", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    // 3. Verify peer session is aborted and disconnected
    tokio::time::sleep(Duration::from_millis(50)).await;
    let req = make_req(3, "sources.status", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let status_obj: Value = resp.result.unwrap();
    assert_eq!(status_obj["status"], "disconnected");
}
