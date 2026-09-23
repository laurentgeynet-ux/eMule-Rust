//! Integration test verifying eD2K source availability discovery on loopback without block requests or .part writes.
//! Phase 6.3: Strictly confined to 127.0.0.1.

use futures_util::{SinkExt, StreamExt};
use mule_daemon::{create_router, AppState, DaemonConfig, DownloadService};
use mule_protocol::ed2k::{Ed2kTestPeer, PartAvailabilityMask, TestPeerMode};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tempfile::tempdir;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

async fn spawn_test_daemon() -> (SocketAddr, String, tempfile::TempDir, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = tempdir().unwrap();
    let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token).with_bind_addr(addr);
    let downloads = DownloadService::in_memory(temp_dir.path().to_path_buf()).unwrap();
    let state = AppState::new(config, downloads);

    let app = create_router(state);

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (addr, token, dir, temp_dir)
}

#[tokio::test]
async fn test_daemon_ed2k_loopback_discovers_source_without_part_file_writes() {
    let file_hash = [0x33; 16];
    let file_hash_hex = hex::encode(file_hash);

    // 1. Spawn controlled eD2K test peer sharing a 5-part file with parts 0, 2, 4 available
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    test_peer
        .add_shared_file(
            file_hash,
            5,
            PartAvailabilityMask::from_parts(&[0, 2, 4], 5),
        )
        .await;
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    // 2. Spawn test daemon
    let (daemon_addr, token, _data_dir, temp_dir) = spawn_test_daemon().await;
    let ws_url = format!("ws://{daemon_addr}/rpc");

    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (mut write, mut read) = ws_stream.split();

    // 3. Authenticate with daemon
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

    let resp_hello = read.next().await.unwrap().unwrap();
    let resp_val: Value = serde_json::from_str(&resp_hello.to_string()).unwrap();
    assert!(resp_val["result"]["daemonVersion"].is_string());

    // 4. Add eD2K download link (5 parts = 48,640,000 bytes)
    let ed2k_link = format!("ed2k://|file|test_discovery.bin|48640000|{file_hash_hex}|/");
    let req_add = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "downloads.addEd2kLink",
        "params": {
            "link": ed2k_link,
            "policy": "none"
        }
    });
    write
        .send(Message::Text(req_add.to_string().into()))
        .await
        .unwrap();

    let mut add_resp_opt = None;
    let mut initial_event_opt = None;
    // Le serveur multiplexe réponses et events broadcast (event.downloadAdded,
    // event.downloadUpdated) via select! : lire jusqu'à avoir les deux attendus.
    for _ in 0..16 {
        if add_resp_opt.is_some() && initial_event_opt.is_some() {
            break;
        }
        let msg = read.next().await.unwrap().unwrap();
        let val: Value = serde_json::from_str(&msg.to_string()).unwrap();
        if val["id"] == 2 {
            add_resp_opt = Some(val);
        } else if val["method"] == "event.downloadUpdated" {
            initial_event_opt = Some(val);
        }
    }

    let add_val = add_resp_opt.expect("add response not received");
    let download_id = add_val["result"]["downloadId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(add_val["result"]["status"], "queued");

    let initial_event = initial_event_opt.expect("initial event not received");
    assert_eq!(initial_event["params"]["download"]["sourceCount"], 0);
    assert_eq!(initial_event["params"]["download"]["receivedBytes"], 0);

    // 5. Verify the .part file on disk exists and has 0 bytes written (AllocationPolicy::None)
    let part_file_path = temp_dir.path().join(format!("{download_id}.part"));
    let size_before = std::fs::metadata(&part_file_path)
        .map(|m| m.len())
        .unwrap_or(0);
    assert_eq!(
        size_before, 0,
        ".part file must have 0 bytes initially with AllocationPolicy::None"
    );

    // 6. Call ed2k.probeSource to probe peer availability over loopback
    let req_probe = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "ed2k.probeSource",
        "params": {
            "downloadId": download_id,
            "peerAddr": peer_addr.to_string()
        }
    });
    write
        .send(Message::Text(req_probe.to_string().into()))
        .await
        .unwrap();

    let mut probe_resp_opt = None;
    let mut update_event_opt = None;

    // Read responses/events (could arrive in either order: response vs broadcast event)
    for _ in 0..16 {
        if probe_resp_opt.is_some() && update_event_opt.is_some() {
            break;
        }
        let msg = read.next().await.unwrap().unwrap();
        let val: Value = serde_json::from_str(&msg.to_string()).unwrap();
        if val["id"] == 3 {
            probe_resp_opt = Some(val);
        } else if val["method"] == "event.downloadUpdated" {
            update_event_opt = Some(val);
        }
    }

    // 7. Verify probe result
    let probe_resp = probe_resp_opt.expect("probe response not received");
    assert_eq!(probe_resp["result"]["success"], true);
    assert_eq!(probe_resp["result"]["sourceCount"], 1);
    assert_eq!(probe_resp["result"]["partCount"], 5);
    assert_eq!(probe_resp["result"]["availableParts"], json!([0, 2, 4]));

    // 8. Verify broadcast update event
    let update_event = update_event_opt.expect("downloadUpdated event not received");
    let dl_event = &update_event["params"]["download"];
    assert_eq!(dl_event["sourceCount"], 1);
    assert_eq!(dl_event["receivedBytes"], 0);
    assert_eq!(dl_event["status"], "queued");

    // 9. CRITICAL INVARIANCE: Verify strictly ZERO writes to the .part file
    let size_after = std::fs::metadata(&part_file_path)
        .map(|m| m.len())
        .unwrap_or(0);
    assert_eq!(
        size_after, size_before,
        "CRITICAL: .part file must NOT have any data written during availability probing!"
    );

    // 10. Verify downloads.get returns updated sourceCount = 1 and 0 bytes received
    let req_get = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "downloads.get",
        "params": {
            "id": download_id
        }
    });
    write
        .send(Message::Text(req_get.to_string().into()))
        .await
        .unwrap();

    let resp_get = read.next().await.unwrap().unwrap();
    let get_val: Value = serde_json::from_str(&resp_get.to_string()).unwrap();
    assert_eq!(get_val["result"]["sourceCount"], 1);
    assert_eq!(get_val["result"]["receivedBytes"], 0);
    assert_eq!(get_val["result"]["status"], "queued");

    // 11. Verify non-loopback address is strictly rejected
    let req_forbidden = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "ed2k.probeSource",
        "params": {
            "downloadId": download_id,
            "peerAddr": "198.51.100.1:39735"
        }
    });
    write
        .send(Message::Text(req_forbidden.to_string().into()))
        .await
        .unwrap();

    let resp_forbidden = read.next().await.unwrap().unwrap();
    let forbidden_val: Value = serde_json::from_str(&resp_forbidden.to_string()).unwrap();
    assert!(forbidden_val["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Non-loopback"));
}
