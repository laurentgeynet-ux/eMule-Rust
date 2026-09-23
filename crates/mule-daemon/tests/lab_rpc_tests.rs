// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use futures_util::{SinkExt, StreamExt};
use mule_daemon::{create_router, AppState, DaemonConfig, DownloadService, WorkDirMode};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tempfile::tempdir;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

async fn spawn_test_daemon(mode: WorkDirMode) -> (SocketAddr, String, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "lab_test_token_456".to_string();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_bind_addr(addr)
        .with_workdir_mode(mode);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);

    let app = create_router(state);

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (addr, token, dir)
}

#[tokio::test]
async fn test_lab_rpc_full_lifecycle() {
    let (addr, token, dir) = spawn_test_daemon(WorkDirMode::Portable).await;
    let ws_url = format!("ws://{addr}/rpc");

    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (mut write, mut read) = ws_stream.split();

    // 1. Handshake
    let req_hello = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "system.hello",
        "params": {
            "protocolVersion": 1,
            "token": token
        }
    });
    write
        .send(Message::Text(req_hello.to_string().into()))
        .await
        .unwrap();

    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(resp["result"]["workdirMode"], "Portable");
    assert_eq!(resp["result"]["daemonVersion"], env!("CARGO_PKG_VERSION"));

    // 2. lab.status initially has no fixtures and no active peer
    let req_status = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "lab.status",
        "params": {}
    });
    write
        .send(Message::Text(req_status.to_string().into()))
        .await
        .unwrap();

    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(resp["result"]["isPortable"], true);
    assert!(resp["result"]["activeLoopbackPeer"].is_null());
    assert_eq!(resp["result"]["fixtures"].as_array().unwrap().len(), 0);

    // 3. lab.createFixture creates sample_1mb
    let req_create = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "lab.createFixture",
        "params": {
            "fixtureType": "sample_1mb"
        }
    });
    write
        .send(Message::Text(req_create.to_string().into()))
        .await
        .unwrap();

    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    let fixture_link = resp["result"]["ed2kLink"].as_str().unwrap();
    assert!(fixture_link.starts_with("ed2k://|file|sample_1mb.bin|1048576|"));
    assert_eq!(resp["result"]["sizeBytes"], 1048576);

    // 4. Verify file was created on disk
    let fixture_path = dir.path().join("fixtures").join("sample_1mb.bin");
    assert!(fixture_path.exists());
    assert_eq!(std::fs::metadata(&fixture_path).unwrap().len(), 1048576);

    // 5. lab.startLoopbackPeer starts the loopback peer and shares the fixture
    let req_start_peer = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "lab.startLoopbackPeer",
        "params": {}
    });
    write
        .send(Message::Text(req_start_peer.to_string().into()))
        .await
        .unwrap();

    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    let peer_addr = resp["result"]["peerAddr"].as_str().unwrap();
    assert!(peer_addr.starts_with("127.0.0.1:"));
    assert_eq!(resp["result"]["sharedCount"], 1);
    assert_eq!(resp["result"]["status"], "running");

    // 6. lab.status now reports the active loopback peer
    let req_status_2 = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "lab.status",
        "params": {}
    });
    write
        .send(Message::Text(req_status_2.to_string().into()))
        .await
        .unwrap();

    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(resp["result"]["activeLoopbackPeer"], peer_addr);
    assert_eq!(resp["result"]["fixtures"].as_array().unwrap().len(), 1);

    // 7. lab.stopLoopbackPeer stops the peer
    let req_stop_peer = json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "lab.stopLoopbackPeer",
        "params": {}
    });
    write
        .send(Message::Text(req_stop_peer.to_string().into()))
        .await
        .unwrap();

    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(resp["result"]["stopped"], true);

    // 8. lab.cleanupTestData purges fixtures in Portable mode
    let req_clean = json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "lab.cleanupTestData",
        "params": {}
    });
    write
        .send(Message::Text(req_clean.to_string().into()))
        .await
        .unwrap();

    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(resp["result"]["success"], true);
    assert_eq!(resp["result"]["deletedFilesCount"], 1);
    assert!(resp["result"]["freedBytes"].as_u64().unwrap() > 0);
    assert!(resp["result"]["cleanedFolders"]
        .as_array()
        .unwrap()
        .contains(&json!("fixtures")));
    assert!(!fixture_path.exists());
}

#[tokio::test]
async fn test_lab_cleanup_refused_in_installed_default_mode() {
    let (addr, token, _dir) = spawn_test_daemon(WorkDirMode::InstalledDefault).await;
    let ws_url = format!("ws://{addr}/rpc");

    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (mut write, mut read) = ws_stream.split();

    // Handshake
    let req_hello = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "system.hello",
        "params": {
            "protocolVersion": 1,
            "token": token
        }
    });
    write
        .send(Message::Text(req_hello.to_string().into()))
        .await
        .unwrap();
    let _ = read.next().await;

    // Cleanup attempt must fail with business error
    let req_clean = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "lab.cleanupTestData",
        "params": {}
    });
    write
        .send(Message::Text(req_clean.to_string().into()))
        .await
        .unwrap();

    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert!(resp["error"].is_object());
    assert_eq!(resp["error"]["code"], -32002);
    assert!(resp["error"]["message"]
        .as_str()
        .unwrap()
        .contains("InstalledDefault"));
}

#[tokio::test]
async fn test_lab_create_fixture_pattern_9728001() {
    let (addr, token, dir) = spawn_test_daemon(WorkDirMode::Portable).await;
    let ws_url = format!("ws://{addr}/rpc");

    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (mut write, mut read) = ws_stream.split();

    // Handshake
    let req_hello = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "system.hello",
        "params": { "protocolVersion": 1, "token": token }
    });
    write
        .send(Message::Text(req_hello.to_string().into()))
        .await
        .unwrap();
    let _ = read.next().await;

    // Create pattern_9728001 (1 part + 1 byte)
    let req_create = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "lab.createFixture",
        "params": {
            "fixtureType": "pattern_9728001"
        }
    });
    write
        .send(Message::Text(req_create.to_string().into()))
        .await
        .unwrap();

    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(resp["result"]["fileName"], "fixture_part_9728001.bin");
    assert_eq!(resp["result"]["sizeBytes"], 9_728_001);
    let link = resp["result"]["ed2kLink"].as_str().unwrap();
    assert!(link.starts_with("ed2k://|file|fixture_part_9728001.bin|9728001|"));

    let file_path = dir.path().join("fixtures").join("fixture_part_9728001.bin");
    assert!(file_path.exists());
    assert_eq!(std::fs::metadata(&file_path).unwrap().len(), 9_728_001);
}
