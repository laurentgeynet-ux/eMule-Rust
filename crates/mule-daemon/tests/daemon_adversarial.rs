use std::fs;
use std::io::Cursor;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use mule_daemon::lab::LabSourceService;
use mule_daemon::{create_router, AppState, DaemonConfig, DirectoryLock, DownloadService};
use mule_format::ed2k;
use mule_protocol::lab::LabSourceEngine;
use serde_json::json;
use tempfile::tempdir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const AUTH_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Lit la prochaine réponse JSON-RPC portant `id` sur le flux WS, en ignorant
/// les notifications d'événements broadcast intercalées (le serveur multiplexe
/// réponses et events via `select!` — l'ordre n'est pas garanti).
async fn read_response<S>(read: &mut S, id: u64) -> serde_json::Value
where
    S: futures_util::Stream<
            Item = Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>,
        > + Unpin,
{
    for _ in 0..64 {
        let msg = tokio::time::timeout(Duration::from_secs(10), read.next())
            .await
            .expect("timeout waiting for RPC response")
            .expect("stream closed")
            .expect("ws error");
        let v: serde_json::Value = serde_json::from_str(&msg.to_string()).unwrap();
        if v.get("id").and_then(|i| i.as_u64()) == Some(id) {
            return v;
        }
    }
    panic!("no JSON-RPC response with id={id} after 64 messages");
}

struct RunningDaemon {
    addr: SocketAddr,
    state: AppState,
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

        let config = DaemonConfig::new(workdir, AUTH_TOKEN).with_bind_addr(addr);
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
async fn test_corrupted_database_fails_explicitly_in_daemon() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("mule.db");
    let temp_parts_dir = dir.path().join("part");

    let garbage = b"Invalid header bytes for SQLite database \x00\xFF\xAA\x55";
    fs::write(&db_path, garbage).expect("write garbage");

    let res = DownloadService::new(&db_path, temp_parts_dir);
    assert!(res.is_err(), "DownloadService::new must fail on corrupt DB");

    // Must not delete or overwrite the corrupt DB
    assert!(db_path.exists(), "Corrupt DB must still exist");
    assert_eq!(fs::read(&db_path).unwrap(), garbage);
}

#[tokio::test]
async fn test_concurrent_downloads_no_crosstalk() {
    // 1. Generate two synthetic files with different sizes and data
    // File 1: 2 blocks (368_640 bytes) filled with 0xAA
    let data1 = vec![0xAAu8; 368_640];
    let hash1 = ed2k::hash_reader(&mut Cursor::new(&data1))
        .unwrap()
        .root_hash;
    let hash1_hex = hex::encode(hash1.0);

    // File 2: 3 blocks (552_960 bytes) filled with 0xBB
    let data2 = vec![0xBBu8; 552_960];
    let hash2 = ed2k::hash_reader(&mut Cursor::new(&data2))
        .unwrap()
        .root_hash;
    let hash2_hex = hex::encode(hash2.0);

    // 2. Setup Source Daemon A hosting both files
    let mut engine = LabSourceEngine::new();
    engine.host_file(hash1.0, "concurrent_1.txt", Bytes::from(data1.clone()));
    engine.host_file(hash2.0, "concurrent_2.txt", Bytes::from(data2.clone()));

    let source_service = LabSourceService::start(engine).await.unwrap();
    let source_addr = source_service.local_addr();

    // 3. Setup Client Daemon B
    let client_dir = tempdir().unwrap();
    let client_target_dir = tempdir().unwrap();

    let client_daemon = RunningDaemon::start(client_dir.path().to_path_buf()).await;
    let client_addr = client_daemon.addr;

    // Connect WebSocket client to Daemon B
    let ws_url = format!("ws://{client_addr}/rpc");
    let (ws_stream, _) = connect_async(&ws_url).await.unwrap();
    let (mut write, mut read) = ws_stream.split();

    // Handshake
    let handshake_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "system.hello",
        "params": {
            "protocolVersion": 1,
            "token": AUTH_TOKEN
        }
    });
    write
        .send(Message::Text(handshake_req.to_string().into()))
        .await
        .unwrap();
    let _ = read.next().await.unwrap().unwrap();

    // Add File 1
    let link1 = format!("ed2k://|file|concurrent_1.txt|368640|{}|/", hash1_hex);
    let add1_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "downloads.addEd2kLink",
        "params": {
            "link": link1,
            "targetDirectory": client_target_dir.path().to_str().unwrap()
        }
    });
    write
        .send(Message::Text(add1_req.to_string().into()))
        .await
        .unwrap();
    let add1_resp: serde_json::Value = read_response(&mut read, 2).await;
    let id1_str = add1_resp["result"]["downloadId"]
        .as_str()
        .unwrap()
        .to_string();

    // Add File 2
    let link2 = format!("ed2k://|file|concurrent_2.txt|552960|{}|/", hash2_hex);
    let add2_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "downloads.addEd2kLink",
        "params": {
            "link": link2,
            "targetDirectory": client_target_dir.path().to_str().unwrap()
        }
    });
    write
        .send(Message::Text(add2_req.to_string().into()))
        .await
        .unwrap();
    let add2_resp: serde_json::Value = read_response(&mut read, 3).await;
    let id2_str = add2_resp["result"]["downloadId"]
        .as_str()
        .unwrap()
        .to_string();

    // Start both transfers simultaneously
    let src_add1_req = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "lab.addPeerSource",
        "params": {
            "downloadId": id1_str,
            "peerAddr": source_addr.to_string()
        }
    });
    write
        .send(Message::Text(src_add1_req.to_string().into()))
        .await
        .unwrap();
    let _ = read.next().await.unwrap().unwrap(); // response for lab.addPeerSource 1

    let src_add2_req = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "lab.addPeerSource",
        "params": {
            "downloadId": id2_str,
            "peerAddr": source_addr.to_string()
        }
    });
    write
        .send(Message::Text(src_add2_req.to_string().into()))
        .await
        .unwrap();
    let _ = read.next().await.unwrap().unwrap(); // response for lab.addPeerSource 2

    // Monitor events until BOTH downloads reach "Completed"
    let mut id1_completed = false;
    let mut id2_completed = false;

    let timeout = Duration::from_secs(10);
    let start = std::time::Instant::now();

    while (!id1_completed || !id2_completed) && start.elapsed() < timeout {
        if let Ok(Some(Ok(msg))) =
            tokio::time::timeout(Duration::from_millis(500), read.next()).await
        {
            let val: serde_json::Value = serde_json::from_str(&msg.to_string()).unwrap_or_default();
            if val["method"] == "event.downloadUpdated" {
                let dl = &val["params"]["download"];
                let dl_id = dl["id"].as_str().unwrap_or_default();
                let status = dl["status"].as_str().unwrap_or_default();
                if dl_id == id1_str && status == "completed" {
                    id1_completed = true;
                }
                if dl_id == id2_str && status == "completed" {
                    id2_completed = true;
                }
            }
        }
    }

    assert!(id1_completed, "Download 1 must reach completed");
    assert!(id2_completed, "Download 2 must reach completed");

    // Verify isolation and accuracy of final files on physical disk
    let file1_final = client_target_dir.path().join("concurrent_1.txt");
    let file2_final = client_target_dir.path().join("concurrent_2.txt");

    assert!(file1_final.exists(), "File 1 must exist at destination");
    assert!(file2_final.exists(), "File 2 must exist at destination");

    let read_data1 = fs::read(&file1_final).unwrap();
    assert_eq!(read_data1.len(), 368_640);
    assert_eq!(read_data1, data1, "File 1 content must match data1 exactly");

    let read_data2 = fs::read(&file2_final).unwrap();
    assert_eq!(read_data2.len(), 552_960);
    assert_eq!(read_data2, data2, "File 2 content must match data2 exactly");

    // Clean up
    client_daemon.stop().await;
    source_service.stop();
}

#[tokio::test]
async fn test_slow_source_delayed_delivery_recovers_cleanly() {
    let data = vec![0x33u8; 10_000];
    let hash = ed2k::hash_reader(&mut Cursor::new(&data))
        .unwrap()
        .root_hash;
    let hash_hex = hex::encode(hash.0);

    // Mock slow server listening on 127.0.0.1:0
    let slow_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let slow_addr = slow_listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = slow_listener.accept().await.unwrap();
        let codec = mule_protocol::PacketCodec::new();
        let mut buf = bytes::BytesMut::new();

        // 1. Read Hello
        let _ = read_lab_msg(&mut stream, &codec, &mut buf).await.unwrap();

        // 2. Send HelloAck
        let ack = mule_protocol::lab::LabMessage::HelloAck {
            server_version: 1,
            nonce_ack: [0x42; 16],
            peer_id: mule_protocol::types::PeerId::new([0x02; 16]),
        };
        send_lab_msg(&mut stream, &codec, &ack).await.unwrap();

        // 3. Send FileOffer
        let offer = mule_protocol::lab::LabMessage::FileOffer {
            file_hash: hash.0,
            file_size: 10_000,
            name: "slow_file.txt".into(),
        };
        send_lab_msg(&mut stream, &codec, &offer).await.unwrap();

        // 4. Read RequestBlock
        let req = read_lab_msg(&mut stream, &codec, &mut buf).await.unwrap();
        if let mule_protocol::lab::LabMessage::RequestBlock {
            start_offset,
            length,
            ..
        } = req
        {
            // Intentionally delay 1 second to simulate slow source / high latency
            tokio::time::sleep(Duration::from_millis(1000)).await;

            let block = mule_protocol::lab::LabMessage::BlockData {
                file_hash: hash.0,
                start_offset,
                data: Bytes::from(
                    data[start_offset as usize..(start_offset as usize + length as usize)].to_vec(),
                ),
            };
            send_lab_msg(&mut stream, &codec, &block).await.unwrap();
        }

        // 5. Read Goodbye
        let _ = read_lab_msg(&mut stream, &codec, &mut buf).await;
    });

    // Start Client Daemon
    let client_dir = tempdir().unwrap();
    let client_target_dir = tempdir().unwrap();

    let client_daemon = RunningDaemon::start(client_dir.path().to_path_buf()).await;
    let client_addr = client_daemon.addr;

    let ws_url = format!("ws://{client_addr}/rpc");
    let (ws_stream, _) = connect_async(&ws_url).await.unwrap();
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
    let _ = read.next().await.unwrap().unwrap();

    // Subscribe
    let req_sub = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "daemon.subscribe",
        "params": {}
    });
    write
        .send(Message::Text(req_sub.to_string().into()))
        .await
        .unwrap();
    let _ = read.next().await.unwrap().unwrap();

    // Add download
    let link = format!("ed2k://|file|slow_file.txt|10000|{hash_hex}|/");
    let req_add = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "downloads.addEd2kLink",
        "params": {
            "link": link,
            "targetDirectory": client_target_dir.path().to_str().unwrap()
        }
    });
    write
        .send(Message::Text(req_add.to_string().into()))
        .await
        .unwrap();
    let add_resp: serde_json::Value =
        serde_json::from_str(&read.next().await.unwrap().unwrap().to_string()).unwrap();
    let dl_id = add_resp["result"]["downloadId"]
        .as_str()
        .unwrap()
        .to_string();
    let _ = read.next().await.unwrap().unwrap(); // Queued event

    // Add slow peer source
    let req_src = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "lab.addPeerSource",
        "params": {
            "downloadId": dl_id,
            "peerAddr": slow_addr.to_string()
        }
    });
    write
        .send(Message::Text(req_src.to_string().into()))
        .await
        .unwrap();
    let _ = read.next().await.unwrap().unwrap();

    // Wait for completed with timeout
    let mut completed = false;
    let timeout = Duration::from_secs(5);
    let start = std::time::Instant::now();

    while !completed && start.elapsed() < timeout {
        if let Ok(Some(Ok(msg))) =
            tokio::time::timeout(Duration::from_millis(500), read.next()).await
        {
            let val: serde_json::Value = serde_json::from_str(&msg.to_string()).unwrap_or_default();
            if val["method"] == "event.downloadUpdated" {
                let status = val["params"]["download"]["status"]
                    .as_str()
                    .unwrap_or_default();
                if status == "completed" {
                    completed = true;
                }
            }
        }
    }

    assert!(
        completed,
        "Slow download must successfully complete despite source delay"
    );
    let final_file = client_target_dir.path().join("slow_file.txt");
    assert!(final_file.exists());
    assert_eq!(fs::read(&final_file).unwrap().len(), 10_000);

    client_daemon.stop().await;
    let _ = server_task.await;
}

async fn send_lab_msg(
    stream: &mut tokio::net::TcpStream,
    codec: &mule_protocol::PacketCodec,
    msg: &mule_protocol::lab::LabMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let packet = msg.to_raw_packet()?;
    let encoded = codec.encode_to_bytes(&packet)?;
    use tokio::io::AsyncWriteExt;
    stream.write_all(&encoded).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_lab_msg(
    stream: &mut tokio::net::TcpStream,
    codec: &mule_protocol::PacketCodec,
    read_buf: &mut bytes::BytesMut,
) -> Result<mule_protocol::lab::LabMessage, Box<dyn std::error::Error + Send + Sync>> {
    use tokio::io::AsyncReadExt;
    loop {
        if let Some(packet) = codec.decode(read_buf)? {
            return Ok(mule_protocol::lab::LabMessage::from_raw_packet(&packet)?);
        }
        let mut temp = [0u8; 4096];
        let n = stream.read(&mut temp).await?;
        if n == 0 {
            return Err("Unexpected EOF".into());
        }
        read_buf.extend_from_slice(&temp[..n]);
    }
}
