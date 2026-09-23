// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use bytes::Bytes;
use serde_json::{json, Value};
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use mule_daemon::rpc::methods::handle_request;
use mule_daemon::rpc::types::JsonRpcRequest;
use mule_daemon::{AppState, DaemonConfig, DownloadService, WorkDirMode};
use mule_protocol::ed2k::{
    Ed2kEncoder, Ed2kFileStatus, Ed2kHelloAnswer, Ed2kSendingPart, PartAvailabilityMask,
    OP_ACCEPTUPLOADREQ, OP_FILESTATUSREQ, OP_HELLO, OP_REQUESTPARTS, OP_REQUESTPARTS_EMULE,
    OP_STARTUPLOADREQ, PR_ED2K,
};
use mule_protocol::framing::PacketCodec;

async fn setup_test_state() -> (AppState, tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "test_token_adversarial".to_string();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);
    state
        .network_policy
        .set_profile(mule_network_policy::NetworkProfile::LocalLab);

    let file_hash_hex = "31d6cfe0d16ae931b73c59d7e0c089c0".to_string();

    // Insert download via RPC downloads.addEd2kLink
    let link = format!("ed2k://|file|test_payload.iso|9728001|{file_hash_hex}|/");
    let req = make_req(1, "downloads.addEd2kLink", Some(json!({ "link": link })));
    let mut authenticated = true;
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    (state, dir, file_hash_hex)
}

fn make_req(id: u64, method: &str, params: Option<Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(id)),
        method: method.to_string(),
        params,
    }
}

#[tokio::test]
async fn test_adversarial_socket_open_then_complete_silence_times_out() {
    let (state, _dir, hash) = setup_test_state().await;
    let mut authenticated = true;

    // Start a mock listener that accepts and never responds
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();

    let _mock_peer = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 128];
        let _ = socket.read(&mut buf).await;
        // Never send anything back
        tokio::time::sleep(Duration::from_secs(15)).await;
    });

    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    // Wait until handshake timeout occurs (8s) + a small margin
    tokio::time::sleep(Duration::from_millis(8500)).await;

    let req = make_req(2, "sources.status", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let status_obj: Value = resp.result.unwrap();
    assert_eq!(status_obj["status"], "disconnected");
    assert_eq!(status_obj["failureReason"], "timeout_handshake");

    // Check that source in registry is marked Failed with fail_count = 1 and cooldown set
    let req = make_req(3, "sources.list", Some(json!({ "fileHash": hash })));
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let list: Vec<Value> = serde_json::from_value(resp.result.unwrap()).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["status"], "failed");
    assert_eq!(list[0]["failCount"], 1);
    assert!(list[0]["cooldownUntilUnixMs"].is_number());
}

#[tokio::test]
async fn test_adversarial_cooldown_rejection_and_force_bypass() {
    let (state, _dir, hash) = setup_test_state().await;
    let mut authenticated = true;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();

    // 1. Manually add source and set it into failed with cooldown
    {
        let mut dl_svc = state.downloads.lock().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let src = mule_core::model::PeerSource {
            file_hash: hash.clone(),
            address: peer_addr.to_string(),
            ip: 2130706433,
            port: peer_addr.port(),
            origin: mule_core::model::PeerSourceOrigin::Manual,
            discovered_at_unix_ms: now,
            last_seen_unix_ms: now,
            status: mule_core::model::PeerSourceStatus::Failed,
            fail_count: 2,
            last_error: Some("Timeout connect".to_string()),
            cooldown_until_unix_ms: Some(now + 60_000), // 60s cooldown
            user_hash: None,
            crypt_options: 0,
            buddy_id: None,
            software: None,
            availability: None,
        };
        let _ = dl_svc.upsert_peer_sources(&hash, &[src]);
    }

    // 2. Attempt connect without force -> must be rejected by cooldown
    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_some());
    let err = resp.error.unwrap();
    assert!(err.message.contains("Source is in cooldown"));

    // 3. Connect with force: true -> must bypass cooldown and proceed
    let req = make_req(
        2,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr.to_string(), "force": true })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_none(),
        "Forced reconnect must bypass cooldown"
    );

    let _ = state.peer_session.disconnect().await;
}

#[tokio::test]
async fn test_adversarial_malformed_hello_answer_fails_handshake() {
    let (state, _dir, hash) = setup_test_state().await;
    let mut authenticated = true;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();

    let _mock_peer = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 512];
        let _ = socket.read(&mut buf).await.unwrap();

        // Send a malformed OP_HELLO_ANSWER with truncated payload (less than 16 bytes for user hash)
        let corrupt_payload = vec![0x11, 0x22, 0x33]; // Invalid length
        let mut out = Vec::new();
        out.push(PR_ED2K);
        out.extend_from_slice(&(1u32 + corrupt_payload.len() as u32).to_le_bytes());
        out.push(mule_protocol::ed2k::OP_HELLO_ANSWER);
        out.extend_from_slice(&corrupt_payload);

        let _ = socket.write_all(&out).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
    });

    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    tokio::time::sleep(Duration::from_millis(200)).await;

    let req = make_req(2, "sources.status", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let status_obj: Value = resp.result.unwrap();
    assert_eq!(status_obj["status"], "disconnected");
    assert_eq!(status_obj["failureReason"], "invalid_handshake");
}

#[tokio::test]
async fn test_adversarial_file_status_hash_mismatch() {
    let (state, _dir, hash) = setup_test_state().await;
    let mut authenticated = true;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();

    let _mock_peer = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut read_buf = bytes::BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let encoder = Ed2kEncoder::default();

        // 1. Read OP_HELLO
        let mut received_hello = false;
        while !received_hello {
            let n = socket.read(&mut raw_buf).await.unwrap();
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
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && (pkt.opcode == OP_FILESTATUSREQ || pkt.opcode == 0x4F)
                {
                    received_status_req = true;
                    break;
                }
            }
        }

        // Send OP_FILESTATUS with WRONG HASH (different from requested)
        let wrong_hash: [u8; 16] = [0xEEu8; 16];
        let file_status = Ed2kFileStatus {
            file_hash: wrong_hash,
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

        tokio::time::sleep(Duration::from_millis(500)).await;
    });

    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    tokio::time::sleep(Duration::from_millis(200)).await;

    let req = make_req(2, "sources.status", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let status_obj: Value = resp.result.unwrap();
    assert_eq!(status_obj["status"], "disconnected");
    assert_eq!(status_obj["failureReason"], "hash_mismatch");
}

#[tokio::test]
async fn test_adversarial_sending_part_out_of_bounds_range_rejected() {
    let (state, _dir, hash) = setup_test_state().await;
    let mut authenticated = true;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();

    let hash_clone = hash.clone();
    let _mock_peer = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut read_buf = bytes::BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let encoder = Ed2kEncoder::default();

        // 1. Read OP_HELLO
        let mut received_hello = false;
        while !received_hello {
            let n = socket.read(&mut raw_buf).await.unwrap();
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
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && (pkt.opcode == OP_FILESTATUSREQ || pkt.opcode == 0x4F)
                {
                    received_status_req = true;
                    break;
                }
            }
        }

        // Send OP_FILESTATUS
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

        // Send OP_SENDINGPART with OUT OF BOUNDS offset (e.g. 500_000 > 184320)
        let dummy_chunk = vec![0xABu8; 1024];
        let sending_part =
            Ed2kSendingPart::new(hash_bytes, 500_000, 501_024, Bytes::from(dummy_chunk));
        let send_pkt = encoder.encode_sending_part(&sending_part).unwrap();
        out.clear();
        out.push(send_pkt.protocol);
        out.extend_from_slice(&(send_pkt.frame_len() as u32).to_le_bytes());
        out.push(send_pkt.opcode);
        out.extend_from_slice(&send_pkt.payload);
        socket.write_all(&out).await.unwrap();

        tokio::time::sleep(Duration::from_millis(500)).await;
    });

    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    tokio::time::sleep(Duration::from_millis(200)).await;

    let req = make_req(2, "sources.status", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let status_obj: Value = resp.result.unwrap();
    assert_eq!(status_obj["status"], "disconnected");
    assert_eq!(status_obj["failureReason"], "invalid_range");
}

#[tokio::test]
async fn test_adversarial_unexpected_disconnect_during_transfer() {
    let (state, _dir, hash) = setup_test_state().await;
    let mut authenticated = true;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();

    let hash_clone = hash.clone();
    let _mock_peer = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut read_buf = bytes::BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let encoder = Ed2kEncoder::default();

        // 1. Read OP_HELLO & Send OP_HELLO_ANSWER
        let mut received_hello = false;
        while !received_hello {
            let n = socket.read(&mut raw_buf).await.unwrap();
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO {
                    received_hello = true;
                    break;
                }
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
        let ans_pkt = encoder.encode_hello_answer(&hello_answer).unwrap();
        let mut out = Vec::new();
        out.push(ans_pkt.protocol);
        out.extend_from_slice(&(ans_pkt.frame_len() as u32).to_le_bytes());
        out.push(ans_pkt.opcode);
        out.extend_from_slice(&ans_pkt.payload);
        socket.write_all(&out).await.unwrap();

        // 2. Read OP_FILESTATUSREQ & Send OP_FILESTATUS
        let mut received_status_req = false;
        while !received_status_req {
            let n = socket.read(&mut raw_buf).await.unwrap();
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && (pkt.opcode == OP_FILESTATUSREQ || pkt.opcode == 0x4F)
                {
                    received_status_req = true;
                    break;
                }
            }
        }

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

        // Send 1st valid chunk then abruptly drop socket (EOF)
        let dummy_chunk = vec![0xABu8; 512];
        let sending_part = Ed2kSendingPart::new(hash_bytes, 0, 512, Bytes::from(dummy_chunk));
        let send_pkt = encoder.encode_sending_part(&sending_part).unwrap();
        out.clear();
        out.push(send_pkt.protocol);
        out.extend_from_slice(&(send_pkt.frame_len() as u32).to_le_bytes());
        out.push(send_pkt.opcode);
        out.extend_from_slice(&send_pkt.payload);
        socket.write_all(&out).await.unwrap();

        // Close socket immediately
        drop(socket);
    });

    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    tokio::time::sleep(Duration::from_millis(200)).await;

    let req = make_req(2, "sources.status", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let status_obj: Value = resp.result.unwrap();
    assert_eq!(status_obj["status"], "disconnected");

    // Permit must be released cleanly and state can accept a new connection
    assert!(!state.peer_session.is_active().await);
}
