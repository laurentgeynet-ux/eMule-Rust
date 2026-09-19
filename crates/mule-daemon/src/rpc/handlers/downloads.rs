use mule_core::AllocationPolicy;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::JsonRpcError;
use crate::state::AppState;

pub async fn handle_downloads_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let seq = state.sequence.load(std::sync::atomic::Ordering::SeqCst);
    let list = state.downloads.lock().await.list_downloads();
    Ok(json!({
        "sequence": seq,
        "downloads": list
    }))
}

pub async fn handle_downloads_get(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let dl = state
        .downloads
        .lock()
        .await
        .get_download_detail(&id)
        .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

    Ok(serde_json::to_value(dl).unwrap_or_default())
}

pub async fn handle_add_ed2k_link(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let link = params["link"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'link' parameter"))?;
    let target_dir_opt = params["targetDirectory"]
        .as_str()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != ".");

    let category_id_opt = params["categoryId"]
        .as_str()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let category_dest = if let Some(ref cid) = category_id_opt {
        state
            .downloads
            .lock()
            .await
            .get_category(cid)
            .ok()
            .flatten()
            .and_then(|c| c.default_destination)
    } else {
        None
    };

    let global_db_dir = {
        state
            .downloads
            .lock()
            .await
            .get_setting("default_download_directory")
            .ok()
            .flatten()
            .map(std::path::PathBuf::from)
    };

    let fallback_bundle_dir = state.config.default_download_dir.clone();
    let effective_base_dir = global_db_dir
        .clone()
        .unwrap_or_else(|| fallback_bundle_dir.clone());

    // 4-Tier Resolution:
    // Tier 1: Explicit targetDirectory
    // Tier 2: Category default destination
    // Tier 3: Global SQLite setting default_download_directory
    // Tier 4: Bundle root default download dir
    let target_dir = if let Some(p) = target_dir_opt {
        let path = std::path::PathBuf::from(p);
        if path.is_absolute() {
            path
        } else {
            effective_base_dir.join(path)
        }
    } else if let Some(cat_dest) = category_dest {
        let path = std::path::PathBuf::from(&cat_dest);
        if path.is_absolute() {
            path
        } else {
            effective_base_dir.join(path)
        }
    } else if let Some(global_dir) = global_db_dir {
        global_dir
    } else {
        fallback_bundle_dir
    };
    let _ = std::fs::create_dir_all(&target_dir);

    let policy = match params["allocationPolicy"].as_str() {
        Some("preallocate") => AllocationPolicy::Preallocate,
        Some("none") => AllocationPolicy::None,
        _ => AllocationPolicy::SparseIfSupported,
    };

    let snapshot =
        state
            .downloads
            .lock()
            .await
            .add_ed2k_link(link, &target_dir, policy, category_id_opt)?;

    state.broadcast_download_update(snapshot.clone());

    // Trigger immediate source query from server and Kad
    let file_info = {
        state
            .downloads
            .lock()
            .await
            .get_download_file_info(&snapshot.id)
    };
    if let Some((_, _, hash, size)) = file_info {
        let hash_hex = hex::encode(hash.0);
        tracing::info!(
            "Download added: requesting immediate sources from server and Kad for file {}",
            hash_hex
        );
        if state.server_session.snapshot().await.status
            == crate::services::server_session::ServerConnectionStatus::Connected
        {
            let req = mule_protocol::ed2k::GetSourcesRequest::with_obfuscation(
                hash.0,
                size,
                state.config.obfuscation_enabled,
            );
            let _ = state.server_session.get_sources(req).await;
        }
        if state.network_policy.profile().allows_udp_kad() {
            let kad = state.kad.clone();
            tokio::spawn(async move {
                let _ = kad.search_sources(&hash_hex, size).await;
            });
        }
    }

    Ok(json!({
        "downloadId": snapshot.id.to_string(),
        "status": snapshot.status
    }))
}

pub async fn handle_pause(state: &AppState, params: Option<Value>) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let snapshot = state.downloads.lock().await.pause_download(&id)?;
    state.broadcast_download_update(snapshot.clone());

    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

pub async fn handle_resume(state: &AppState, params: Option<Value>) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let snapshot = state.downloads.lock().await.resume_download(&id)?;
    state.broadcast_download_update(snapshot.clone());

    // Trigger immediate source query on resume
    let file_info = {
        state
            .downloads
            .lock()
            .await
            .get_download_file_info(&snapshot.id)
    };
    if let Some((_, _, hash, size)) = file_info {
        let hash_hex = hex::encode(hash.0);
        if state.server_session.snapshot().await.status
            == crate::services::server_session::ServerConnectionStatus::Connected
        {
            let req = mule_protocol::ed2k::GetSourcesRequest::with_obfuscation(
                hash.0,
                size,
                state.config.obfuscation_enabled,
            );
            let _ = state.server_session.get_sources(req).await;
        }
        if state.network_policy.profile().allows_udp_kad() {
            let kad = state.kad.clone();
            tokio::spawn(async move {
                let _ = kad.search_sources(&hash_hex, size).await;
            });
        }
    }

    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

pub async fn handle_remove(state: &AppState, params: Option<Value>) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;
    let delete_files = params["deleteFiles"].as_bool().unwrap_or(false);

    state
        .downloads
        .lock()
        .await
        .remove_download(&id, delete_files)?;

    Ok(json!({ "success": true }))
}

pub async fn handle_set_destination(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;
    let target_dir_str = params["targetDirectory"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'targetDirectory' parameter"))?;

    let target_dir = std::path::PathBuf::from(target_dir_str);
    let detail = state
        .downloads
        .lock()
        .await
        .set_destination(&id, &target_dir)?;

    state.broadcast_download_update(detail.snapshot.clone());

    Ok(serde_json::to_value(detail).unwrap_or_default())
}

pub async fn handle_inspect_part_met(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let met_path_str = params["metPath"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'metPath' parameter"))?;
    let part_path_str = params["partPath"].as_str().filter(|s| !s.trim().is_empty());

    let met_path = std::path::Path::new(met_path_str);
    let part_path = part_path_str.map(std::path::Path::new);

    let inspection = state
        .downloads
        .lock()
        .await
        .inspect_part_met(met_path, part_path)?;

    Ok(serde_json::to_value(inspection).unwrap_or_default())
}

pub async fn handle_import_part_met(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let met_path_str = params["metPath"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'metPath' parameter"))?;
    let part_path_str = params["partPath"].as_str().filter(|s| !s.trim().is_empty());

    let target_dir_opt = params["targetDirectory"]
        .as_str()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != ".");

    let category_id_opt = params["categoryId"]
        .as_str()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let category_dest = if let Some(ref cid) = category_id_opt {
        state
            .downloads
            .lock()
            .await
            .get_category(cid)
            .ok()
            .flatten()
            .and_then(|c| c.default_destination)
    } else {
        None
    };

    let global_db_dir = {
        state
            .downloads
            .lock()
            .await
            .get_setting("default_download_directory")
            .ok()
            .flatten()
            .map(std::path::PathBuf::from)
    };

    let fallback_bundle_dir = state.config.default_download_dir.clone();
    let effective_base_dir = global_db_dir
        .clone()
        .unwrap_or_else(|| fallback_bundle_dir.clone());

    let target_dir = if let Some(p) = target_dir_opt {
        let path = std::path::PathBuf::from(p);
        if path.is_absolute() {
            path
        } else {
            effective_base_dir.join(path)
        }
    } else if let Some(cat_dest) = category_dest {
        let path = std::path::PathBuf::from(&cat_dest);
        if path.is_absolute() {
            path
        } else {
            effective_base_dir.join(path)
        }
    } else if let Some(global_dir) = global_db_dir {
        global_dir
    } else {
        fallback_bundle_dir
    };
    let _ = std::fs::create_dir_all(&target_dir);

    let met_path = std::path::Path::new(met_path_str);
    let part_path = part_path_str.map(std::path::Path::new);

    let result = state.downloads.lock().await.import_part_met(
        met_path,
        part_path,
        &target_dir,
        category_id_opt,
    )?;

    state.broadcast_download_update(result.download.clone());

    Ok(serde_json::to_value(result).unwrap_or_default())
}

pub async fn handle_retry_connection(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let detail = state.downloads.lock().await.retry_connection(&id)?;

    state.broadcast_download_update(detail.snapshot.clone());

    Ok(serde_json::to_value(detail).unwrap_or_default())
}

pub async fn handle_downloads_set_category(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;
    let id = Uuid::parse_str(id_str)
        .map_err(|e| JsonRpcError::invalid_params(&format!("Invalid UUID: {e}")))?;

    let category_id = params["categoryId"].as_str().map(String::from);

    let detail = state
        .downloads
        .lock()
        .await
        .set_download_category(&id, category_id)?;

    state.broadcast_download_update(detail.snapshot.clone());

    serde_json::to_value(detail)
        .map_err(|e| JsonRpcError::business(format!("Serialization error: {e}")))
}
