// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use serde_json::{json, Value};

use crate::error::JsonRpcError;
use crate::state::AppState;

pub async fn handle_kad_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let status = state.kad.get_status().await;
    Ok(serde_json::to_value(status).unwrap_or_default())
}

pub async fn handle_kad_inspect_nodes_dat(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let path_str_opt = params["path"].as_str();
    let data_base64_opt = params["dataBase64"].as_str();

    let bytes = if let Some(path_str) = path_str_opt {
        std::fs::read(path_str)
            .map_err(|e| JsonRpcError::business(format!("Failed to read file {path_str}: {e}")))?
    } else if let Some(b64) = data_base64_opt {
        data_encoding::BASE64
            .decode(b64.as_bytes())
            .map_err(|e| JsonRpcError::invalid_params(&format!("Invalid base64: {e}")))?
    } else {
        return Err(JsonRpcError::invalid_params(
            "Missing 'path' or 'dataBase64' parameter",
        ));
    };

    let report = state
        .kad
        .inspect_nodes_dat(&bytes)
        .map_err(JsonRpcError::business)?;

    Ok(serde_json::to_value(report).unwrap_or_default())
}

pub async fn handle_kad_import_nodes_dat(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let path_str_opt = params["path"].as_str();
    let data_base64_opt = params["dataBase64"].as_str();

    let bytes = if let Some(path_str) = path_str_opt {
        std::fs::read(path_str)
            .map_err(|e| JsonRpcError::business(format!("Failed to read file {path_str}: {e}")))?
    } else if let Some(b64) = data_base64_opt {
        data_encoding::BASE64
            .decode(b64.as_bytes())
            .map_err(|e| JsonRpcError::invalid_params(&format!("Invalid base64: {e}")))?
    } else {
        return Err(JsonRpcError::invalid_params(
            "Missing 'path' or 'dataBase64' parameter",
        ));
    };

    let imported_count = state
        .kad
        .import_nodes_dat(&bytes)
        .await
        .map_err(JsonRpcError::business)?;

    let status = state.kad.get_status().await;
    emit_kad_changed(state);
    Ok(json!({
        "importedCount": imported_count,
        "status": status,
    }))
}

pub async fn handle_kad_list_contacts(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let limit = params
        .as_ref()
        .and_then(|p| p["limit"].as_u64())
        .unwrap_or(100) as usize;
    let limit = limit.clamp(1, 1000);

    let contacts = state.kad.list_contacts(limit).await;
    Ok(json!({
        "contacts": contacts,
        "total": contacts.len(),
    }))
}

fn emit_kad_changed(state: &AppState) {
    state.emit_event(|sequence| crate::services::DaemonEvent::KadStatusChanged { sequence });
}

pub async fn handle_kad_clear_contacts(state: &AppState) -> Result<Value, JsonRpcError> {
    let cleared = state
        .kad
        .clear_contacts()
        .await
        .map_err(JsonRpcError::business)?;

    emit_kad_changed(state);
    Ok(json!({
        "cleared": cleared,
    }))
}

pub async fn handle_kad_start_udp(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let default_bind = if state.network_policy.profile().is_loopback_only() {
        "127.0.0.1:39737"
    } else {
        "0.0.0.0:39737"
    };
    let bind_addr_str = params
        .as_ref()
        .and_then(|p| p["bindAddress"].as_str())
        .unwrap_or(default_bind);

    let addr: std::net::SocketAddr = bind_addr_str
        .parse()
        .map_err(|e| JsonRpcError::invalid_params(&format!("Invalid bind address: {e}")))?;

    let local_addr = state
        .kad
        .udp()
        .start(addr)
        .await
        .map_err(JsonRpcError::business)?;

    let status = state.kad.get_status().await;
    emit_kad_changed(state);
    Ok(json!({
        "boundAddress": local_addr.to_string(),
        "status": status,
    }))
}

pub async fn handle_kad_stop_udp(state: &AppState) -> Result<Value, JsonRpcError> {
    state
        .kad
        .udp()
        .stop()
        .await
        .map_err(JsonRpcError::business)?;

    let status = state.kad.get_status().await;

    Ok(json!({
        "stopped": true,
        "status": status,
    }))
}

pub async fn handle_kad_ping(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let ip = params["ip"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'ip' parameter"))?;
    let port = params["port"]
        .as_u64()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'port' parameter"))?
        as u16;
    let target_kad_id = params["kadId"].as_str();

    let latency_ms = state
        .kad
        .ping_contact(ip, port, target_kad_id)
        .await
        .map_err(JsonRpcError::business)?;

    let status = state.kad.get_status().await;

    Ok(json!({
        "success": true,
        "latencyMs": latency_ms,
        "status": status,
    }))
}

pub async fn handle_kad_bootstrap(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let ip = params["ip"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'ip' parameter"))?;
    let port = params["port"]
        .as_u64()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'port' parameter"))?
        as u16;

    let received_count = state
        .kad
        .bootstrap_from_node(ip, port)
        .await
        .map_err(JsonRpcError::business)?;

    let status = state.kad.get_status().await;
    emit_kad_changed(state);
    Ok(json!({
        "success": true,
        "receivedContacts": received_count,
        "status": status,
    }))
}

pub async fn handle_kad_find_node(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let target_id_hex = params["targetId"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'targetId' parameter"))?;

    let target_kad_id = mule_core::kad::KadId::from_hex(target_id_hex)
        .ok_or_else(|| JsonRpcError::invalid_params("Invalid 128-bit hex targetId"))?;

    let contacts = state
        .kad
        .find_node(&target_kad_id)
        .await
        .map_err(JsonRpcError::business)?;

    let status = state.kad.get_status().await;

    Ok(json!({
        "contacts": contacts,
        "count": contacts.len(),
        "status": status,
    }))
}

pub async fn handle_kad_search_sources(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let file_hash = params["fileHash"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;
    let file_size = params["fileSize"].as_u64().unwrap_or(0);

    let sources = state
        .kad
        .search_sources(file_hash, file_size)
        .await
        .map_err(JsonRpcError::business)?;

    Ok(json!({
        "fileHash": file_hash,
        "sources": sources,
        "count": sources.len(),
    }))
}

pub async fn handle_kad_publish_source(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let file_hash = params["fileHash"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;
    let file_size = params["fileSize"]
        .as_u64()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileSize' parameter"))?;
    let tcp_port = params["tcpPort"].as_u64().unwrap_or(39735) as u16;

    let published_count = state
        .kad
        .publish_source(file_hash, file_size, tcp_port)
        .await
        .map_err(JsonRpcError::business)?;

    Ok(json!({
        "fileHash": file_hash,
        "publishedCount": published_count,
    }))
}

pub async fn handle_kad_search_keywords(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let term = params["term"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'term' parameter"))?;
    let min_size = params["minSize"].as_u64();
    let max_size = params["maxSize"].as_u64();

    let results = state
        .kad
        .search_keywords(term, min_size, max_size)
        .await
        .map_err(JsonRpcError::business)?;

    Ok(json!({
        "term": term,
        "results": results,
        "count": results.len(),
    }))
}

pub async fn handle_kad_publish_keywords(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let file_hash = params["fileHash"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;
    let file_name = params["fileName"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileName' parameter"))?;
    let file_size = params["fileSize"]
        .as_u64()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileSize' parameter"))?;

    let published_count = state
        .kad
        .publish_keyword(file_hash, file_name, file_size)
        .await
        .map_err(JsonRpcError::business)?;

    Ok(json!({
        "fileHash": file_hash,
        "fileName": file_name,
        "publishedCount": published_count,
    }))
}

pub async fn handle_kad_search_notes(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let file_hash = params["fileHash"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;
    let file_size = params["fileSize"]
        .as_u64()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileSize' parameter"))?;

    let notes = state
        .kad
        .search_notes(file_hash, file_size)
        .await
        .map_err(JsonRpcError::business)?;

    Ok(json!({
        "fileHash": file_hash,
        "notes": notes,
        "count": notes.len(),
    }))
}

pub async fn handle_kad_publish_note(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let file_hash = params["fileHash"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;
    let file_name = params["fileName"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileName' parameter"))?;
    let file_size = params["fileSize"]
        .as_u64()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileSize' parameter"))?;
    let comment = params["comment"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'comment' parameter"))?;
    let rating = params["rating"].as_u64().map(|r| r.min(5) as u8);

    let published_count = state
        .kad
        .publish_note(file_hash, file_name, file_size, comment, rating)
        .await
        .map_err(JsonRpcError::business)?;

    Ok(json!({
        "fileHash": file_hash,
        "publishedCount": published_count,
    }))
}

pub async fn handle_kad_bootstrap_from_url(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let url = params["url"]
        .as_str()
        .unwrap_or("http://upd.emule-security.org/nodes.dat");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| JsonRpcError::business(format!("HTTP client build error: {e}")))?;

    let response = client.get(url).send().await.map_err(|e| {
        JsonRpcError::business(format!("Failed to fetch nodes.dat from {url}: {e}"))
    })?;

    if !response.status().is_success() {
        return Err(JsonRpcError::business(format!(
            "HTTP request to {url} returned status {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| JsonRpcError::business(format!("Failed to read response body: {e}")))?;

    let imported_count = state
        .kad
        .import_nodes_dat(&bytes)
        .await
        .map_err(JsonRpcError::business)?;

    // Spawn background bootstrap across seed contacts to discover active live neighbors
    let kad_clone = state.kad.clone();
    tokio::spawn(async move {
        kad_clone.bootstrap_active_contacts(15).await;
    });

    let status = state.kad.get_status().await;
    emit_kad_changed(state);
    Ok(json!({
        "url": url,
        "importedCount": imported_count,
        "status": status,
    }))
}
