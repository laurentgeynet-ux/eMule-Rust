// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde_json::{json, Value};

use crate::error::JsonRpcError;
use crate::state::AppState;

pub async fn handle_categories_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let list = state.downloads.lock().await.list_categories()?;
    serde_json::to_value(list)
        .map_err(|e| JsonRpcError::business(format!("Serialization error: {e}")))
}

pub async fn handle_categories_create(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let name = params["name"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'name' parameter"))?;
    let color = params["color"].as_str().unwrap_or("#2196F3");
    let default_dest = params["defaultDestination"].as_str().map(String::from);
    let default_prio = params["defaultPriority"].as_str().map(String::from);

    let created =
        state
            .downloads
            .lock()
            .await
            .create_category(name, color, default_dest, default_prio)?;

    serde_json::to_value(created)
        .map_err(|e| JsonRpcError::business(format!("Serialization error: {e}")))
}

pub async fn handle_categories_update(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;

    let name = params["name"].as_str().map(String::from);
    let color = params["color"].as_str().map(String::from);
    let default_dest = if params.get("defaultDestination").is_some() {
        Some(params["defaultDestination"].as_str().map(String::from))
    } else {
        None
    };
    let default_prio = if params.get("defaultPriority").is_some() {
        Some(params["defaultPriority"].as_str().map(String::from))
    } else {
        None
    };
    let sort_order = params["sortOrder"].as_i64().map(|n| n as i32);

    let updated = state.downloads.lock().await.update_category(
        id,
        name,
        color,
        default_dest,
        default_prio,
        sort_order,
    )?;

    serde_json::to_value(updated)
        .map_err(|e| JsonRpcError::business(format!("Serialization error: {e}")))
}

pub async fn handle_categories_delete(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;

    let reassigned = state.downloads.lock().await.delete_category(id)?;

    // Broadcast updates for all active downloads
    let snapshots = state.downloads.lock().await.list_downloads();
    for snap in snapshots {
        state.broadcast_download_update(snap);
    }

    Ok(json!({
        "deleted": true,
        "reassignedDownloads": reassigned,
    }))
}

pub async fn handle_categories_reorder(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let ids_val = params["categoryIds"]
        .as_array()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'categoryIds' array parameter"))?;

    let category_ids: Vec<String> = ids_val
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    state
        .downloads
        .lock()
        .await
        .reorder_categories(&category_ids)?;

    Ok(json!({ "success": true }))
}
