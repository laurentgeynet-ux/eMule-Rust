// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use std::io::Read;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use mule_daemon::lab::LabSourceService;
use mule_daemon::{create_router, AppState, DaemonConfig, DirectoryLock, DownloadService};
use mule_format::fixtures::{PatternReader, FIXTURE_EXACT_ONE_PART, REPEATING_0_TO_255};
use mule_format::Ed2kHash;
use mule_protocol::lab::LabSourceEngine;
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
async fn test_inter_daemon_loopback_transfer_and_interruption_resumption() {
    // 1. Prepare isolated directories
    let dir_source = tempdir().unwrap();
    let dir_client = tempdir().unwrap();
    let client_target_dir = tempdir().unwrap();

    let fixture_hash = Ed2kHash::from_hex(FIXTURE_EXACT_ONE_PART.expected_ed2k_hex).unwrap();

    // Generate exact fixture data (9 728 000 bytes)
    let mut pattern_reader = PatternReader::new(&REPEATING_0_TO_255, FIXTURE_FILE_SIZE);
    let mut fixture_data = Vec::with_capacity(FIXTURE_FILE_SIZE as usize);
    pattern_reader.read_to_end(&mut fixture_data).unwrap();
    let fixture_bytes = Bytes::from(fixture_data);

    // 2. Start Source Daemon (Daemon A)
    let _source_lock = DirectoryLock::acquire(dir_source.path()).unwrap();
    let mut engine = LabSourceEngine::new();
    engine.host_file(fixture_hash.0, "fixture_9728000.bin", fixture_bytes.clone());
    let source_service = LabSourceService::start(engine).await.unwrap();
    let source_addr = source_service.local_addr();

    // 3. Start Client Daemon (Daemon B)
    let client_daemon = RunningDaemon::start(dir_client.path().to_path_buf()).await;
    let client_addr = client_daemon.addr;

    // Connect WebSocket to Client Daemon
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

    // Add ed2k download for the fixture
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

    // Collect response and initial event — le serveur multiplexe réponses et
    // events broadcast (downloadAdded/downloadUpdated) via select!, sans ordre
    // garanti : lire jusqu'à avoir les deux attendus.
    let mut add_val: Option<Value> = None;
    let mut evt_val: Option<Value> = None;
    for _ in 0..16 {
        if add_val.is_some() && evt_val.is_some() {
            break;
        }
        let msg = read.next().await.unwrap().unwrap();
        let val: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        if val["id"] == 3 {
            add_val = Some(val);
        } else if val["method"] == "event.downloadUpdated" {
            evt_val = Some(val);
        }
    }
    let add_val = add_val.expect("add response not received");
    let evt_val = evt_val.expect("initial downloadUpdated event not received");
    let download_id = add_val["result"]["downloadId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(add_val["result"]["status"], "queued");
    assert_eq!(evt_val["params"]["download"]["status"], "queued");

    // 4. Attach Source Daemon via lab.addPeerSource
    let req_source = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "lab.addPeerSource",
        "params": {
            "downloadId": download_id,
            "peerAddr": source_addr.to_string()
        }
    });
    write
        .send(Message::Text(req_source.to_string().into()))
        .await
        .unwrap();
    let mut src_val: Option<Value> = None;
    for _ in 0..16 {
        let msg = read.next().await.unwrap().unwrap();
        let val: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        if val["id"] == 4 {
            src_val = Some(val);
            break;
        }
    }
    let src_val = src_val.expect("addPeerSource response not received");
    assert_eq!(src_val["result"]["started"], true);

    // 5. Monitor progress until ~50% (at least 4.5 MB), verifying transitions
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

    assert!(
        saw_waiting_for_sources,
        "Expected waiting_for_sources state transition"
    );
    assert!(saw_downloading, "Expected downloading state transition");
    assert!(
        (4_500_000..FIXTURE_FILE_SIZE).contains(&received_at_interrupt),
        "Interrupted at ~50%: received {} bytes",
        received_at_interrupt
    );

    // 6. Abruptly kill Client Daemon (crash simulation)
    drop(write);
    drop(read);
    client_daemon.stop().await;

    // Short pause to ensure socket and lock cleanup on Windows
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 7. Restart Client Daemon on the EXACT SAME workdir!
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

    // Verify persisted state via downloads.list
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
    let mut list_val: Option<Value> = None;
    for _ in 0..16 {
        let msg = read_2.next().await.unwrap().unwrap();
        let val: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        if val["id"] == 11 {
            list_val = Some(val);
            break;
        }
    }
    let list_val = list_val.expect("downloads.list response not received");
    let dls = list_val["result"]["downloads"].as_array().unwrap();
    assert_eq!(dls.len(), 1, "Exactly one download restored from SQLite");
    let restored_dl = &dls[0];
    assert_eq!(restored_dl["id"], download_id);
    let restored_received = restored_dl["receivedBytes"].as_u64().unwrap();
    assert!(
        (received_at_interrupt..FIXTURE_FILE_SIZE).contains(&restored_received),
        "Restored receivedBytes ({restored_received}) is >= observed interrupt ({received_at_interrupt}) and < total ({FIXTURE_FILE_SIZE})"
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

    // 8. Resume transfer by re-attaching source
    let req_resume = json!({
        "jsonrpc": "2.0",
        "id": 13,
        "method": "lab.addPeerSource",
        "params": {
            "downloadId": download_id,
            "peerAddr": source_addr.to_string()
        }
    });
    write_2
        .send(Message::Text(req_resume.to_string().into()))
        .await
        .unwrap();
    let mut resume_got = false;
    for _ in 0..16 {
        let msg = read_2.next().await.unwrap().unwrap();
        let val: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        if val["id"] == 13 {
            resume_got = true;
            break;
        }
    }
    assert!(resume_got, "resume response not received");

    // 9. Collect events through Verifying and Completed
    let mut saw_verifying = false;
    let mut saw_completed = false;

    while let Some(Ok(msg)) = read_2.next().await {
        let text = msg.to_text().unwrap();
        if let Ok(evt) = serde_json::from_str::<Value>(text) {
            if evt["method"] == "event.downloadUpdated" {
                let dl = &evt["params"]["download"];
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

    // 10. Verify finalized file on disk
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
        "Final eD2K hash must match fixture_exact_one_part exactly"
    );

    // Clean shutdown
    client_daemon_2.stop().await;
    source_service.stop();
}
