use std::time::Duration;
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use mule_daemon::rpc::methods::handle_request;
use mule_daemon::rpc::types::JsonRpcRequest;
use mule_daemon::{AppState, DaemonConfig, DownloadService, WorkDirMode};
use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kHello, Ed2kRequestParts, PR_ED2K,
};
use mule_protocol::framing::PacketCodec;

async fn setup_upload_test_state() -> (AppState, tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "test_token_upload".to_string();

    let config =
        DaemonConfig::new(dir.path().to_path_buf(), &token).with_workdir_mode(WorkDirMode::Portable);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);

    let file_hash_hex = "31d6cfe0d16ae931b73c59d7e0c089c0".to_string();

    // 1. Register an explicitly shared file
    let mut authenticated = true;
    let share_req = make_req(
        1,
        "shares.add",
        Some(json!({
            "fileHash": file_hash_hex,
            "fileName": "test_shared_archive.iso",
            "fileSize": 18874368,
            "filePath": "C:\\Shared\\test_shared_archive.iso"
        })),
    );
    let resp = handle_request(&state, share_req, &mut authenticated).await.unwrap();
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
async fn test_inbound_listener_rejected_under_disabled_network() {
    let (state, _dir, _hash) = setup_upload_test_state().await;
    let mut authenticated = true;

    // Disconnect network via kill switch
    let _ = handle_request(&state, make_req(1, "network.disconnectAll", None), &mut authenticated).await.unwrap();

    // Attempt to start listener
    let req = make_req(2, "listener.start", Some(json!({ "port": 0 })));
    let resp = handle_request(&state, req, &mut authenticated).await.unwrap();
    assert!(resp.error.is_some(), "Listener start must fail when network is disabled");
    let err = resp.error.unwrap();
    assert!(err.message.contains("Network is disabled"));
}

#[tokio::test]
async fn test_inbound_handshake_and_request_shared_file_upload_streamed() {
    let (state, _dir, hash) = setup_upload_test_state().await;
    let mut authenticated = true;

    // Start listener on ephemeral loopback port
    let start_req = make_req(1, "listener.start", Some(json!({ "port": 0 })));
    let start_resp = handle_request(&state, start_req, &mut authenticated).await.unwrap();
    assert!(start_resp.error.is_none());
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"].as_str().unwrap().to_string();

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
    let status_req = mule_protocol::ed2k::Ed2kFileStatusReq { file_hash: hash_bytes };
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
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange { start: 0, end: 184320 }],
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
                let part = decoder.decode_sending_part(pkt.protocol, pkt.opcode, &pkt.payload, 0).unwrap();
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
    let status_resp = handle_request(&state, status_req, &mut authenticated).await.unwrap();
    let upload_snap: Value = status_resp.result.unwrap();
    assert_eq!(upload_snap["state"], "uploading");
    assert!(upload_snap["transferredBytes"].as_u64().unwrap() >= 184320);

    // Stop listener
    let stop_req = make_req(3, "listener.stop", None);
    let _ = handle_request(&state, stop_req, &mut authenticated).await.unwrap();
}

#[tokio::test]
async fn test_inbound_request_unshared_file_does_not_upload() {
    let (state, _dir, _hash) = setup_upload_test_state().await;
    let mut authenticated = true;

    let start_resp = handle_request(&state, make_req(1, "listener.start", Some(json!({ "port": 0 }))), &mut authenticated).await.unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"].as_str().unwrap().to_string();

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
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange { start: 0, end: 184320 }],
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
    let status_resp = handle_request(&state, make_req(2, "uploads.status", None), &mut authenticated).await.unwrap();
    assert!(status_resp.result.unwrap().is_null(), "Unshared file request must not allocate upload slot");

    let _ = handle_request(&state, make_req(3, "listener.stop", None), &mut authenticated).await.unwrap();
}

#[tokio::test]
async fn test_upload_queue_ranking_and_single_slot_concurrency() {
    let (state, _dir, hash) = setup_upload_test_state().await;
    let mut authenticated = true;

    // Start listener
    let start_resp = handle_request(&state, make_req(1, "listener.start", Some(json!({ "port": 0 }))), &mut authenticated).await.unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"].as_str().unwrap().to_string();

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
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange { start: 0, end: 184320 }],
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
        tcp_port: 4663,
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
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange { start: 0, end: 184320 }],
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
            if pkt.protocol == PR_ED2K && pkt.opcode == mule_protocol::ed2k::OP_QUEUERANKING {
                let rank = u32::from_le_bytes(pkt.payload[0..4].try_into().unwrap());
                assert_eq!(rank, 1, "Client 2 should have queue rank 1");
                received_queue_ranking = true;
                break;
            }
        }
    }

    // Verify uploads.list RPC returns active upload and queued upload
    let list_resp = handle_request(&state, make_req(2, "uploads.list", None), &mut authenticated).await.unwrap();
    let uploads_list: Value = list_resp.result.unwrap();
    assert_eq!(uploads_list["active"].as_array().unwrap().len(), 1);
    assert_eq!(uploads_list["queued"].as_array().unwrap().len(), 1);

    let _ = handle_request(&state, make_req(3, "listener.stop", None), &mut authenticated).await.unwrap();
}

#[tokio::test]
async fn test_kill_switch_aborts_inbound_listener_and_active_uploads() {
    let (state, _dir, _hash) = setup_upload_test_state().await;
    let mut authenticated = true;

    // Start listener
    let start_resp = handle_request(&state, make_req(1, "listener.start", Some(json!({ "port": 0 }))), &mut authenticated).await.unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"].as_str().unwrap().to_string();

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
    let kill_resp = handle_request(&state, make_req(2, "network.disconnectAll", None), &mut authenticated).await.unwrap();
    assert!(kill_resp.error.is_none());

    // Verify listener is now stopped
    tokio::time::sleep(Duration::from_millis(50)).await;
    let status_resp = handle_request(&state, make_req(3, "listener.status", None), &mut authenticated).await.unwrap();
    let status_val: Value = status_resp.result.unwrap();
    assert_eq!(status_val["isListening"], false);
    assert_eq!(status_val["activeUploads"], 0);

    // Socket should receive EOF on read
    let n = client.read(&mut raw_buf).await.unwrap_or(0);
    assert_eq!(n, 0, "Socket must be closed when kill switch triggers");
}

#[tokio::test]
async fn test_inbound_handshake_returns_real_identity_and_listen_port() {
    let (state, _dir, _hash) = setup_upload_test_state().await;
    let mut authenticated = true;

    // Start listener
    let start_resp = handle_request(&state, make_req(1, "listener.start", Some(json!({ "port": 0 }))), &mut authenticated).await.unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"].as_str().unwrap().to_string();
    let listen_port: u16 = listen_addr_str.split(':').next_back().unwrap().parse().unwrap();

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
                let ans = decoder.decode_hello_answer(pkt.protocol, pkt.opcode, &pkt.payload, 0).unwrap();
                ans_opt = Some(ans);
                break;
            }
        }
    }

    let ans = ans_opt.unwrap();
    assert_eq!(ans.user_hash, expected_hash, "Ed2kHelloAnswer must use real identity user_hash");
    assert_eq!(ans.tcp_port, listen_port, "Ed2kHelloAnswer must use actual listener port");
}

#[tokio::test]
async fn test_inbound_connection_per_ip_limit() {
    let (state, _dir, _hash) = setup_upload_test_state().await;
    let mut authenticated = true;

    // Start listener
    let start_resp = handle_request(&state, make_req(1, "listener.start", Some(json!({ "port": 0 }))), &mut authenticated).await.unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"].as_str().unwrap().to_string();

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
    assert!(n > 0, "New connection should be accepted after dropping an existing client");
}

#[tokio::test]
async fn test_inbound_secident_signature_without_challenge_rejected() {
    let (state, _dir, _hash) = setup_upload_test_state().await;
    let mut authenticated = true;

    // Start listener
    let start_resp = handle_request(&state, make_req(1, "listener.start", Some(json!({ "port": 0 }))), &mut authenticated).await.unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"].as_str().unwrap().to_string();

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
            if pkt.protocol == mule_protocol::ed2k::PR_EMULE && pkt.opcode == mule_protocol::ed2k::OP_SECIDENTSTATE {
                let st = decoder.decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0).unwrap();
                state_opt = Some(st);
                break;
            }
        }
    }

    let st = state_opt.unwrap();
    assert_eq!(st.state, 0, "Unsolicited signature without prior challenge must be rejected with state 0");
}
