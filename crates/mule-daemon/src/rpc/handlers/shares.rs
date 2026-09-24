// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde_json::{json, Value};

use crate::error::JsonRpcError;
use crate::state::AppState;

/// Nudge `shares.changed` — les clients refetchent `shares.list` (le badge
/// sidebar inclus) après chaque mutation, y compris celles initiées hors
/// UI (MCP, CLI).
fn emit_shares_changed(state: &AppState) {
    state.emit_event(|sequence| crate::services::DaemonEvent::Custom {
        method: "shares.changed".to_string(),
        payload: json!({ "sequence": sequence }),
    });
}

pub async fn handle_shares_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let mut list = {
        let dl_svc = state.downloads.lock().await;
        dl_svc.list_all_shared_files()?
    };
    // Statistiques d'upload par fichier (jointure sur le hash en
    // minuscules) : débit mesuré instantané agrégé sur les slots actifs +
    // total cumulé persisté (eMule colonnes « Upload speed » /
    // « Transferred » de la liste des fichiers partagés).
    let um = state.inbound_listener.upload_manager();
    let rates = um.per_file_rates_bps().await;
    let totals = um.per_file_uploaded_bytes();
    for f in &mut list {
        let key = f.file_hash.to_lowercase();
        f.upload_rate_bps = rates.get(&key).copied().unwrap_or(0);
        f.uploaded_bytes = totals.get(&key).copied().unwrap_or(0);
    }
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

    let mut shared = mule_core::model::SharedFile::new(file_hash, file_name, file_size, file_path);
    {
        let mut dl_svc = state.downloads.lock().await;
        // eMule `UAPPref` : un nouveau partage démarre en priorité « auto ».
        if dl_svc.auto_up_priority_enabled() {
            shared.up_priority = "auto".to_string();
        }
        dl_svc.add_shared_file(&shared)?;
    }
    emit_shares_changed(state);
    Ok(crate::rpc::handlers::json_value(&shared))
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
    let delete_file = params["deleteFile"]
        .as_bool()
        .or_else(|| params["delete_file"].as_bool())
        .unwrap_or(false);

    let removed = state
        .downloads
        .lock()
        .await
        .remove_shared_file(file_hash, delete_file)?;
    if removed {
        emit_shares_changed(state);
    }
    Ok(json!({ "removed": removed }))
}

/// `shares.rename` — renomme un fichier partagé complet (`{fileHash,
/// newName}`) : fichier disque + entrée `shared_files` (et le download
/// complété associé s'il existe).
pub async fn handle_shares_rename(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let file_hash = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash'"))?;
    let new_name = params["newName"]
        .as_str()
        .or_else(|| params["new_name"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'newName'"))?;

    state
        .downloads
        .lock()
        .await
        .rename_shared_file(file_hash, new_name)?;
    emit_shares_changed(state);
    Ok(json!({ "renamed": true }))
}

/// `shares.rescan` — re-scans the configured download (Incoming) directory,
/// registers newly found files as shared and drops shares whose file was
/// deleted from disk, like eMule's manual reload of the shared-files list.
/// Honors the `share_download_dir` opt-out.
pub async fn handle_shares_rescan(state: &AppState) -> Result<Value, JsonRpcError> {
    let dir = {
        let dl = state.downloads.lock().await;
        dl.get_setting("default_download_directory")?
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| state.config.default_download_dir.clone())
    };
    let report =
        crate::services::incoming_share::scan_download_dir_shares(&state.downloads, dir.clone())
            .await;
    if report.added + report.removed > 0 {
        emit_shares_changed(state);
    }
    Ok(json!({
        "added": report.added,
        "removed": report.removed,
        "dir": dir.to_string_lossy(),
    }))
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
    match file_opt {
        Some(ref mut sf) if sf.is_partial => {
            // Persisting a transient partial share would freeze its `.part`
            // path in `shared_files` and serve it later as a complete file.
            // eMule does not allow unsharing an in-progress part file.
            Err(JsonRpcError::business(
                "Cannot enable/disable a partial share (download in progress)",
            ))
        }
        Some(ref mut sf) => {
            sf.enabled = enabled;
            dl_svc.add_shared_file(sf)?;
            drop(dl_svc);
            emit_shares_changed(state);
            Ok(crate::rpc::handlers::json_value(&sf))
        }
        None => Err(JsonRpcError::business("Shared file not found")),
    }
}

/// `shares.setUploadPriority` — eMule upload priority for a shared file:
/// "verylow" | "low" | "normal" | "high" | "veryhigh" | "powershare" |
/// "auto". "auto" est eMule `m_bAutoUpPriority` : la priorité effective
/// est recalculée selon le nombre de clients qui uploadent le fichier
/// (`UpdateAutoUpPriority` : >20 → low, >1 → normal, sinon → high).
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
        "verylow" | "low" | "normal" | "high" | "veryhigh" | "powershare" | "auto" => {}
        _ => return Err(JsonRpcError::invalid_params("Invalid priority")),
    }
    let mut dl_svc = state.downloads.lock().await;
    let mut file_opt = dl_svc.get_shared_file(file_hash)?;
    match file_opt {
        Some(ref mut sf) if sf.is_partial => Err(JsonRpcError::business(
            "Cannot set upload priority on a partial share (download in progress)",
        )),
        Some(ref mut sf) => {
            // Resolve-then-upsert like setEnabled: also works on completed
            // downloads that were never persisted in `shared_files`.
            sf.up_priority = priority.clone();
            dl_svc.add_shared_file(sf)?;
            drop(dl_svc);
            emit_shares_changed(state);
            Ok(json!({ "ok": true, "fileHash": file_hash, "upPriority": priority }))
        }
        None => Err(JsonRpcError::business("Shared file not found")),
    }
}

/// `shares.setCategory` — associe (ou retire avec `categoryId: null`) une
/// catégorie à un fichier partagé. Fonctionne aussi sur les partages
/// synthétisés depuis un download (partiel ou complété) : la catégorie du
/// download fait alors foi et est mise à jour.
pub async fn handle_shares_set_category(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let file_hash = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash'"))?;
    let category_id = params["categoryId"].as_str().map(String::from);

    state
        .downloads
        .lock()
        .await
        .set_shared_file_category(file_hash, category_id)?;
    emit_shares_changed(state);
    Ok(json!({ "ok": true, "fileHash": file_hash }))
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
    Ok(crate::rpc::handlers::json_value(&snap))
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
    let status = crate::rpc::handlers::json_value(&id_guard.status());
    let sui = crate::rpc::handlers::json_value(&state.sui_stats.snapshot());
    match status {
        Value::Object(mut m) => {
            m.insert("sui".to_string(), sui);
            Ok(Value::Object(m))
        }
        other => Ok(json!({ "identity": other, "sui": sui })),
    }
}

pub async fn handle_identity_rotate(state: &AppState) -> Result<Value, JsonRpcError> {
    let status = state
        .rotate_identity()
        .await
        .map_err(JsonRpcError::business)?;
    Ok(crate::rpc::handlers::json_value(&status))
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
