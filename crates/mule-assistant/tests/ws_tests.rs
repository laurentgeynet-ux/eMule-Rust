// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Test e2e du transport WS : auth par jeton daemon, chat, events.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use mule_assistant::config::AssistantConfig;
use mule_assistant::llm::mock::{text_turn, MockBackend};
use mule_assistant::session::Session;
use mule_assistant::ws::serve_ws;
use mule_test_support::spawn_test_daemon;
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct WsHarness {
    ws_url: String,
    _daemon_dir: TempDir,
    server: tokio::task::JoinHandle<()>,
}

async fn start_ws_server() -> WsHarness {
    let (addr, token, dir) = spawn_test_daemon(TOKEN).await;
    let backend = Arc::new(MockBackend::new(vec![text_turn("pong du modèle")]));
    let session = Session::connect(
        Box::new(backend),
        AssistantConfig::default(),
        Some(format!("ws://{addr}/rpc")),
        Some(token),
        None,
        None,
    )
    .await
    .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ws_addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let _ = serve_ws(listener, session, TOKEN.to_string()).await;
    });
    WsHarness {
        ws_url: format!("ws://{ws_addr}/assistant"),
        _daemon_dir: dir,
        server,
    }
}

async fn recv_json(
    ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> Value {
    loop {
        match ws.next().await.unwrap().unwrap() {
            Message::Text(t) => return serde_json::from_str(&t).unwrap(),
            _ => continue,
        }
    }
}

/// Cycle complet : hello mauvais jeton → erreur ; hello bon jeton →
/// serverInfo ; chat → events jusqu'à `assistant.turnEnd`.
#[tokio::test]
async fn test_ws_chat_flow() {
    let h = start_ws_server().await;
    let (mut ws, _) = connect_async(&h.ws_url).await.unwrap();

    // Mauvais jeton → erreur.
    ws.send(Message::Text(
        json!({"id":1,"method":"assistant.hello","params":{"token":"bad"}})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let resp = recv_json(&mut ws).await;
    assert!(resp.get("error").is_some());

    // Sans auth → refus.
    ws.send(Message::Text(
        json!({"id":2,"method":"assistant.chat","params":{"text":"salut"}})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let resp = recv_json(&mut ws).await;
    assert!(resp.get("error").is_some());

    // Bon jeton → serverInfo + nombre d'outils.
    ws.send(Message::Text(
        json!({"id":3,"method":"assistant.hello","params":{"token":TOKEN}})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let resp = recv_json(&mut ws).await;
    assert!(resp["result"]["tools"].as_u64().unwrap() > 0);

    // Chat → accepted puis events jusqu'à turnEnd avec Final "pong".
    ws.send(Message::Text(
        json!({"id":4,"method":"assistant.chat","params":{"text":"ping"}})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let resp = recv_json(&mut ws).await;
    assert_eq!(resp["result"]["accepted"], true);

    let mut saw_final = false;
    let mut saw_turn_end = false;
    for _ in 0..20 {
        let msg = recv_json(&mut ws).await;
        match msg.get("method").and_then(|m| m.as_str()) {
            Some("assistant.event") => {
                if msg["params"]["type"] == "final"
                    && msg["params"]["content"] == "pong du modèle"
                {
                    saw_final = true;
                }
            }
            Some("assistant.turnEnd") => {
                saw_turn_end = true;
                assert!(msg["params"]["error"].is_null());
                break;
            }
            _ => {}
        }
    }
    h.server.abort();
    assert!(saw_final, "event final attendu");
    assert!(saw_turn_end, "turnEnd attendu");
}

/// Bind non loopback : `serve_ws` refuse de démarrer.
#[tokio::test]
async fn test_ws_bind_rejects_non_loopback() {
    let (addr, token, _dir) = spawn_test_daemon(TOKEN).await;
    let backend = Arc::new(MockBackend::new(vec![]));
    let session = Session::connect(
        Box::new(backend),
        AssistantConfig::default(),
        Some(format!("ws://{addr}/rpc")),
        Some(token),
        None,
        None,
    )
    .await
    .unwrap();

    // Bind sur une adresse non loopback → refus de configuration.
    let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
    let res = serve_ws(listener, session, TOKEN.to_string()).await;
    assert!(res.is_err());
}
