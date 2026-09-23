// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

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
    let token = "details_test_token_123".to_string();

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

async fn recv_response_by_id(
    read: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    expected_id: u64,
) -> Value {
    for _ in 0..10 {
        if let Some(Ok(msg)) = read.next().await {
            if let Ok(text) = msg.to_text() {
                if let Ok(val) = serde_json::from_str::<Value>(text) {
                    if val.get("id").and_then(|i| i.as_u64()) == Some(expected_id) {
                        return val;
                    }
                }
            }
        }
    }
    panic!("Did not receive response for request id {expected_id}");
}

#[tokio::test]
async fn test_download_details_part_map_and_boundaries() {
    let (addr, token, _dir) = spawn_test_daemon(WorkDirMode::Portable).await;
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
    let resp = recv_response_by_id(&mut read, 1).await;
    assert_eq!(resp["result"]["workdirMode"], "Portable");

    // 2. Add 9_728_001 bytes file (1 full part of 9_728_000 + 1 residual byte part of 1 byte)
    let link_9728001 = "ed2k://|file|boundary_test.bin|9728001|31d6cfe0d16ae931b73c59d7e0c089c0|/";
    let req_add = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "downloads.addEd2kLink",
        "params": {
            "link": link_9728001
        }
    });
    write
        .send(Message::Text(req_add.to_string().into()))
        .await
        .unwrap();
    let resp = recv_response_by_id(&mut read, 2).await;
    let dl_id = resp["result"]["downloadId"].as_str().unwrap().to_string();

    // 3. Query details via downloads.get
    let req_get = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "downloads.get",
        "params": {
            "id": dl_id
        }
    });
    write
        .send(Message::Text(req_get.to_string().into()))
        .await
        .unwrap();
    let resp = recv_response_by_id(&mut read, 3).await;
    let detail = resp["result"].clone();

    // Verify fields
    assert_eq!(detail["fileName"], "boundary_test.bin");
    assert_eq!(detail["totalBytes"], 9728001);
    assert_eq!(detail["remainingBytes"], 9728001);
    assert_eq!(detail["ed2kHash"], "31d6cfe0d16ae931b73c59d7e0c089c0");
    assert!(detail["ed2kLink"]
        .as_str()
        .unwrap()
        .contains("ed2k://|file|boundary_test.bin|9728001|31d6cfe0d16ae931b73c59d7e0c089c0|/"));

    // Verify parts map: exactly 2 parts
    let parts = detail["parts"].as_array().expect("parts array");
    assert_eq!(parts.len(), 2);

    // Part 0: full part [0, 9728000)
    assert_eq!(parts[0]["partIndex"], 0);
    assert_eq!(parts[0]["startOffset"], 0);
    assert_eq!(parts[0]["endOffset"], 9728000);
    assert_eq!(parts[0]["size"], 9728000);
    assert_eq!(parts[0]["receivedBytes"], 0);
    assert_eq!(parts[0]["state"], "missing");

    // Part 1: residual part [9728000, 9728001)
    assert_eq!(parts[1]["partIndex"], 1);
    assert_eq!(parts[1]["startOffset"], 9728000);
    assert_eq!(parts[1]["endOffset"], 9728001);
    assert_eq!(parts[1]["size"], 1);
    assert_eq!(parts[1]["receivedBytes"], 0);
    assert_eq!(parts[1]["state"], "missing");
}

#[tokio::test]
async fn test_downloads_set_destination_and_retry_connection() {
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
        "params": { "protocolVersion": 1, "token": token }
    });
    write
        .send(Message::Text(req_hello.to_string().into()))
        .await
        .unwrap();
    let _ = recv_response_by_id(&mut read, 1).await;

    // 2. Add sample download
    let link = "ed2k://|file|test_dest.txt|100|0123456789abcdef0123456789abcdef|/";
    let req_add = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "downloads.addEd2kLink",
        "params": { "link": link }
    });
    write
        .send(Message::Text(req_add.to_string().into()))
        .await
        .unwrap();
    let resp = recv_response_by_id(&mut read, 2).await;
    let dl_id = resp["result"]["downloadId"].as_str().unwrap().to_string();

    // 3. Change destination
    let custom_dest_dir = dir.path().join("my_custom_folder");
    let req_set_dest = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "downloads.setDestination",
        "params": {
            "id": dl_id,
            "targetDirectory": custom_dest_dir.to_string_lossy()
        }
    });
    write
        .send(Message::Text(req_set_dest.to_string().into()))
        .await
        .unwrap();
    let resp = recv_response_by_id(&mut read, 3).await;
    let detail = resp["result"].clone();
    assert!(detail["targetPath"]
        .as_str()
        .unwrap()
        .contains("my_custom_folder"));
    // The tempPath must remain untouched in data/temp
    assert!(detail["tempPath"].as_str().unwrap().contains(".part"));

    // 4. Retry connection
    let req_retry = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "downloads.retryConnection",
        "params": { "id": dl_id }
    });
    write
        .send(Message::Text(req_retry.to_string().into()))
        .await
        .unwrap();
    let resp = recv_response_by_id(&mut read, 4).await;
    assert_eq!(resp["result"]["connectionAttempts"], 1);
}

#[tokio::test]
async fn test_settings_rpc_get_set_reset() {
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
        "params": { "protocolVersion": 1, "token": token }
    });
    write
        .send(Message::Text(req_hello.to_string().into()))
        .await
        .unwrap();
    let _ = recv_response_by_id(&mut read, 1).await;

    // 2. Query initial settings
    let req_get = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "settings.get"
    });
    write
        .send(Message::Text(req_get.to_string().into()))
        .await
        .unwrap();
    let resp = recv_response_by_id(&mut read, 2).await;
    assert_eq!(resp["result"]["isCustom"], false);

    // 3. Set custom download directory
    let custom_dl = dir.path().join("custom_downloads_global");
    let req_set = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "settings.set",
        "params": {
            "defaultDownloadDir": custom_dl.to_string_lossy()
        }
    });
    write
        .send(Message::Text(req_set.to_string().into()))
        .await
        .unwrap();
    let resp = recv_response_by_id(&mut read, 3).await;
    assert_eq!(resp["result"]["success"], true);

    // 4. Query settings again to confirm updated
    let req_get2 = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "settings.get"
    });
    write
        .send(Message::Text(req_get2.to_string().into()))
        .await
        .unwrap();
    let resp = recv_response_by_id(&mut read, 4).await;
    assert_eq!(resp["result"]["isCustom"], true);
    assert_eq!(
        resp["result"]["defaultDownloadDir"].as_str().unwrap(),
        custom_dl.to_string_lossy().as_ref()
    );

    // 5. Reset download directory
    let req_reset = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "settings.resetDownloadDir"
    });
    write
        .send(Message::Text(req_reset.to_string().into()))
        .await
        .unwrap();
    let resp = recv_response_by_id(&mut read, 5).await;
    assert_eq!(resp["result"]["isCustom"], false);
}

#[test]
fn test_get_pending_verifications_does_not_trigger_on_sparse_file_length() {
    let dir = tempfile::tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let target_dir = dir.path().join("incoming");
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::create_dir_all(&target_dir).unwrap();

    // The file hash must match the injected data: the incremental part-hash
    // check in inject_range uses the file hash as part hash for files smaller
    // than one eD2K part, and evicts the range on mismatch.
    let tail = vec![0xaa; 1000];
    let head = vec![0xbb; 999_000];
    let mut full = head.clone();
    full.extend_from_slice(&tail);
    let mut hasher = mule_format::ed2k::Ed2kHasher::new();
    hasher.update(&full);
    let file_hash = hasher.finalize().unwrap().root_hash.to_hex();

    let mut service = DownloadService::in_memory(temp_dir).unwrap();
    let ed2k = format!("ed2k://|file|test.bin|1000000|{file_hash}|/");
    let ed2k = ed2k.as_str();
    let snap = service
        .add_ed2k_link(
            ed2k,
            &target_dir,
            mule_core::storage::AllocationPolicy::None,
            None,
        )
        .unwrap();

    // Simulate writing only the last 1,000 bytes (offset 999_000 to 1_000_000).
    // On NTFS/filesystems, this extends the file length on disk to 1_000_000 bytes!
    let _ = service.inject_range(&snap.id, 999_000, &tail).unwrap();

    // Verify that covered_len is only 1,000 bytes
    assert_eq!(service.get_covered_len(&snap.id), Some(1000));

    // Even though file length on disk is 1,000,000 bytes (total_size),
    // get_pending_verifications MUST NOT queue it for verification because ranges are incomplete!
    let pending = service.get_pending_verifications();
    assert!(
        pending.is_empty(),
        "Sparse file must not trigger premature verification!"
    );

    // Now inject the remaining 0..999_000 bytes
    let _ = service.inject_range(&snap.id, 0, &head).unwrap();
    assert_eq!(service.get_covered_len(&snap.id), Some(1_000_000));

    // Now that all 1,000,000 bytes are covered, it MUST trigger verification!
    let pending = service.get_pending_verifications();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].0, snap.id);
}

#[test]
fn test_add_duplicate_ed2k_link_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let target_dir = dir.path().join("incoming");
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::create_dir_all(&target_dir).unwrap();

    let mut service = DownloadService::in_memory(temp_dir).unwrap();
    let ed2k = "ed2k://|file|sample.txt|1000|abcdefabcdefabcdefabcdefabcdefab|/";
    let res1 = service.add_ed2k_link(
        ed2k,
        &target_dir,
        mule_core::storage::AllocationPolicy::None,
        None,
    );
    assert!(res1.is_ok(), "First add must succeed");

    // Attempting to add the exact same ed2k hash again must fail
    let ed2k_duplicate = "ed2k://|file|sample_renamed.txt|1000|abcdefabcdefabcdefabcdefabcdefab|/";
    let res2 = service.add_ed2k_link(
        ed2k_duplicate,
        &target_dir,
        mule_core::storage::AllocationPolicy::None,
        None,
    );
    assert!(res2.is_err(), "Duplicate add must be rejected");
    let err = res2.unwrap_err();
    assert!(
        err.to_string()
            .contains("déjà dans la liste des téléchargements"),
        "Error message must indicate duplicate: {}",
        err
    );
}

#[test]
fn test_inject_range_after_completed_does_not_regress_status() {
    let dir = tempfile::tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let target_dir = dir.path().join("incoming");

    let mut service = DownloadService::new(&dir.path().join("mule.db"), temp_dir).unwrap();

    // File hash must match the injected bytes (single-part files verify the
    // part against the file hash on completion).
    let data = vec![0x42; 50000];
    let mut hasher = mule_format::ed2k::Ed2kHasher::new();
    hasher.update(&data);
    let file_hash = hasher.finalize().unwrap().root_hash.to_hex();
    let ed2k = format!("ed2k://|file|packet_test.bin|50000|{file_hash}|/");
    let ed2k = ed2k.as_str();
    let snap = service
        .add_ed2k_link(
            ed2k,
            &target_dir,
            mule_core::storage::AllocationPolicy::None,
            None,
        )
        .unwrap();

    // 1. Inject all 50,000 bytes -> reaches 100% and Verifying
    let snap_verif = service.inject_range(&snap.id, 0, &data).unwrap();
    assert_eq!(snap_verif.status.to_lowercase(), "verifying");

    // 2. Finalize download -> Completed
    let snap_completed = service.finalize_download(&snap.id).unwrap();
    assert_eq!(snap_completed.status.to_lowercase(), "completed");

    // 3. Late duplicate packet arrives from peer session
    let late_data = vec![0x42; 1000];
    let snap_after_late = service.inject_range(&snap.id, 0, &late_data).unwrap();
    assert_eq!(
        snap_after_late.status.to_lowercase(),
        "completed",
        "Late packet must NOT regress Completed download to Verifying!"
    );

    // 4. get_pending_verifications must be empty
    let pending = service.get_pending_verifications();
    assert!(
        pending.is_empty(),
        "Completed download must not be queued for verification"
    );
}

#[test]
fn test_startup_self_heal_completed() {
    let dir = tempfile::tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let target_dir = dir.path().join("incoming");
    let db_path = dir.path().join("mule.db");

    let download_id = {
        let mut service = DownloadService::new(&db_path, temp_dir.clone()).unwrap();
        let ed2k = "ed2k://|file|heal_test.bin|2048|fedcba9876543210fedcba9876543210|/";
        let snap = service
            .add_ed2k_link(
                ed2k,
                &target_dir,
                mule_core::storage::AllocationPolicy::None,
                None,
            )
            .unwrap();

        // Inject full data
        let data = vec![0x11; 2048];
        let _ = service.inject_range(&snap.id, 0, &data).unwrap();
        let _ = service.finalize_download(&snap.id).unwrap();

        // Force status back to verifying in DB to simulate old bug
        service
            .db_mut()
            .update_status(&snap.id, mule_core::model::DownloadStatus::Verifying)
            .unwrap();
        snap.id
    };

    // Reopen service: startup should self-heal since target file exists with size 2048
    let service = DownloadService::new(&db_path, temp_dir).unwrap();
    let dl = service.get_download(&download_id).unwrap();
    assert_eq!(
        dl.status.to_lowercase(),
        "completed",
        "Startup self-healing must restore status to Completed"
    );
}
