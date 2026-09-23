// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use serde_json::{json, Value};

use crate::error::JsonRpcError;
use crate::state::AppState;

pub async fn handle_history_list(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let filter = if let Some(p) = params {
        serde_json::from_value::<mule_core::HistoryFilter>(p).unwrap_or_default()
    } else {
        mule_core::HistoryFilter::default()
    };

    let entries = state.downloads.lock().await.query_history(&filter)?;
    Ok(serde_json::to_value(entries).unwrap_or_default())
}

pub async fn handle_history_stats(state: &AppState) -> Result<Value, JsonRpcError> {
    let stats = state.downloads.lock().await.get_history_stats()?;
    Ok(serde_json::to_value(stats).unwrap_or_default())
}

pub async fn handle_history_clear(state: &AppState) -> Result<Value, JsonRpcError> {
    let cleared_count = state.downloads.lock().await.clear_history()?;
    Ok(json!({
        "success": true,
        "clearedCount": cleared_count
    }))
}

pub async fn handle_history_export(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let format = params["format"].as_str().unwrap_or("json");

    let filter =
        serde_json::from_value::<mule_core::HistoryFilter>(params.clone()).unwrap_or_default();

    let dls = state.downloads.lock().await;
    let content = dls.export_history(format, &filter)?;
    let entries = dls.query_history(&filter)?;

    Ok(json!({
        "format": format.to_ascii_lowercase(),
        "content": content,
        "count": entries.len()
    }))
}
