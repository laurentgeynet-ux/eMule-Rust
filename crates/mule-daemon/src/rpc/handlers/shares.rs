use serde_json::{json, Value};

use crate::error::JsonRpcError;
use crate::state::AppState;

pub async fn handle_shares_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let dl_svc = state.downloads.lock().await;
    let list = dl_svc.list_shared_files()?;
    Ok(json!({ "sharedFiles": list }))
}

pub async fn handle_shares_add(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let file_hash = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash'"))?;
    let file_name = params["fileName"]
        .as_str()
        .or_else(|| params["name"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileName'"))?;
    let file_size = params["fileSize"]
        .as_u64()
        .or_else(|| params["size"].as_u64())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileSize'"))?;
    let file_path = params["filePath"]
        .as_str()
        .or_else(|| params["path"].as_str())
        .unwrap_or("");

    let shared = mule_core::model::SharedFile::new(file_hash, file_name, file_size, file_path);
    state.downloads.lock().await.add_shared_file(&shared)?;
    Ok(serde_json::to_value(shared).unwrap_or_default())
}

pub async fn handle_shares_remove(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let file_hash = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash'"))?;

    let removed = state.downloads.lock().await.remove_shared_file(file_hash)?;
    Ok(json!({ "removed": removed }))
}

pub async fn handle_shares_set_enabled(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let file_hash = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash'"))?;
    let enabled = params["enabled"].as_bool().unwrap_or(true);

    let mut dl_svc = state.downloads.lock().await;
    let mut file_opt = dl_svc.get_shared_file(file_hash)?;
    if let Some(ref mut sf) = file_opt {
        sf.enabled = enabled;
        dl_svc.add_shared_file(sf)?;
        Ok(serde_json::to_value(sf).unwrap_or_default())
    } else {
        Err(JsonRpcError::business("Shared file not found"))
    }
}

/// `shares.setUploadPriority` — eMule upload priority (powershare) for a
/// shared file: "verylow" | "low" | "normal" | "high" | "veryhigh".
/// Weighted into the upload queue score (`GetFilePrioAsNumber` semantics).
pub async fn handle_shares_set_upload_priority(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let file_hash = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash'"))?;
    let priority = params["priority"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'priority'"))?
        .to_lowercase();
    match priority.as_str() {
        "verylow" | "low" | "normal" | "high" | "veryhigh" | "powershare" => {}
        _ => return Err(JsonRpcError::invalid_params("Invalid priority")),
    }
    let updated = state
        .downloads
        .lock()
        .await
        .set_shared_up_priority(file_hash, &priority)?;
    if updated {
        Ok(json!({ "ok": true, "fileHash": file_hash, "upPriority": priority }))
    } else {
        Err(JsonRpcError::business("Shared file not found"))
    }
}

pub async fn handle_listener_start(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let port = params
        .as_ref()
        .and_then(|p| p["port"].as_u64())
        .unwrap_or(39735) as u16;

    let bind_ip = params
        .as_ref()
        .and_then(|p| p["bindIp"].as_str())
        .unwrap_or("127.0.0.1");

    let bind_addr: std::net::SocketAddr = format!("{bind_ip}:{port}")
        .parse()
        .map_err(|e| JsonRpcError::invalid_params(&format!("Invalid bind address: {e}")))?;

    let actual_addr = state
        .inbound_listener
        .start(bind_addr)
        .await
        .map_err(JsonRpcError::business)?;

    Ok(json!({
        "started": true,
        "listenAddress": actual_addr.to_string(),
    }))
}

pub async fn handle_listener_stop(state: &AppState) -> Result<Value, JsonRpcError> {
    state.inbound_listener.stop().await;
    Ok(json!({ "stopped": true }))
}

pub async fn handle_listener_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let snap = state.inbound_listener.snapshot().await;
    Ok(serde_json::to_value(snap).unwrap_or_default())
}

pub async fn handle_uploads_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let active_list = state
        .inbound_listener
        .upload_manager()
        .active_snapshots()
        .await;
    let queue = state.inbound_listener.upload_manager().list_queue().await;
    Ok(json!({
        "active": active_list,
        "queued": queue,
    }))
}

pub async fn handle_uploads_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let active = state
        .inbound_listener
        .upload_manager()
        .active_snapshot()
        .await;
    Ok(serde_json::to_value(active).unwrap_or(Value::Null))
}

pub async fn handle_uploads_cancel(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let client_address = params["clientAddress"]
        .as_str()
        .or_else(|| params["address"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'clientAddress'"))?;

    state
        .inbound_listener
        .upload_manager()
        .release_slot(client_address)
        .await;

    Ok(json!({ "cancelled": true }))
}

pub async fn handle_identity_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let id_guard = state.identity.lock().await;
    let status = serde_json::to_value(id_guard.status()).unwrap_or_default();
    let sui = serde_json::to_value(state.sui_stats.snapshot()).unwrap_or_default();
    match status {
        Value::Object(mut m) => {
            m.insert("sui".to_string(), sui);
            Ok(Value::Object(m))
        }
        other => Ok(json!({ "identity": other, "sui": sui })),
    }
}

pub async fn handle_identity_rotate(state: &AppState) -> Result<Value, JsonRpcError> {
    let mut id_guard = state.identity.lock().await;
    let new_id = crate::services::IdentityService::rotate_key(&state.config.data_dir)
        .map_err(|e| JsonRpcError::business(format!("Failed to rotate identity key: {e}")))?;
    *id_guard = new_id;
    let status = id_guard.status();
    Ok(serde_json::to_value(status).unwrap_or_default())
}

pub async fn handle_credits_list(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let limit = params
        .as_ref()
        .and_then(|p| p["limit"].as_u64())
        .unwrap_or(50) as usize;
    let limit = limit.clamp(1, 500);

    let credits = state.downloads.lock().await.list_peer_credits(limit)?;
    let credit_list: Vec<Value> = credits
        .into_iter()
        .map(|c| {
            json!({
                "pubkeyHash": c.pubkey_hash,
                "pubkeyHex": c.pubkey_hex,
                "uploadedBytes": c.uploaded_bytes,
                "downloadedBytes": c.downloaded_bytes,
                "creditMultiplier": c.credit_multiplier(),
                "firstSeenUnixMs": c.first_seen_unix_ms,
                "lastSeenUnixMs": c.last_seen_unix_ms,
            })
        })
        .collect();

    Ok(json!({
        "credits": credit_list,
        "total": credit_list.len(),
    }))
}

pub async fn handle_credits_stats(state: &AppState) -> Result<Value, JsonRpcError> {
    let credits = state.downloads.lock().await.list_peer_credits(1000)?;
    let total_peers = credits.len();
    let mut total_uploaded_bytes = 0u64;
    let mut total_downloaded_bytes = 0u64;
    let mut active_credited_peers = 0;

    for c in &credits {
        total_uploaded_bytes = total_uploaded_bytes.saturating_add(c.uploaded_bytes);
        total_downloaded_bytes = total_downloaded_bytes.saturating_add(c.downloaded_bytes);
        if c.uploaded_bytes > 0 || c.downloaded_bytes > 0 {
            active_credited_peers += 1;
        }
    }

    Ok(json!({
        "totalPeers": total_peers,
        "totalUploadedBytes": total_uploaded_bytes,
        "totalDownloadedBytes": total_downloaded_bytes,
        "activeCreditedPeers": active_credited_peers,
    }))
}
