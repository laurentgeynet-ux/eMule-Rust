// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde_json::{json, Value};

use crate::error::JsonRpcError;
use crate::state::AppState;

/// `friends.list` — lists configured friends (eMule `friends.met`).
pub async fn handle_friends_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let friends = state.downloads.lock().await.list_friends()?;
    let list: Vec<Value> = friends
        .iter()
        .map(|f| {
            json!({
                "userHash": f.user_hash,
                "name": f.name,
                "addedAtUnixMs": f.added_at_unix_ms,
            })
        })
        .collect();
    Ok(json!({ "friends": list, "total": list.len() }))
}

/// `friends.add` — grants the user hash a dedicated upload slot.
/// Params: `userHash` (hex, required), `name` (optional).
pub async fn handle_friends_add(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let user_hash = params
        .as_ref()
        .and_then(|p| p["userHash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'userHash'"))?
        .to_lowercase();
    let name = params
        .as_ref()
        .and_then(|p| p["name"].as_str())
        .unwrap_or("")
        .to_string();
    state.downloads.lock().await.add_friend(&user_hash, &name)?;
    state
        .inbound_listener
        .upload_manager()
        .set_friend(&format!("uh:{user_hash}"), true);
    Ok(json!({ "ok": true, "userHash": user_hash }))
}

/// `friends.remove` — revokes the friend slot.
/// Params: `userHash` (hex, required).
pub async fn handle_friends_remove(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let user_hash = params
        .as_ref()
        .and_then(|p| p["userHash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'userHash'"))?
        .to_lowercase();
    state.downloads.lock().await.remove_friend(&user_hash)?;
    state
        .inbound_listener
        .upload_manager()
        .set_friend(&format!("uh:{user_hash}"), false);
    Ok(json!({ "ok": true, "userHash": user_hash }))
}
