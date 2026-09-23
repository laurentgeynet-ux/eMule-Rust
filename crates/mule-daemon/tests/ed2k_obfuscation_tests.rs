// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use serde_json::{json, Value};
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use mule_daemon::rpc::methods::handle_request;
use mule_daemon::rpc::types::JsonRpcRequest;
use mule_daemon::services::obfuscation_stream::ObfuscatedStream;
use mule_daemon::{AppState, DaemonConfig, DownloadService, WorkDirMode};
use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kHello, Ed2kHelloAnswer, OP_HELLO, OP_HELLOANSWER, PR_ED2K,
};
use mule_protocol::framing::PacketCodec;

fn make_req(id: u64, method: &str, params: Option<Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(id)),
        method: method.to_string(),
        params,
    }
}

async fn setup_obfu_test_state(
    obfu_enabled: bool,
    obfu_required: bool,
) -> (AppState, tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "test_token_obfu".to_string();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable)
        .with_obfuscation(obfu_enabled, obfu_required);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);
    state
        .network_policy
        .set_profile(mule_network_policy::NetworkProfile::LocalLab);

    let file_hash_hex = "31d6cfe0d16ae931b73c59d7e0c089c0".to_string();

    // Register a download so peer sessions have a target
    let link = format!("ed2k://|file|test_obfu_file.iso|9728001|{file_hash_hex}|/");
    let req = make_req(1, "downloads.addEd2kLink", Some(json!({ "link": link })));
    let mut authenticated = true;
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    (state, dir, file_hash_hex)
}

#[tokio::test]
async fn test_outgoing_connection_to_strict_obfuscated_peer_succeeds() {
    let (state, _dir, hash) = setup_obfu_test_state(true, false).await;
    let mut authenticated = true;

    // Strict mock peer listening on 127.0.0.1
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();
    let mock_peer_user_hash = [0xAAu8; 16];

    let mock_peer_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        // 1. Read first byte
        let mut first_byte = [0u8; 1];
        socket.read_exact(&mut first_byte).await.unwrap();

        // Strict behavior: if remote client connects with plaintext 0xE3, drop immediately!
        if first_byte[0] == PR_ED2K {
            panic!("Mock peer received unencrypted PR_ED2K (0xE3)! Should have been obfuscated.");
        }

        // 2. Perform inbound obfuscation handshake
        let mut stream =
            ObfuscatedStream::accept_peer_incoming(socket, first_byte[0], &mock_peer_user_hash, 16)
                .await
                .expect("Mock peer failed accept_peer_incoming");
        assert!(stream.is_obfuscated());

        // 3. Read encrypted OP_HELLO via packet codec
        let mut read_buf = bytes::BytesMut::with_capacity(4096);
        let mut raw_buf = [0u8; 1024];
        let codec = PacketCodec::new();
        let decoder = Ed2kDecoder::default();
        let encoder = Ed2kEncoder::default();

        let mut received_hello = false;
        while !received_hello {
            let n = stream.read(&mut raw_buf).await.unwrap();
            assert!(n > 0, "Unexpected EOF while reading OP_HELLO");
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO {
                    let hello = decoder
                        .decode_hello(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                        .unwrap();
                    assert_eq!(hello.client_id, 0);
                    received_hello = true;
                    break;
                }
            }
        }

        // 4. Send encrypted OP_HELLO_ANSWER
        let hello_ans = Ed2kHelloAnswer {
            user_hash: mock_peer_user_hash,
            client_id: 0x0100007F,
            tcp_port: peer_addr.port(),
            tags: vec![],
            unknown_tags: vec![],
            server_ip: 0,
            server_port: 0,
        };
        let ans_pkt = encoder.encode_hello_answer(&hello_ans).unwrap();
        let mut wire = Vec::new();
        wire.push(ans_pkt.protocol);
        wire.extend_from_slice(&(ans_pkt.frame_len() as u32).to_le_bytes());
        wire.push(ans_pkt.opcode);
        wire.extend_from_slice(&ans_pkt.payload);

        stream.write_all(&wire).await.unwrap();
        stream.flush().await.unwrap();

        // Keep connection open briefly so client transitions to Connected
        tokio::time::sleep(Duration::from_millis(150)).await;
    });

    // Request connection through RPC specifying mock peer's user_hash and crypt_options (RequireCrypt = 0x07)
    let peer_hash_hex = hex::encode(mock_peer_user_hash);
    let connect_req = make_req(
        10,
        "sources.connect",
        Some(json!({
            "fileHash": hash,
            "address": peer_addr.to_string(),
            "userHash": peer_hash_hex,
            "cryptOptions": 7 // SupportsCrypt | RequestsCrypt | RequiresCrypt
        })),
    );
    let resp = handle_request(&state, connect_req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_none(),
        "RPC sources.connect failed: {:?}",
        resp.error
    );

    let _ = mock_peer_task.await;

    // Verify session snapshot has obfuscated: true via sources.status
    let status_req = make_req(11, "sources.status", None);
    let status_resp = handle_request(&state, status_req, &mut authenticated)
        .await
        .unwrap();
    let snap_val = status_resp.result.unwrap();
    assert!(
        snap_val["status"] == "queued"
            || snap_val["status"] == "handshaking"
            || snap_val["status"] == "connected",
        "Unexpected status: {:?}",
        snap_val["status"]
    );
    assert_eq!(snap_val["obfuscated"], true);
}

#[tokio::test]
async fn test_outgoing_connection_to_strict_peer_fails_when_user_hash_missing() {
    let (state, _dir, hash) = setup_obfu_test_state(true, false).await;
    let mut authenticated = true;

    // We configure a peer with crypt_options requiring crypt (0x04), but NO user_hash
    let peer_addr = "127.0.0.1:49999";
    let connect_req = make_req(
        11,
        "sources.connect",
        Some(json!({
            "fileHash": hash,
            "address": peer_addr,
            "cryptOptions": 4 // RequiresCrypt bit set, but no userHash given
        })),
    );
    let resp = handle_request(&state, connect_req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_some(),
        "Expected connection failure due to missing user hash"
    );
    let err = resp.error.unwrap();
    assert!(
        err.message.contains("requires protocol obfuscation") || err.message.contains("user hash"),
        "Unexpected error message: {}",
        err.message
    );
}

#[tokio::test]
async fn test_inbound_listener_strict_obfuscation_rejects_plaintext() {
    // Obfuscation required = true
    let (state, _dir, _hash) = setup_obfu_test_state(true, true).await;
    let mut authenticated = true;

    // Start listener on ephemeral port
    let start_req = make_req(20, "listener.start", Some(json!({ "port": 0 })));
    let start_resp = handle_request(&state, start_req, &mut authenticated)
        .await
        .unwrap();
    assert!(start_resp.error.is_none());
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    // Connect remote peer sending standard unencrypted 0xE3
    let mut socket = TcpStream::connect(&listen_addr_str).await.unwrap();
    socket.write_all(&[PR_ED2K]).await.unwrap();
    socket.flush().await.unwrap();

    // Daemon should immediately drop/close the unencrypted connection
    let mut buf = [0u8; 16];
    let n = socket.read(&mut buf).await.unwrap();
    assert_eq!(
        n, 0,
        "Daemon must close unencrypted connection when obfuscation is required"
    );
}

#[tokio::test]
async fn test_inbound_listener_strict_obfuscation_accepts_encrypted_handshake() {
    // Obfuscation required = true
    let (state, _dir, _hash) = setup_obfu_test_state(true, true).await;
    let mut authenticated = true;

    // Start listener on ephemeral port
    let start_req = make_req(30, "listener.start", Some(json!({ "port": 0 })));
    let start_resp = handle_request(&state, start_req, &mut authenticated)
        .await
        .unwrap();
    assert!(start_resp.error.is_none());
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let daemon_user_hash = state.identity.lock().await.user_hash();

    // Connect client and perform obfuscated handshake
    let socket = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut stream = ObfuscatedStream::connect_peer_outgoing(socket, &daemon_user_hash, 16)
        .await
        .expect("Client outgoing obfuscation handshake to daemon failed");
    assert!(stream.is_obfuscated());

    // Send OP_HELLO through encrypted stream
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let codec = PacketCodec::new();

    let client_user_hash = [0x55u8; 16];
    let hello = Ed2kHello {
        user_hash: client_user_hash,
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut wire = Vec::new();
    wire.push(hello_pkt.protocol);
    wire.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    wire.push(hello_pkt.opcode);
    wire.extend_from_slice(&hello_pkt.payload);

    stream.write_all(&wire).await.unwrap();
    stream.flush().await.unwrap();

    // Read and verify encrypted OP_HELLO_ANSWER from daemon
    let mut read_buf = bytes::BytesMut::with_capacity(4096);
    let mut raw_buf = [0u8; 1024];
    let mut received_ans = false;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !received_ans && tokio::time::Instant::now() < deadline {
        let n = stream.read(&mut raw_buf).await.unwrap();
        if n == 0 {
            break;
        }
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLOANSWER {
                let ans = decoder
                    .decode_hello_answer(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    .unwrap();
                assert_eq!(ans.user_hash, daemon_user_hash);
                received_ans = true;
                break;
            }
        }
    }
    assert!(
        received_ans,
        "Client did not receive OP_HELLO_ANSWER over obfuscated connection"
    );
}

#[tokio::test]
async fn test_inbound_listener_opportunistic_accepts_both_plaintext_and_encrypted() {
    // Obfuscation enabled = true, required = false (opportunistic)
    let (state, _dir, _hash) = setup_obfu_test_state(true, false).await;
    let mut authenticated = true;

    let start_req = make_req(40, "listener.start", Some(json!({ "port": 0 })));
    let start_resp = handle_request(&state, start_req, &mut authenticated)
        .await
        .unwrap();
    assert!(start_resp.error.is_none());
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let daemon_user_hash = state.identity.lock().await.user_hash();
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let codec = PacketCodec::new();

    // 1. Test plaintext inbound connection
    {
        let mut socket = TcpStream::connect(&listen_addr_str).await.unwrap();
        let hello = Ed2kHello {
            user_hash: [0x11u8; 16],
            client_id: 0x0100007F,
            tcp_port: 39735,
            tags: vec![],
            unknown_tags: vec![],
            server_ip: 0,
            server_port: 0,
        };
        let hello_pkt = encoder.encode_hello(&hello).unwrap();
        let mut wire = Vec::new();
        wire.push(hello_pkt.protocol);
        wire.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
        wire.push(hello_pkt.opcode);
        wire.extend_from_slice(&hello_pkt.payload);

        socket.write_all(&wire).await.unwrap();
        socket.flush().await.unwrap();

        let mut read_buf = bytes::BytesMut::with_capacity(4096);
        let mut raw_buf = [0u8; 1024];
        let mut received_ans = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !received_ans && tokio::time::Instant::now() < deadline {
            let n = socket.read(&mut raw_buf).await.unwrap();
            if n == 0 {
                break;
            }
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLOANSWER {
                    let ans = decoder
                        .decode_hello_answer(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                        .unwrap();
                    assert_eq!(ans.user_hash, daemon_user_hash);
                    received_ans = true;
                    break;
                }
            }
        }
        assert!(
            received_ans,
            "Plaintext connection must be accepted in opportunistic mode"
        );
    }

    // 2. Test encrypted inbound connection
    {
        let socket = TcpStream::connect(&listen_addr_str).await.unwrap();
        let mut stream = ObfuscatedStream::connect_peer_outgoing(socket, &daemon_user_hash, 16)
            .await
            .unwrap();

        let hello = Ed2kHello {
            user_hash: [0x22u8; 16],
            client_id: 0x0100007F,
            tcp_port: 39736,
            tags: vec![],
            unknown_tags: vec![],
            server_ip: 0,
            server_port: 0,
        };
        let hello_pkt = encoder.encode_hello(&hello).unwrap();
        let mut wire = Vec::new();
        wire.push(hello_pkt.protocol);
        wire.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
        wire.push(hello_pkt.opcode);
        wire.extend_from_slice(&hello_pkt.payload);

        stream.write_all(&wire).await.unwrap();
        stream.flush().await.unwrap();

        let mut read_buf = bytes::BytesMut::with_capacity(4096);
        let mut raw_buf = [0u8; 1024];
        let mut received_ans = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !received_ans && tokio::time::Instant::now() < deadline {
            let n = stream.read(&mut raw_buf).await.unwrap();
            if n == 0 {
                break;
            }
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLOANSWER {
                    let ans = decoder
                        .decode_hello_answer(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                        .unwrap();
                    assert_eq!(ans.user_hash, daemon_user_hash);
                    received_ans = true;
                    break;
                }
            }
        }
        assert!(
            received_ans,
            "Encrypted connection must be accepted in opportunistic mode"
        );
    }
}
