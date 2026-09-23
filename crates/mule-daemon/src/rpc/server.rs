// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tower_http::services::{ServeDir, ServeFile};

use crate::rpc::methods::{handle_request, DAEMON_VERSION};
use crate::rpc::types::{JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let mut router = Router::new()
        .route("/health", get(health_check))
        .route("/rpc", get(ws_handler));

    // Embedded web remote controller: serve the bundled Flutter web build
    // plus a same-origin session bootstrap (endpoint + token) so the page
    // connects without any user input. `index.html` is the SPA fallback.
    if let Some(web_dir) = state.config.web_dir.clone() {
        router = router
            .route("/session-config.js", get(session_config_js))
            .fallback_service(
                ServeDir::new(&web_dir).fallback(ServeFile::new(web_dir.join("index.html"))),
            );
    }

    router.with_state(state)
}

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "daemonVersion": DAEMON_VERSION
    }))
}

/// Emits `window.MULE_SESSION = {endpoint, token}` consumed by the bundled
/// web UI to auto-connect. The token is only exposed when the socket is
/// loopback-bound — on a LAN-facing bind a remote page must never learn
/// `auth_token` (it grants full daemon control).
async fn session_config_js(State(state): State<AppState>) -> impl IntoResponse {
    if !state.config.bind_addr.ip().is_loopback() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let session = json!({
        "endpoint": format!("ws://{}/rpc", state.config.bind_addr),
        "token": state.config.auth_token,
    });
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        format!("window.MULE_SESSION={session};"),
    )
        .into_response()
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Maps a notification method name to its subscription topic, so clients
/// can opt into a subset of the broadcast stream via `events.subscribe`.
/// `None` = topic-less event that is always forwarded (e.g. snapshotRequired).
/// Delegates to `DaemonEvent::topic` — the single source of truth.
fn event_topic(method: &str) -> Option<&'static str> {
    crate::services::DaemonEvent::Custom {
        method: method.to_string(),
        payload: serde_json::Value::Null,
    }
    .topic()
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx_events = state.event_tx.subscribe();
    let is_authenticated = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        state.config.auth_token.is_empty(),
    ));

    // Per-session topic filter: `None` = forward everything (default, for
    // backward compatibility with clients that never call events.subscribe
    // with topics).
    let topics = std::sync::Arc::new(tokio::sync::RwLock::<
        Option<std::collections::HashSet<String>>,
    >::new(None));

    // Outbound message queue so spawned request handlers can send responses concurrently
    let (tx_out, mut rx_out) = tokio::sync::mpsc::channel::<Message>(128);

    loop {
        tokio::select! {
            // Outbound message from spawned tasks or broadcast
            out_msg = rx_out.recv() => {
                match out_msg {
                    Some(msg) => {
                        if sender.send(msg).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            // Incoming client message
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let parsed: Result<JsonRpcRequest, _> = serde_json::from_str(&text);
                        match parsed {
                            Ok(req) => {
                                // Session-scoped subscription handled here, not
                                // in handle_request, because it mutates this
                                // connection's own event filter. Only honored
                                // once authenticated — otherwise falls through
                                // to handle_request which answers auth_failed.
                                if req.method == "events.subscribe"
                                    && is_authenticated
                                        .load(std::sync::atomic::Ordering::SeqCst)
                                {
                                    let topics_clone = topics.clone();
                                    let tx_out_clone = tx_out.clone();
                                    tokio::spawn(async move {
                                        let set: Option<std::collections::HashSet<String>> =
                                            req.params
                                                .as_ref()
                                                .and_then(|p| p.get("topics"))
                                                .and_then(|t| t.as_array())
                                                .map(|arr| {
                                                    arr.iter()
                                                        .filter_map(|v| v.as_str().map(String::from))
                                                        .collect()
                                                });
                                        *topics_clone.write().await = set;
                                        if let Some(id) = req.id {
                                            let resp = JsonRpcResponse::success(
                                                id,
                                                serde_json::json!({ "subscribed": true }),
                                            );
                                            if let Ok(json_str) = serde_json::to_string(&resp) {
                                                let _ = tx_out_clone.send(Message::Text(json_str.into())).await;
                                            }
                                        }
                                    });
                                    continue;
                                }

                                let state_clone = state.clone();
                                let auth_clone = is_authenticated.clone();
                                let tx_out_clone = tx_out.clone();

                                tokio::spawn(async move {
                                    tracing::debug!("RPC incoming request: method={}, id={:?}", req.method, req.id);
                                    let mut auth_val = auth_clone.load(std::sync::atomic::Ordering::SeqCst);
                                    if let Some(resp) = handle_request(&state_clone, req, &mut auth_val).await {
                                        auth_clone.store(auth_val, std::sync::atomic::Ordering::SeqCst);
                                        if let Some(ref err) = resp.error {
                                            tracing::warn!("RPC response error: id={:?}, code={}, msg={}", resp.id, err.code, err.message);
                                        } else {
                                            tracing::debug!("RPC response success: id={:?}", resp.id);
                                        }
                                        if let Ok(json_str) = serde_json::to_string(&resp) {
                                            let _ = tx_out_clone.send(Message::Text(json_str.into())).await;
                                        }
                                    }
                                });
                            }
                            Err(_) => {
                                let err_resp = JsonRpcResponse::error(
                                    serde_json::Value::Null,
                                    JsonRpcError::new(-32700, "Parse error"),
                                );
                                if let Ok(json_str) = serde_json::to_string(&err_resp) {
                                    let _ = tx_out.send(Message::Text(json_str.into())).await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            // Outgoing broadcast events (forwarded only to authenticated sessions)
            event = rx_events.recv() => {
                match event {
                    Ok(event_json) => {
                        if is_authenticated.load(std::sync::atomic::Ordering::SeqCst) {
                            let forward = match &*topics.read().await {
                                None => true,
                                Some(set) => {
                                    let method = serde_json::from_str::<serde_json::Value>(&event_json)
                                        .ok()
                                        .and_then(|v| v.get("method").and_then(|m| m.as_str()).map(String::from));
                                    match method.as_deref().and_then(event_topic) {
                                        Some(topic) => set.contains(topic),
                                        None => true, // control events always pass
                                    }
                                }
                            };
                            if forward {
                                let _ = tx_out.send(Message::Text(event_json.into())).await;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        // This client fell behind: events were dropped. Ask it to
                        // resynchronize via a full snapshot instead of pretending
                        // the stream is continuous.
                        if is_authenticated.load(std::sync::atomic::Ordering::SeqCst) {
                            let notif = JsonRpcNotification::new(
                                "event.snapshotRequired",
                                serde_json::json!({ "skipped": skipped }),
                            );
                            if let Ok(json_str) = serde_json::to_string(&notif) {
                                let _ = tx_out.send(Message::Text(json_str.into())).await;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}
