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
    let mut is_authenticated = state.config.auth_token.is_empty(); // True if no token required

    loop {
        tokio::select! {
            // Incoming client message
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let parsed: Result<JsonRpcRequest, _> = serde_json::from_str(&text);
                        match parsed {
                            Ok(req) => {
                                if let Some(resp) = handle_request(&state, req, &mut is_authenticated).await {
                                    if let Ok(json_str) = serde_json::to_string(&resp) {
                                        if sender.send(Message::Text(json_str.into())).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                let err_resp = JsonRpcResponse::error(
                                    serde_json::Value::Null,
                                    JsonRpcError::new(-32700, "Parse error"),
                                );
                                if let Ok(json_str) = serde_json::to_string(&err_resp) {
                                    let _ = sender.send(Message::Text(json_str.into())).await;
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
                    if is_authenticated
                        && sender.send(Message::Text(event_json.into())).await.is_err()
                    {
                        break;
                    }
                }
            }
        }
    }
}
