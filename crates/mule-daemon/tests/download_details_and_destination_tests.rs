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
    assert!(detail["ed2kLink"].as_str().unwrap().contains("ed2k://|file|boundary_test.bin|9728001|31d6cfe0d16ae931b73c59d7e0c089c0|/"));

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
