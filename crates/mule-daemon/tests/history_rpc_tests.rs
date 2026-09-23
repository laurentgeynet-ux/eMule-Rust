// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use futures_util::{SinkExt, StreamExt};
use mule_daemon::{create_router, AppState, DaemonConfig, DownloadService, WorkDirMode};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tempfile::tempdir;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

async fn spawn_test_daemon() -> (SocketAddr, String, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "history_test_token_456".to_string();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_bind_addr(addr)
        .with_workdir_mode(WorkDirMode::Portable);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);

    let app = create_router(state);

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (addr, token, dir)
}

async fn send_json(
    write: &mut (impl SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin),
    val: Value,
) {
    write
        .send(Message::Text(val.to_string().into()))
        .await
        .unwrap();
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
async fn test_history_rpc_lifecycle_stats_export_and_retention() {
    let (addr, token, _dir) = spawn_test_daemon().await;
    let ws_url = format!("ws://{addr}/rpc");

    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (mut write, mut read) = ws_stream.split();

    // 1. Handshake: verify 'history' capability is advertised
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "system.hello",
            "params": {
                "protocolVersion": 1,
                "token": token
            }
        }),
    )
    .await;
    let res_hello = recv_response_by_id(&mut read, 1).await;
    let caps = res_hello["result"]["capabilities"]
        .as_array()
        .expect("capabilities array");
    assert!(
        caps.iter().any(|c| c.as_str() == Some("history")),
        "history capability missing from capabilities: {:?}",
        caps
    );

    // 2. Initial history list should be empty
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "history.list",
            "params": {}
        }),
    )
    .await;
    let res_hist_empty = recv_response_by_id(&mut read, 2).await;
    let empty_list = res_hist_empty["result"]
        .as_array()
        .expect("history list array");
    assert_eq!(empty_list.len(), 0);

    // 3. Check settings.get returns historyRetentionDays
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "settings.get"
        }),
    )
    .await;
    let res_settings = recv_response_by_id(&mut read, 3).await;
    assert_eq!(
        res_settings["result"]["historyRetentionDays"].as_u64(),
        Some(30)
    );

    // 4. Add an ed2k link
    let link = "ed2k://|file|test_history.iso|1024|31d6cfe0d16ae931b73c59d7e0c089c0|/";
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "downloads.addEd2kLink",
            "params": {
                "link": link
            }
        }),
    )
    .await;
    let res_add = recv_response_by_id(&mut read, 4).await;
    let dl_id = res_add["result"]["downloadId"]
        .as_str()
        .expect("downloadId")
        .to_string();

    // 5. Pause and resume download (should record 'resumed' history event)
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "downloads.pause",
            "params": { "id": dl_id }
        }),
    )
    .await;
    let _ = recv_response_by_id(&mut read, 5).await;

    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "downloads.resume",
            "params": { "id": dl_id }
        }),
    )
    .await;
    let _ = recv_response_by_id(&mut read, 6).await;

    // 6. Verify history.list now contains the 'resumed' event
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "history.list",
            "params": {}
        }),
    )
    .await;
    let res_hist_resume = recv_response_by_id(&mut read, 7).await;
    let list_resume = res_hist_resume["result"].as_array().expect("history list");
    assert_eq!(list_resume.len(), 1);
    assert_eq!(list_resume[0]["eventType"], "resumed");
    assert_eq!(list_resume[0]["downloadId"], dl_id);
    assert_eq!(list_resume[0]["fileName"], "test_history.iso");
    assert_eq!(list_resume[0]["resumeCount"], 1);

    // 7. Remove download (should record 'removed' history event)
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "downloads.remove",
            "params": {
                "id": dl_id,
                "deleteFiles": true
            }
        }),
    )
    .await;
    let _ = recv_response_by_id(&mut read, 8).await;

    // Verify history now has both 'removed' and 'resumed'
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "history.list",
            "params": {}
        }),
    )
    .await;
    let res_hist_remove = recv_response_by_id(&mut read, 9).await;
    let list_remove = res_hist_remove["result"].as_array().expect("history list");
    assert_eq!(list_remove.len(), 2);
    assert_eq!(list_remove[0]["eventType"], "removed");
    assert_eq!(list_remove[1]["eventType"], "resumed");

    // 8. Test history.stats aggregation
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "history.stats"
        }),
    )
    .await;
    let res_stats = recv_response_by_id(&mut read, 10).await;
    let stats = &res_stats["result"];
    assert_eq!(stats["totalResumesCount"], 1);
    assert_eq!(stats["totalCompletedCount"], 0);

    // 9. Test history.export in JSON format
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "history.export",
            "params": {
                "format": "json"
            }
        }),
    )
    .await;
    let res_exp_json = recv_response_by_id(&mut read, 11).await;
    assert_eq!(res_exp_json["result"]["format"], "json");
    assert_eq!(res_exp_json["result"]["count"], 2);
    let json_content = res_exp_json["result"]["content"]
        .as_str()
        .expect("json content string");
    assert!(json_content.contains("test_history.iso"));

    // 10. Test history.export in CSV format
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "history.export",
            "params": {
                "format": "csv"
            }
        }),
    )
    .await;
    let res_exp_csv = recv_response_by_id(&mut read, 12).await;
    assert_eq!(res_exp_csv["result"]["format"], "csv");
    assert_eq!(res_exp_csv["result"]["count"], 2);
    let csv_content = res_exp_csv["result"]["content"]
        .as_str()
        .expect("csv content string");
    assert!(csv_content.starts_with("id,download_id,file_name"));
    assert!(csv_content.contains("test_history.iso"));

    // 11. Test settings.setRetentionDays
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "settings.setRetentionDays",
            "params": {
                "retentionDays": 7
            }
        }),
    )
    .await;
    let res_set_ret = recv_response_by_id(&mut read, 13).await;
    assert_eq!(res_set_ret["result"]["success"], true);
    assert_eq!(res_set_ret["result"]["retentionDays"], 7);

    // Verify settings.get reflects updated retention
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 14,
            "method": "settings.get"
        }),
    )
    .await;
    let res_settings_2 = recv_response_by_id(&mut read, 14).await;
    assert_eq!(
        res_settings_2["result"]["historyRetentionDays"].as_u64(),
        Some(7)
    );

    // 12. Test history.clear
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 15,
            "method": "history.clear"
        }),
    )
    .await;
    let res_clear = recv_response_by_id(&mut read, 15).await;
    assert_eq!(res_clear["result"]["success"], true);
    assert_eq!(res_clear["result"]["clearedCount"], 2);

    // Verify history.list is now empty
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 16,
            "method": "history.list",
            "params": {}
        }),
    )
    .await;
    let res_hist_after_clear = recv_response_by_id(&mut read, 16).await;
    let list_after_clear = res_hist_after_clear["result"]
        .as_array()
        .expect("history list");
    assert_eq!(list_after_clear.len(), 0);
}
