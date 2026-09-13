//! Integration test verifying eD2K block transfer, interruption at 50%, cold restart, gap-only resumption,
//! and final root eD2K hash verification.
//! Phase 6.4: Strictly confined to local loopback (127.0.0.1). Zero public network.

use std::io::Read;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use mule_core::Database;
use mule_daemon::{create_router, AppState, DaemonConfig, DirectoryLock, DownloadService};
use mule_format::fixtures::{
    PatternReader, FIXTURE_EXACT_ONE_PART, FIXTURE_ONE_PART_PLUS_ONE, REPEATING_0_TO_255,
};
use mule_format::Ed2kHash;
use mule_protocol::ed2k::{Ed2kTestPeer, PartAvailabilityMask, TestPeerMode};
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const FIXTURE_FILE_SIZE: u64 = 9_728_000;
const AUTH_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct RunningDaemon {
    addr: SocketAddr,
    state: AppState,
    _token: String,
    _lock: DirectoryLock,
    server_handle: JoinHandle<()>,
}

impl RunningDaemon {
    async fn start(workdir: PathBuf) -> Self {
        let lock = DirectoryLock::acquire(&workdir).expect("acquires exclusive directory lock");
        let db_path = workdir.join("mule.db");
        let temp_dir = workdir.join("part");

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let token = AUTH_TOKEN.to_string();
        let config = DaemonConfig::new(workdir, &token).with_bind_addr(addr);
        let downloads =
            DownloadService::new(&db_path, temp_dir).expect("opens persistent download service");
        let state = AppState::new(config, downloads);
        let app = create_router(state.clone());

        let server_handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        Self {
            addr,
            state,
            _token: token,
            _lock: lock,
            server_handle,
        }
    }

    async fn stop(self) {
        self.state.abort_all_tasks().await;
        self.server_handle.abort();
    }
}

#[tokio::test]
async fn test_ed2k_transfer_with_interruption_and_gap_only_resumption() {
    // 1. Prepare isolated directories
    let dir_client = tempdir().unwrap();
    let client_target_dir = tempdir().unwrap();

    let fixture_hash = Ed2kHash::from_hex(FIXTURE_EXACT_ONE_PART.expected_ed2k_hex).unwrap();

    // Generate exact fixture data (9 728 000 bytes)
    let mut pattern_reader = PatternReader::new(&REPEATING_0_TO_255, FIXTURE_FILE_SIZE);
    let mut fixture_data = Vec::with_capacity(FIXTURE_FILE_SIZE as usize);
    pattern_reader.read_to_end(&mut fixture_data).unwrap();
    let fixture_bytes = Bytes::from(fixture_data);

    // 2. Start controlled loopback test peer sharing the exact fixture
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();

    test_peer
        .add_shared_file(
            fixture_hash.0,
            1,
            PartAvailabilityMask::from_parts(&[0], 1),
        )
        .await;
    test_peer
        .add_shared_file_data(fixture_hash.0, fixture_bytes.clone())
        .await;

    let (peer_handle, peer_stopped) = test_peer.spawn_service();

    // 3. Start Client Daemon B
    let client_daemon = RunningDaemon::start(dir_client.path().to_path_buf()).await;
    let client_addr = client_daemon.addr;

    // Connect WebSocket
    let ws_url = format!("ws://{client_addr}/rpc");
    let (ws_stream, _) = connect_async(&ws_url).await.expect("connects to daemon B");
    let (mut write, mut read) = ws_stream.split();

    // Handshake
    let req_hello = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "system.hello",
        "params": {
            "protocolVersion": 1,
            "token": AUTH_TOKEN
        }
    });
    write
        .send(Message::Text(req_hello.to_string().into()))
        .await
        .unwrap();
    let resp = read.next().await.unwrap().unwrap();
    let hello_val: Value = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    assert_eq!(hello_val["result"]["protocolVersion"], 1);

    // Subscribe to events
    let req_sub = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "events.subscribe",
        "params": {}
    });
    write
        .send(Message::Text(req_sub.to_string().into()))
        .await
        .unwrap();
    let _ = read.next().await.unwrap().unwrap();

    // Add eD2K download link
    let ed2k_link = format!(
        "ed2k://|file|fixture_9728000.bin|{}|{}|/",
        FIXTURE_FILE_SIZE, fixture_hash
    );
    let req_add = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "downloads.addEd2kLink",
        "params": {
            "link": ed2k_link,
            "targetDirectory": client_target_dir.path().to_str().unwrap()
        }
    });
    write
        .send(Message::Text(req_add.to_string().into()))
        .await
        .unwrap();

    let resp_add = read.next().await.unwrap().unwrap();
    let add_val: Value = serde_json::from_str(resp_add.to_text().unwrap()).unwrap();
    let download_id = add_val["result"]["downloadId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(add_val["result"]["status"], "queued");

    // Read initial event
    let evt_msg = read.next().await.unwrap().unwrap();
    let evt_val: Value = serde_json::from_str(evt_msg.to_text().unwrap()).unwrap();
    assert_eq!(evt_val["method"], "event.downloadUpdated");
    assert_eq!(evt_val["params"]["download"]["status"], "queued");

    // 4. Start eD2K transfer via ed2k.startDownload
    let req_start = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "ed2k.startDownload",
        "params": {
            "downloadId": download_id,
            "peerAddr": peer_addr.to_string()
        }
    });
    write
        .send(Message::Text(req_start.to_string().into()))
        .await
        .unwrap();
    let resp_start = read.next().await.unwrap().unwrap();
    let start_val: Value = serde_json::from_str(resp_start.to_text().unwrap()).unwrap();
    assert_eq!(start_val["result"]["started"], true);

    // 5. Monitor transfer progress until ~50% (at least 4.5 MB = ~25 blocks)
    let mut saw_waiting_for_sources = false;
    let mut saw_downloading = false;
    let mut received_at_interrupt = 0u64;

    while let Some(Ok(msg)) = read.next().await {
        let text = msg.to_text().unwrap();
        if let Ok(evt) = serde_json::from_str::<Value>(text) {
            if evt["method"] == "event.downloadUpdated" {
                let dl = &evt["params"]["download"];
                let status = dl["status"].as_str().unwrap_or("");
                let received = dl["receivedBytes"].as_u64().unwrap_or(0);

                if status == "waiting_for_sources" {
                    saw_waiting_for_sources = true;
                } else if status == "downloading" {
                    saw_downloading = true;
                    if received >= 4_500_000 {
                        received_at_interrupt = received;
                        break;
                    }
                }
            }
        }
    }

    assert!(saw_waiting_for_sources, "Expected waiting_for_sources state");
    assert!(saw_downloading, "Expected downloading state");
    assert!(
        (4_500_000..FIXTURE_FILE_SIZE).contains(&received_at_interrupt),
        "Interrupted at ~50%: received {} bytes",
        received_at_interrupt
    );

    // 6. Abruptly kill Client Daemon B (crash simulation)
    drop(write);
    drop(read);
    client_daemon.stop().await;

    // Small delay to ensure file lock cleanup on Windows
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 7. Verify disk and SQLite WAL state at interruption
    let part_file_path = dir_client.path().join("part").join(format!("{download_id}.part"));
    assert!(part_file_path.exists(), ".part file must exist on disk");
    let part_size_at_interrupt = std::fs::metadata(&part_file_path).unwrap().len();
    assert!(
        part_size_at_interrupt >= received_at_interrupt,
        ".part file size ({part_size_at_interrupt}) must be at least received ({received_at_interrupt})"
    );

    // Verify SQLite WAL DB has synced received ranges
    let db_path = dir_client.path().join("mule.db");
    let db = Database::open(&db_path).expect("open SQLite DB");
    let restored_id = uuid::Uuid::parse_str(&download_id).unwrap();
    let saved_ranges = db.load_received_ranges(&restored_id).expect("load ranges");
    assert!(
        saved_ranges.covered_len() >= received_at_interrupt,
        "DB ranges covered length ({}) must be at least observed received bytes ({received_at_interrupt})",
        saved_ranges.covered_len()
    );
    drop(db);

    // 8. Cold restart Client Daemon on the EXACT SAME workdir
    let client_daemon_2 = RunningDaemon::start(dir_client.path().to_path_buf()).await;
    let client_addr_2 = client_daemon_2.addr;

    let ws_url_2 = format!("ws://{client_addr_2}/rpc");
    let (ws_stream_2, _) = connect_async(&ws_url_2)
        .await
        .expect("reconnects to restarted daemon B");
    let (mut write_2, mut read_2) = ws_stream_2.split();

    // Handshake
    let req_hello_2 = json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "system.hello",
        "params": {
            "protocolVersion": 1,
            "token": AUTH_TOKEN
        }
    });
    write_2
        .send(Message::Text(req_hello_2.to_string().into()))
        .await
        .unwrap();
    let resp = read_2.next().await.unwrap().unwrap();
    let hello_val_2: Value = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    assert_eq!(hello_val_2["result"]["protocolVersion"], 1);

    // 9. Verify restored download status and neutral SourceAvailabilitySummary
    let req_list = json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "downloads.list",
        "params": {}
    });
    write_2
        .send(Message::Text(req_list.to_string().into()))
        .await
        .unwrap();
    let resp_list = read_2.next().await.unwrap().unwrap();
    let list_val: Value = serde_json::from_str(resp_list.to_text().unwrap()).unwrap();
    let dls = list_val["result"]["downloads"].as_array().unwrap();
    assert_eq!(dls.len(), 1, "Exactly one download restored from SQLite");
    let restored_dl = &dls[0];
    assert_eq!(restored_dl["id"], download_id);
    let restored_received = restored_dl["receivedBytes"].as_u64().unwrap();
    assert!(
        (received_at_interrupt..FIXTURE_FILE_SIZE).contains(&restored_received),
        "Restored receivedBytes ({restored_received}) is >= observed interrupt ({received_at_interrupt})"
    );

    // Subscribe to events
    let req_sub_2 = json!({
        "jsonrpc": "2.0",
        "id": 12,
        "method": "events.subscribe",
        "params": {}
    });
    write_2
        .send(Message::Text(req_sub_2.to_string().into()))
        .await
        .unwrap();
    let _ = read_2.next().await.unwrap().unwrap();

    // 10. Resume transfer by re-invoking ed2k.startDownload
    let req_resume = json!({
        "jsonrpc": "2.0",
        "id": 13,
        "method": "ed2k.startDownload",
        "params": {
            "downloadId": download_id,
            "peerAddr": peer_addr.to_string()
        }
    });
    write_2
        .send(Message::Text(req_resume.to_string().into()))
        .await
        .unwrap();

    // 11. Collect events through Verifying and Completed
    let mut saw_verifying = false;
    let mut saw_completed = false;

    while let Some(Ok(msg)) = read_2.next().await {
        let text = msg.to_text().unwrap();
        if let Ok(val) = serde_json::from_str::<Value>(text) {
            if val.get("id").and_then(|id| id.as_u64()) == Some(13) {
                assert_eq!(val["result"]["started"], true);
            } else if val["method"] == "event.downloadUpdated" {
                let dl = &val["params"]["download"];
                let status = dl["status"].as_str().unwrap_or("");
                let received = dl["receivedBytes"].as_u64().unwrap_or(0);

                if status == "verifying" {
                    saw_verifying = true;
                    assert_eq!(
                        received, FIXTURE_FILE_SIZE,
                        "All bytes must be received before verifying"
                    );
                } else if status == "completed" {
                    saw_completed = true;
                    assert_eq!(received, FIXTURE_FILE_SIZE);
                    assert_eq!(dl["progress"], 1.0);
                    break;
                }
            }
        }
    }

    assert!(saw_verifying, "Expected verifying state transition");
    assert!(saw_completed, "Expected completed state transition");

    // 12. Verify neutral source summary via downloads.getSources
    let req_sources = json!({
        "jsonrpc": "2.0",
        "id": 14,
        "method": "downloads.getSources",
        "params": {
            "downloadId": download_id
        }
    });
    write_2
        .send(Message::Text(req_sources.to_string().into()))
        .await
        .unwrap();

    let mut sources_val = Value::Null;
    while let Some(Ok(msg)) = read_2.next().await {
        let text = msg.to_text().unwrap();
        if let Ok(val) = serde_json::from_str::<Value>(text) {
            if val.get("id").and_then(|id| id.as_u64()) == Some(14) {
                sources_val = val;
                break;
            }
        }
    }
    assert!(sources_val.get("result").is_some(), "Expected result for getSources: {sources_val:?}");
    let src_list = sources_val["result"]["sources"].as_array().unwrap();
    assert!(!src_list.is_empty(), "Source availability must be recorded");
    let src_item = &src_list[0];
    assert!(src_item["sourceId"].is_string());
    assert_eq!(src_item["availablePartCount"], 1);
    assert_eq!(src_item["totalPartCount"], 1);
    assert!(src_item.get("peerAddr").is_none(), "Raw SocketAddr must NEVER be exposed in DTO");

    // 12. Verify finalized file on disk
    let finalized_path = client_target_dir.path().join("fixture_9728000.bin");
    assert!(
        finalized_path.exists(),
        "Finalized target file must exist on disk"
    );
    assert_eq!(
        std::fs::metadata(&finalized_path).unwrap().len(),
        FIXTURE_FILE_SIZE,
        "Finalized file length must match fixture exactly"
    );

    let mut target_file = std::fs::File::open(&finalized_path).unwrap();
    let final_ed2k = mule_format::ed2k::hash_reader(&mut target_file).unwrap();
    assert_eq!(
        final_ed2k.root_hash, fixture_hash,
        "Final eD2K root hash must match fixture_exact_one_part exactly"
    );

    // Verify byte-for-byte fidelity
    let mut finalized_bytes = Vec::with_capacity(FIXTURE_FILE_SIZE as usize);
    target_file = std::fs::File::open(&finalized_path).unwrap();
    target_file.read_to_end(&mut finalized_bytes).unwrap();
    assert_eq!(finalized_bytes, fixture_bytes.as_ref());

    // Clean shutdown
    client_daemon_2.stop().await;
    peer_stopped.store(true, Ordering::SeqCst);
    peer_handle.abort();
}

#[tokio::test]
async fn test_ed2k_transfer_residual_part_file_completion() {
    let file_size = FIXTURE_ONE_PART_PLUS_ONE.length; // 9_728_001 bytes
    let fixture_hash = Ed2kHash::from_hex(FIXTURE_ONE_PART_PLUS_ONE.expected_ed2k_hex).unwrap();

    // 1. Prepare isolated directories
    let dir_client = tempdir().unwrap();
    let client_target_dir = tempdir().unwrap();

    // Generate exact fixture data (9 728 001 bytes: 1 full part + 1 byte residual)
    let mut pattern_reader = PatternReader::new(&REPEATING_0_TO_255, file_size);
    let mut fixture_data = Vec::with_capacity(file_size as usize);
    pattern_reader.read_to_end(&mut fixture_data).unwrap();
    let fixture_bytes = Bytes::from(fixture_data);

    // 2. Start controlled loopback test peer sharing 2 parts
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();

    test_peer
        .add_shared_file(
            fixture_hash.0,
            2,
            PartAvailabilityMask::from_parts(&[0, 1], 2),
        )
        .await;
    test_peer
        .add_shared_file_data(fixture_hash.0, fixture_bytes.clone())
        .await;

    let (peer_handle, peer_stopped) = test_peer.spawn_service();

    // 3. Start Client Daemon
    let client_daemon = RunningDaemon::start(dir_client.path().to_path_buf()).await;
    let client_addr = client_daemon.addr;

    // Connect WebSocket RPC
    let ws_url = format!("ws://{client_addr}/rpc");
    let (ws_stream, _) = connect_async(&ws_url).await.expect("connects to daemon");
    let (mut write, mut read) = ws_stream.split();

    // Handshake
    let req_hello = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "system.hello",
        "params": {
            "protocolVersion": 1,
            "token": AUTH_TOKEN
        }
    });
    write
        .send(Message::Text(req_hello.to_string().into()))
        .await
        .unwrap();
    let resp = read.next().await.unwrap().unwrap();
    let hello_val: Value = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    assert_eq!(hello_val["result"]["protocolVersion"], 1);

    // Subscribe to events
    let req_sub = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "events.subscribe",
        "params": {}
    });
    write
        .send(Message::Text(req_sub.to_string().into()))
        .await
        .unwrap();
    let _ = read.next().await.unwrap().unwrap();

    // Add eD2K download link
    let ed2k_link = format!(
        "ed2k://|file|fixture_9728001.bin|{}|{}|/",
        file_size, fixture_hash
    );
    let req_add = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "downloads.addEd2kLink",
        "params": {
            "link": ed2k_link,
            "targetDirectory": client_target_dir.path().to_str().unwrap()
        }
    });
    write
        .send(Message::Text(req_add.to_string().into()))
        .await
        .unwrap();

    let resp_add = read.next().await.unwrap().unwrap();
    let add_val: Value = serde_json::from_str(resp_add.to_text().unwrap()).unwrap();
    let download_id = add_val["result"]["downloadId"]
        .as_str()
        .unwrap()
        .to_string();

    // Read initial event
    let _ = read.next().await.unwrap().unwrap();

    // 4. Start eD2K transfer
    let req_start = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "ed2k.startDownload",
        "params": {
            "downloadId": download_id,
            "peerAddr": peer_addr.to_string()
        }
    });
    write
        .send(Message::Text(req_start.to_string().into()))
        .await
        .unwrap();

    // 5. Monitor transfer through to completion
    let mut saw_verifying = false;
    let mut saw_completed = false;

    while let Some(Ok(msg)) = read.next().await {
        let text = msg.to_text().unwrap();
        if let Ok(val) = serde_json::from_str::<Value>(text) {
            if val.get("id").and_then(|id| id.as_u64()) == Some(4) {
                assert_eq!(val["result"]["started"], true);
            } else if val["method"] == "event.downloadUpdated" {
                let dl = &val["params"]["download"];
                let status = dl["status"].as_str().unwrap_or("");
                let received = dl["receivedBytes"].as_u64().unwrap_or(0);

                if status == "verifying" {
                    saw_verifying = true;
                    assert_eq!(received, file_size, "All bytes must be received before verifying");
                } else if status == "completed" {
                    saw_completed = true;
                    assert_eq!(received, file_size);
                    assert_eq!(dl["progress"], 1.0);
                    break;
                }
            }
        }
    }

    assert!(saw_verifying, "Expected verifying state transition");
    assert!(saw_completed, "Expected completed state transition");

    // 6. Verify source summary has 2 parts available
    let req_sources = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "downloads.getSources",
        "params": {
            "downloadId": download_id
        }
    });
    write
        .send(Message::Text(req_sources.to_string().into()))
        .await
        .unwrap();

    let mut sources_val = Value::Null;
    while let Some(Ok(msg)) = read.next().await {
        let text = msg.to_text().unwrap();
        if let Ok(val) = serde_json::from_str::<Value>(text) {
            if val.get("id").and_then(|id| id.as_u64()) == Some(5) {
                sources_val = val;
                break;
            }
        }
    }
    assert!(sources_val.get("result").is_some());
    let src_list = sources_val["result"]["sources"].as_array().unwrap();
    assert!(!src_list.is_empty());
    let src_item = &src_list[0];
    assert_eq!(src_item["availablePartCount"], 2);
    assert_eq!(src_item["totalPartCount"], 2);
    assert!(src_item.get("peerAddr").is_none(), "Raw SocketAddr must NEVER be exposed");

    // 7. Verify finalized file on disk
    let finalized_path = client_target_dir.path().join("fixture_9728001.bin");
    assert!(finalized_path.exists(), "Finalized file must exist");
    assert_eq!(
        std::fs::metadata(&finalized_path).unwrap().len(),
        file_size,
        "Finalized file size must be exactly 9 728 001 bytes"
    );

    let mut target_file = std::fs::File::open(&finalized_path).unwrap();
    let final_ed2k = mule_format::ed2k::hash_reader(&mut target_file).unwrap();
    assert_eq!(
        final_ed2k.root_hash, fixture_hash,
        "Final eD2K root hash must match fixture_one_part_plus_one (695b81b1727666818988524bd7fc823c)"
    );

    // Verify byte-for-byte fidelity
    let mut finalized_bytes = Vec::with_capacity(file_size as usize);
    target_file = std::fs::File::open(&finalized_path).unwrap();
    target_file.read_to_end(&mut finalized_bytes).unwrap();
    assert_eq!(finalized_bytes, fixture_bytes.as_ref());

    // Clean shutdown
    client_daemon.stop().await;
    peer_stopped.store(true, Ordering::SeqCst);
    peer_handle.abort();
}

