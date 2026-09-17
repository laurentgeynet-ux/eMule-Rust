use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;

use crate::rpc::methods::{handle_request, DAEMON_VERSION};
use crate::rpc::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/rpc", get(ws_handler))
        .with_state(state)
}

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "daemonVersion": DAEMON_VERSION
    }))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx_events = state.event_tx.subscribe();
    let is_authenticated = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        state.config.auth_token.is_empty(),
    ));

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
                                let state_clone = state.clone();
                                let auth_clone = is_authenticated.clone();
                                let tx_out_clone = tx_out.clone();

                                tokio::spawn(async move {
                                    tracing::info!("RPC incoming request: method={}, id={:?}", req.method, req.id);
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
                if let Ok(event_json) = event {
                    if is_authenticated.load(std::sync::atomic::Ordering::SeqCst) {
                        let _ = tx_out.send(Message::Text(event_json.into())).await;
                    }
                }
            }
        }
    }
}
