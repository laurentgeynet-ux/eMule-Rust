// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

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
    let token = "test_token_peer".to_string();

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
async fn test_sources_list_and_add_manual() {
    let (state, _dir, hash) = setup_test_state().await;
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
    let (state, _dir, hash) = setup_test_state().await;
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
    let (state, _dir, hash) = setup_test_state().await;
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
    let (state, _dir, hash) = setup_test_state().await;
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

#[tokio::test]
async fn test_peer_connect_swarm_peer_limit() {
    let (state, _dir, hash) = setup_test_state().await;
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
    let (state, _dir, hash) = setup_test_state().await;
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
