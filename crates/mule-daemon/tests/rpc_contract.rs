use futures_util::{SinkExt, StreamExt};
use mule_daemon::{create_router, AppState, DaemonConfig, DownloadService};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tempfile::tempdir;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

async fn spawn_test_daemon() -> (SocketAddr, String, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "secret_test_token_123".to_string();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token).with_bind_addr(addr);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);

    let app = create_router(state);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (addr, token, dir)
}

#[tokio::test]
async fn test_full_rpc_contract_and_event_streaming() {
    let (addr, token, _dir) = spawn_test_daemon().await;
    let ws_url = format!("ws://{addr}/rpc");

    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (mut write, mut read) = ws_stream.split();

    // 1. Calling method before handshake must be rejected
    let req_unauth = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "downloads.list",
        "params": {}
    });
    write
        .send(Message::Text(req_unauth.to_string().into()))
        .await
        .unwrap();

    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(resp["error"]["code"], -32000); // Auth failed

    // 2. Handshake with bad token
    let req_bad_token = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "system.hello",
        "params": {
            "protocolVersion": 1,
            "token": "wrong_token"
        }
    });
    write
        .send(Message::Text(req_bad_token.to_string().into()))
        .await
        .unwrap();
    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(resp["error"]["code"], -32000);

    // 3. Handshake with valid token
    let req_hello = json!({
        "jsonrpc": "2.0",
        "id": 3,
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
    assert!(resp["result"]["instanceId"].is_string());
    assert_eq!(resp["result"]["protocolVersion"], 1);

    // 4. Add eD2K Link
    let valid_link = "ed2k://|file|test_archive.zip|10000|0123456789abcdef0123456789abcdef|/";
    let req_add = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "downloads.addEd2kLink",
        "params": {
            "link": valid_link,
            "targetDirectory": "."
        }
    });
    write
        .send(Message::Text(req_add.to_string().into()))
        .await
        .unwrap();

    // Might receive either the response or the broadcast event
    let mut add_resp: Option<Value> = None;
    let mut add_event: Option<Value> = None;

    for _ in 0..2 {
        let msg = read.next().await.unwrap().unwrap();
        let val: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        if val.get("id").is_some() {
            add_resp = Some(val);
        } else if val.get("method").and_then(|m| m.as_str()) == Some("event.downloadUpdated") {
            add_event = Some(val);
        }
    }

    let add_resp = add_resp.expect("Expected response for addEd2kLink");
    let download_id = add_resp["result"]["downloadId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(add_resp["result"]["status"], "queued");

    let add_event = add_event.expect("Expected event.downloadUpdated notification");
    assert_eq!(add_event["params"]["download"]["id"], download_id);

    // 5. List downloads
    let req_list = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "downloads.list"
    });
    write
        .send(Message::Text(req_list.to_string().into()))
        .await
        .unwrap();
    let msg = read.next().await.unwrap().unwrap();
    let resp: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    let list = resp["result"]["downloads"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], download_id);
    assert_eq!(list[0]["fileName"], "test_archive.zip");

    #[cfg(any(feature = "debug-rpc", feature = "lab-tools"))]
    {
        // 6. Inject Range (simulate data arrival)
        let req_inject = json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "debug.injectRange",
            "params": {
                "id": download_id,
                "offset": 0,
                "dataHex": "1122334455"
            }
        });
        write
            .send(Message::Text(req_inject.to_string().into()))
            .await
            .unwrap();

        let mut inject_resp: Option<Value> = None;
        let mut inject_event: Option<Value> = None;

        for _ in 0..2 {
            let msg = read.next().await.unwrap().unwrap();
            let val: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
            if val.get("id").is_some() {
                inject_resp = Some(val);
            } else if val.get("method").and_then(|m| m.as_str()) == Some("event.downloadUpdated") {
                inject_event = Some(val);
            }
        }

        let inject_resp = inject_resp.expect("Expected inject response");
        assert_eq!(inject_resp["result"]["receivedBytes"], 5);

        let inject_event = inject_event.expect("Expected inject event");
        assert_eq!(inject_event["params"]["download"]["receivedBytes"], 5);
        assert!(inject_event["params"]["sequence"].as_u64().unwrap() > 0);
    }

    // 7. Pause and Resume
    let req_pause = json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "downloads.pause",
        "params": { "id": download_id }
    });
    write
        .send(Message::Text(req_pause.to_string().into()))
        .await
        .unwrap();

    let mut pause_resp = None;
    for _ in 0..2 {
        let msg = read.next().await.unwrap().unwrap();
        let val: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        if val.get("id").is_some() {
            pause_resp = Some(val);
            break;
        }
    }
    assert_eq!(pause_resp.unwrap()["result"]["status"], "paused");

    // 8. Test system.stop
    let req_stop = json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "system.stop",
        "params": {}
    });
    write
        .send(Message::Text(req_stop.to_string().into()))
        .await
        .unwrap();

    let mut stop_resp = None;
    for _ in 0..2 {
        let msg = read.next().await.unwrap().unwrap();
        let val: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        if val.get("id").is_some() {
            stop_resp = Some(val);
            break;
        }
    }
    assert_eq!(stop_resp.unwrap()["result"]["stopping"], true);
}
