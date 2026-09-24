// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Helpers WebSocket : envoi JSON et attente de la réponse portant l'id
//! attendu. Les notifications d'événements broadcast sont multiplexées sur le
//! socket et ignorées jusqu'à la réponse.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

/// Nombre maximal de trames lues en quête d'une réponse (les events broadcast
/// peuvent précéder la réponse RPC sur le même socket).
const MAX_RESPONSE_FRAMES: usize = 64;

/// Timeout par trame lue — borne la patience totale sans bloquer
/// indéfiniment un test sur un socket silencieux.
const FRAME_TIMEOUT: Duration = Duration::from_secs(10);

/// Envoie `val` sérialisé en JSON sur le flux d'écriture WS.
pub async fn send_json(
    write: &mut (impl SinkExt<Message, Error = WsError> + Unpin),
    val: Value,
) {
    write
        .send(Message::Text(val.to_string().into()))
        .await
        .unwrap();
}

/// Lit le flux WS jusqu'à la réponse JSON-RPC portant `id == expected_id`.
/// Panique si aucune réponse ne survient dans [`MAX_RESPONSE_FRAMES`] trames
/// ou si une trame dépasse [`FRAME_TIMEOUT`].
pub async fn recv_response_by_id(
    read: &mut (impl StreamExt<Item = Result<Message, WsError>> + Unpin),
    expected_id: u64,
) -> Value {
    for _ in 0..MAX_RESPONSE_FRAMES {
        let msg = tokio::time::timeout(FRAME_TIMEOUT, read.next())
            .await
            .expect("timeout waiting for RPC response")
            .expect("ws stream ended")
            .expect("ws frame error");
        if let Ok(text) = msg.to_text() {
            if let Ok(val) = serde_json::from_str::<Value>(text) {
                if val.get("id").and_then(|v| v.as_u64()) == Some(expected_id) {
                    return val;
                }
            }
        }
    }
    panic!("Did not receive response for request id {expected_id}");
}
