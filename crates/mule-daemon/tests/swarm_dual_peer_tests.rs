use bytes::Bytes;
use serde_json::{json, Value};
use std::sync::Arc;
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
    let token = "test_token_swarm".to_string();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);
    state
        .network_policy
        .set_profile(mule_network_policy::NetworkProfile::LocalLab);

    let file_hash_hex = "31d6cfe0d16ae931b73c59d7e0c089c0".to_string();

    // Insert download of 18 MB (2 parts) via RPC downloads.addEd2kLink
    let link = format!("ed2k://|file|test_swarm_payload.iso|18874368|{file_hash_hex}|/");
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

/// Helper function to spawn a mock peer server that answers hello, filestatus (full 2 parts),
/// and records requested block ranges.
fn spawn_mock_peer_recorder(
    hash_hex: String,
    requested_ranges: Arc<std::sync::Mutex<Vec<(u64, u64)>>>,
    disconnect_after_first_chunk: bool,
) -> (tokio::task::JoinHandle<()>, std::net::SocketAddr) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let peer_addr = listener.local_addr().unwrap();
    let tokio_listener = TcpListener::from_std(listener).unwrap();

    let handle = tokio::spawn(async move {
        let (mut socket, _) = tokio_listener.accept().await.unwrap();
        let mut read_buf = bytes::BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let encoder = Ed2kEncoder::default();
        let hash_bytes: [u8; 16] = hex::decode(&hash_hex).unwrap().try_into().unwrap();

        // 1. Read OP_HELLO & Send OP_HELLO_ANSWER
        let mut received_hello = false;
        while !received_hello {
            let n = socket.read(&mut raw_buf).await.unwrap();
            if n == 0 {
                return;
            }
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
        let _ = socket.write_all(&out).await;

        // 2. Read OP_FILESTATUSREQ & Send OP_FILESTATUS (part_count = 2)
        let mut received_status_req = false;
        while !received_status_req {
            let n = socket.read(&mut raw_buf).await.unwrap();
            if n == 0 {
                return;
            }
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && (pkt.opcode == OP_FILESTATUSREQ || pkt.opcode == 0x4F)
                {
                    received_status_req = true;
                    break;
                }
            }
        }

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
        let _ = socket.write_all(&out).await;

        // 3. Read OP_STARTUPLOADREQ and reply with OP_ACCEPTUPLOADREQ
        let mut received_start_upload = false;
        while !received_start_upload {
            let n = socket.read(&mut raw_buf).await.unwrap();
            if n == 0 {
                return;
            }
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

        // 4. Read OP_REQUESTPARTS and send data
        let mut received_req = false;
        while !received_req {
            let n = socket.read(&mut raw_buf).await.unwrap();
            if n == 0 {
                return;
            }
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K
                    && (pkt.opcode == OP_REQUESTPARTS || pkt.opcode == OP_REQUESTPARTS_EMULE)
                {
                    // Extract requested ranges from packet
                    if let Ok(req_parts) = mule_protocol::ed2k::Ed2kDecoder::default()
                        .decode_request_parts(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    {
                        for r in &req_parts.ranges {
                            requested_ranges
                                .lock()
                                .unwrap()
                                .push((r.start as u64, r.end as u64));
                        }
                    }
                    received_req = true;
                    break;
                }
            }
        }

        if disconnect_after_first_chunk {
            // Drop immediately to simulate unexpected disconnect
            drop(socket);
            return;
        }

        // Send 1 valid chunk (1024 bytes)
        let dummy = vec![0x33u8; 1024];
        let req_start = requested_ranges
            .lock()
            .unwrap()
            .first()
            .map(|r| r.0)
            .unwrap_or(0);
        let sending_part = Ed2kSendingPart::new(
            hash_bytes,
            req_start as u32,
            (req_start + 1024) as u32,
            Bytes::from(dummy),
        );
        let send_pkt = encoder.encode_sending_part(&sending_part).unwrap();
        out.clear();
        out.push(send_pkt.protocol);
        out.extend_from_slice(&(send_pkt.frame_len() as u32).to_le_bytes());
        out.push(send_pkt.opcode);
        out.extend_from_slice(&send_pkt.payload);
        let _ = socket.write_all(&out).await;

        // Hold open for duration of test
        tokio::time::sleep(Duration::from_millis(1500)).await;
    });

    (handle, peer_addr)
}

#[tokio::test]
async fn test_swarm_dual_peer_disjoint_block_allocation() {
    let (state, _dir, hash) = setup_test_state().await;
    let mut authenticated = true;

    let peer1_ranges = Arc::new(std::sync::Mutex::new(Vec::new()));
    let peer2_ranges = Arc::new(std::sync::Mutex::new(Vec::new()));

    let (_h1, addr1) = spawn_mock_peer_recorder(hash.clone(), peer1_ranges.clone(), false);
    let (_h2, addr2) = spawn_mock_peer_recorder(hash.clone(), peer2_ranges.clone(), false);

    // 1. Connect peer 1
    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": addr1.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    // 2. Connect peer 2 concurrently
    let req = make_req(
        2,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": addr2.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    // Wait for handshake and request parts exchange
    tokio::time::sleep(Duration::from_millis(300)).await;

    // 3. Inspect swarm status via RPC sources.swarmStatus
    let req = make_req(3, "sources.swarmStatus", Some(json!({ "fileHash": hash })));
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let swarm: Value = resp.result.unwrap();
    assert_eq!(swarm["activePeerCount"], 2);
    let in_flight = swarm["inFlightBlocks"].as_array().unwrap();
    assert_eq!(
        in_flight.len(),
        2,
        "Both peers should have in-flight blocks"
    );

    // 4. Verify that requested ranges are DISJOINT (no overlap)
    let p1_r = peer1_ranges.lock().unwrap().clone();
    let p2_r = peer2_ranges.lock().unwrap().clone();
    assert!(!p1_r.is_empty(), "Peer 1 should have received a request");
    assert!(!p2_r.is_empty(), "Peer 2 should have received a request");

    let r1 = p1_r[0];
    let r2 = p2_r[0];
    assert!(
        r1.1 <= r2.0 || r2.1 <= r1.0,
        "Ranges must be disjoint: peer1={:?}, peer2={:?}",
        r1,
        r2
    );

    // 5. Disconnect single peer (peer 1)
    let req = make_req(
        4,
        "sources.disconnect",
        Some(json!({ "address": addr1.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify peer 2 is still active
    let req = make_req(5, "sources.swarmStatus", Some(json!({ "fileHash": hash })));
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let swarm: Value = resp.result.unwrap();
    assert_eq!(swarm["activePeerCount"], 1);

    // 6. Disconnect all (swarm teardown)
    let req = make_req(6, "sources.disconnect", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    let req = make_req(7, "sources.swarmStatus", Some(json!({ "fileHash": hash })));
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    let swarm: Value = resp.result.unwrap();
    assert_eq!(swarm["activePeerCount"], 0);
}

#[tokio::test]
async fn test_swarm_disconnect_releases_in_flight_block_for_reallocation() {
    let (state, _dir, hash) = setup_test_state().await;
    let mut authenticated = true;

    let peer1_ranges = Arc::new(std::sync::Mutex::new(Vec::new()));
    let peer2_ranges = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Peer 1 will disconnect abruptly
    let (_h1, addr1) = spawn_mock_peer_recorder(hash.clone(), peer1_ranges.clone(), true);
    // Peer 2 connects after peer 1 disconnects
    let (_h2, addr2) = spawn_mock_peer_recorder(hash.clone(), peer2_ranges.clone(), false);

    // 1. Connect peer 1
    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": addr1.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    // Wait for peer 1 disconnect
    tokio::time::sleep(Duration::from_millis(250)).await;

    // 2. Connect peer 2
    let req = make_req(
        2,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": addr2.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    tokio::time::sleep(Duration::from_millis(250)).await;

    // Peer 2 should have been able to request block 0 (which was abandoned by peer 1)
    let p2_r = peer2_ranges.lock().unwrap().clone();
    assert!(!p2_r.is_empty());
    assert_eq!(
        p2_r[0].0, 0,
        "Peer 2 should claim block 0 after peer 1 failed"
    );

    let _ = state.peer_session.disconnect_peer(None).await;
}

#[tokio::test]
async fn test_swarm_kill_switch_aborts_all_peers_and_releases_permits() {
    let (state, _dir, hash) = setup_test_state().await;
    let mut authenticated = true;

    let dummy_ranges1 = Arc::new(std::sync::Mutex::new(Vec::new()));
    let dummy_ranges2 = Arc::new(std::sync::Mutex::new(Vec::new()));

    let (_h1, addr1) = spawn_mock_peer_recorder(hash.clone(), dummy_ranges1, false);
    let (_h2, addr2) = spawn_mock_peer_recorder(hash.clone(), dummy_ranges2, false);

    // Connect dual peers
    let _ = handle_request(
        &state,
        make_req(
            1,
            "sources.connect",
            Some(json!({ "fileHash": hash, "address": addr1.to_string() })),
        ),
        &mut authenticated,
    )
    .await
    .unwrap();
    let _ = handle_request(
        &state,
        make_req(
            2,
            "sources.connect",
            Some(json!({ "fileHash": hash, "address": addr2.to_string() })),
        ),
        &mut authenticated,
    )
    .await
    .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(
        state
            .peer_session
            .swarm_snapshot(None)
            .await
            .active_peer_count,
        2
    );

    // Trigger Kill Switch: network.disconnectAll
    let req = make_req(3, "network.disconnectAll", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    tokio::time::sleep(Duration::from_millis(50)).await;

    let swarm = state.peer_session.swarm_snapshot(None).await;
    assert_eq!(
        swarm.active_peer_count, 0,
        "Kill switch must abort all active peers in swarm"
    );
    assert!(
        swarm.in_flight_blocks.is_empty(),
        "In-flight blocks must be cleared on kill switch"
    );
}

#[tokio::test]
async fn test_swarm_global_peer_limit_across_multiple_downloads() {
    let (state, _dir, hash1) = setup_test_state().await;
    let mut authenticated = true;
    state.peer_session.set_swarm_limits(2, 4).await;

    // Add a second download
    let hash2 = "41d6cfe0d16ae931b73c59d7e0c089c1".to_string();
    let link2 = format!("ed2k://|file|test_second_file.iso|18874368|{hash2}|/");
    let _ = handle_request(
        &state,
        make_req(2, "downloads.addEd2kLink", Some(json!({ "link": link2 }))),
        &mut authenticated,
    )
    .await
    .unwrap();

    let r1 = Arc::new(std::sync::Mutex::new(Vec::new()));
    let r2 = Arc::new(std::sync::Mutex::new(Vec::new()));
    let r3 = Arc::new(std::sync::Mutex::new(Vec::new()));
    let r4 = Arc::new(std::sync::Mutex::new(Vec::new()));

    let (_h1, a1) = spawn_mock_peer_recorder(hash1.clone(), r1, false);
    let (_h2, a2) = spawn_mock_peer_recorder(hash1.clone(), r2, false);
    let (_h3, a3) = spawn_mock_peer_recorder(hash2.clone(), r3, false);
    let (_h4, a4) = spawn_mock_peer_recorder(hash2.clone(), r4, false);

    // Connect 2 peers to file 1
    assert!(handle_request(
        &state,
        make_req(
            10,
            "sources.connect",
            Some(json!({ "fileHash": hash1, "address": a1.to_string() }))
        ),
        &mut authenticated
    )
    .await
    .unwrap()
    .error
    .is_none());
    assert!(handle_request(
        &state,
        make_req(
            11,
            "sources.connect",
            Some(json!({ "fileHash": hash1, "address": a2.to_string() }))
        ),
        &mut authenticated
    )
    .await
    .unwrap()
    .error
    .is_none());

    // Connect 2 peers to file 2 (Total = 4 global peers)
    assert!(handle_request(
        &state,
        make_req(
            12,
            "sources.connect",
            Some(json!({ "fileHash": hash2, "address": a3.to_string() }))
        ),
        &mut authenticated
    )
    .await
    .unwrap()
    .error
    .is_none());
    assert!(handle_request(
        &state,
        make_req(
            13,
            "sources.connect",
            Some(json!({ "fileHash": hash2, "address": a4.to_string() }))
        ),
        &mut authenticated
    )
    .await
    .unwrap()
    .error
    .is_none());

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        state
            .peer_session
            .swarm_snapshot(None)
            .await
            .active_peer_count,
        4
    );

    // Try to connect a 5th global peer to a new address -> Must be rejected with global limit
    let resp = handle_request(
        &state,
        make_req(
            14,
            "sources.connect",
            Some(json!({ "fileHash": hash2, "address": "127.0.0.1:9998" })),
        ),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_some(), "5th global peer must be rejected");
    let err = resp.error.unwrap();
    assert!(err.message.contains("Global peer connection limit reached"));

    let _ = state.peer_session.disconnect_peer(None).await;
}

#[tokio::test]
async fn test_swarm_pause_download_aborts_active_leases_and_sessions() {
    let (state, _dir, hash) = setup_test_state().await;
    let mut authenticated = true;

    let r1 = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (_h1, a1) = spawn_mock_peer_recorder(hash.clone(), r1, false);

    let resp = handle_request(
        &state,
        make_req(
            1,
            "sources.connect",
            Some(json!({ "fileHash": hash, "address": a1.to_string() })),
        ),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        state
            .peer_session
            .swarm_snapshot(None)
            .await
            .active_peer_count,
        1
    );

    // Disconnect swarm for download
    let resp = handle_request(
        &state,
        make_req(2, "sources.disconnect", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());

    let swarm = state.peer_session.swarm_snapshot(Some(&hash)).await;
    assert_eq!(swarm.active_peer_count, 0);
    assert!(swarm.in_flight_blocks.is_empty());
}
