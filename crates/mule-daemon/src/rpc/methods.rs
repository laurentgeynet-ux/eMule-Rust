use mule_core::AllocationPolicy;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::JsonRpcError;
use crate::rpc::types::{JsonRpcRequest, JsonRpcResponse};
use crate::state::AppState;

pub const PROTOCOL_VERSION: u32 = 1;
pub const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn handle_request(
    state: &AppState,
    req: JsonRpcRequest,
    is_authenticated: &mut bool,
) -> Option<JsonRpcResponse> {
    let req_id = req.id?; // Notifications do not receive responses

    // Handshake check: the first call must be system.hello unless already authenticated
    if req.method == "system.hello" {
        let resp = handle_hello(state, req.params, is_authenticated);
        return Some(match resp {
            Ok(res) => JsonRpcResponse::success(req_id, res),
            Err(e) => JsonRpcResponse::error(req_id, e),
        });
    }

    if !*is_authenticated {
        return Some(JsonRpcResponse::error(req_id, JsonRpcError::auth_failed()));
    }

    let result: Result<Value, JsonRpcError> = match req.method.as_str() {
        "system.status" => handle_status(state).await,
        "system.stop" | "system.shutdown" => handle_stop(state).await,
        "downloads.list" => handle_downloads_list(state).await,
        "downloads.get" => handle_downloads_get(state, req.params).await,
        "downloads.addEd2kLink" => handle_add_ed2k_link(state, req.params).await,
        "downloads.pause" => handle_pause(state, req.params).await,
        "downloads.resume" => handle_resume(state, req.params).await,
        "downloads.remove" => handle_remove(state, req.params).await,
        "downloads.setDestination" => handle_set_destination(state, req.params).await,
        "downloads.inspectPartMet" => handle_inspect_part_met(state, req.params).await,
        "downloads.importPartMet" => handle_import_part_met(state, req.params).await,
        "downloads.retryConnection" | "downloads.retry" => {
            handle_retry_connection(state, req.params).await
        }
        "history.list" => handle_history_list(state, req.params).await,
        "history.stats" => handle_history_stats(state).await,
        "history.clear" => handle_history_clear(state).await,
        "history.export" => handle_history_export(state, req.params).await,
        "settings.get" => handle_settings_get(state).await,
        "settings.set" => handle_settings_set(state, req.params).await,
        "settings.setRetentionDays" => handle_settings_set_retention_days(state, req.params).await,
        "settings.resetDownloadDir" => handle_settings_reset_download_dir(state).await,
        "categories.list" => handle_categories_list(state).await,
        "categories.create" => handle_categories_create(state, req.params).await,
        "categories.update" => handle_categories_update(state, req.params).await,
        "categories.delete" => handle_categories_delete(state, req.params).await,
        "categories.reorder" => handle_categories_reorder(state, req.params).await,
        "downloads.setCategory" => handle_downloads_set_category(state, req.params).await,
        "network.getPolicy" => handle_network_get_policy(state).await,
        "network.setProfile" => handle_network_set_profile(state, req.params).await,
        "network.disconnectAll" => handle_network_disconnect_all(state).await,
        "network.enable" => handle_network_enable(state).await,
        "network.getAuditLog" => handle_network_get_audit_log(state, req.params).await,
        "network.setBudgets" => handle_network_set_budgets(state, req.params).await,
        "events.subscribe" => Ok(json!({ "subscribed": true })),
        #[cfg(any(feature = "debug-rpc", feature = "lab-tools"))]
        "debug.injectRange" => handle_inject_range(state, req.params).await,
        "downloads.getSources" | "ed2k.getSources" => {
            handle_get_sources(state, req.params).await
        }
        #[cfg(feature = "lab-tools")]
        "lab.addPeerSource" | "downloads.addSource" => {
            handle_lab_add_peer_source(state, req.params).await
        }
        #[cfg(feature = "lab-tools")]
        "lab.status" => handle_lab_status(state).await,
        #[cfg(feature = "lab-tools")]
        "lab.createFixture" => handle_lab_create_fixture(state, req.params).await,
        #[cfg(feature = "lab-tools")]
        "lab.startLoopbackPeer" => handle_lab_start_loopback_peer(state, req.params).await,
        #[cfg(feature = "lab-tools")]
        "lab.stopLoopbackPeer" => handle_lab_stop_loopback_peer(state).await,
        #[cfg(feature = "lab-tools")]
        "lab.cleanupTestData" => handle_lab_cleanup_test_data(state).await,
        #[cfg(feature = "ed2k-loopback")]
        "ed2k.probeSource" | "downloads.probeEd2kSource" => {
            handle_ed2k_probe_source(state, req.params).await
        }
        #[cfg(feature = "ed2k-loopback")]
        "ed2k.startDownload" | "downloads.startEd2kDownload" => {
            handle_ed2k_start_download(state, req.params).await
        }
        unknown => Err(JsonRpcError::method_not_found(unknown)),
    };

    Some(match result {
        Ok(res) => JsonRpcResponse::success(req_id, res),
        Err(err) => JsonRpcResponse::error(req_id, err),
    })
}

fn handle_hello(
    state: &AppState,
    params: Option<Value>,
    is_authenticated: &mut bool,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let proto_ver = params["protocolVersion"].as_u64().unwrap_or(0) as u32;
    if proto_ver != PROTOCOL_VERSION {
        return Err(JsonRpcError::protocol_mismatch(PROTOCOL_VERSION, proto_ver));
    }

    // Check token if provided
    let token = params["token"].as_str().unwrap_or("");
    if !state.config.auth_token.is_empty() && token != state.config.auth_token {
        return Err(JsonRpcError::auth_failed());
    }

    *is_authenticated = true;

    let capabilities = vec![
        "downloads",
        "download-events",
        "history",
        "categories",
        "import_part_met",
        "network-policy",
        #[cfg(feature = "lab-tools")]
        "lab-tools",
    ];

    Ok(json!({
        "daemonVersion": DAEMON_VERSION,
        "protocolVersion": PROTOCOL_VERSION,
        "instanceId": state.instance_id.to_string(),
        "sessionId": state.session_id.to_string(),
        "capabilities": capabilities,
        "workdirMode": format!("{:?}", state.config.workdir_mode),
        "workdirPath": state.config.data_dir.to_string_lossy(),
        "defaultDownloadDir": state.config.default_download_dir.to_string_lossy(),
    }))
}

async fn handle_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let dl_count = state.downloads.lock().await.list_downloads().len();
    let capabilities = vec![
        "downloads",
        "download-events",
        "history",
        "categories",
        "import_part_met",
        "network-policy",
        #[cfg(feature = "lab-tools")]
        "lab-tools",
    ];

    let default_dl_dir = {
        state
            .downloads
            .lock()
            .await
            .get_setting("default_download_directory")
            .ok()
            .flatten()
            .unwrap_or_else(|| state.config.default_download_dir.to_string_lossy().to_string())
    };

    Ok(json!({
        "version": DAEMON_VERSION,
        "status": "healthy",
        "downloadsCount": dl_count,
        "instanceId": state.instance_id.to_string(),
        "capabilities": capabilities,
        "workdirMode": format!("{:?}", state.config.workdir_mode),
        "workdirPath": state.config.data_dir.to_string_lossy(),
        "defaultDownloadDir": default_dl_dir,
        "networkState": state.network_policy.state().as_str(),
        "networkProfile": state.network_policy.profile().to_string(),
    }))
}

async fn handle_stop(state: &AppState) -> Result<Value, JsonRpcError> {
    let notify = state.shutdown_notify.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        notify.notify_waiters();
    });
    Ok(json!({ "stopping": true }))
}

async fn handle_downloads_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let seq = state.sequence.load(std::sync::atomic::Ordering::SeqCst);
    let list = state.downloads.lock().await.list_downloads();
    Ok(json!({
        "sequence": seq,
        "downloads": list
    }))
}

async fn handle_downloads_get(
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

async fn handle_add_ed2k_link(
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
    let effective_base_dir = global_db_dir.clone().unwrap_or_else(|| fallback_bundle_dir.clone());

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

    let snapshot = state
        .downloads
        .lock()
        .await
        .add_ed2k_link(link, &target_dir, policy, category_id_opt)?;

    state.broadcast_download_update(snapshot.clone());

    Ok(json!({
        "downloadId": snapshot.id.to_string(),
        "status": snapshot.status
    }))
}

async fn handle_pause(state: &AppState, params: Option<Value>) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let snapshot = state.downloads.lock().await.pause_download(&id)?;
    state.broadcast_download_update(snapshot.clone());

    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

async fn handle_resume(state: &AppState, params: Option<Value>) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let snapshot = state.downloads.lock().await.resume_download(&id)?;
    state.broadcast_download_update(snapshot.clone());

    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

async fn handle_remove(state: &AppState, params: Option<Value>) -> Result<Value, JsonRpcError> {
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

async fn handle_set_destination(
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

async fn handle_inspect_part_met(
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

async fn handle_import_part_met(
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
    let effective_base_dir = global_db_dir.clone().unwrap_or_else(|| fallback_bundle_dir.clone());

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

    let result = state
        .downloads
        .lock()
        .await
        .import_part_met(met_path, part_path, &target_dir, category_id_opt)?;

    state.broadcast_download_update(result.download.clone());

    Ok(serde_json::to_value(result).unwrap_or_default())
}


async fn handle_retry_connection(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let detail = state
        .downloads
        .lock()
        .await
        .retry_connection(&id)?;

    state.broadcast_download_update(detail.snapshot.clone());

    Ok(serde_json::to_value(detail).unwrap_or_default())
}

async fn handle_settings_get(state: &AppState) -> Result<Value, JsonRpcError> {
    let (custom_dir, retention_days) = {
        let dls = state.downloads.lock().await;
        let dir = dls.get_setting("default_download_directory")?;
        let ret = dls
            .get_setting("history_retention_days")?
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(30);
        (dir, ret)
    };

    let current_dir = custom_dir
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| state.config.default_download_dir.clone());

    let portable_default =
        crate::workdir::WorkDirectoryResolver::resolve(None, None).default_download_dir;

    Ok(json!({
        "defaultDownloadDir": current_dir.to_string_lossy(),
        "portableDownloadDir": portable_default.to_string_lossy(),
        "isCustom": custom_dir.is_some(),
        "historyRetentionDays": retention_days
    }))
}

async fn handle_settings_set(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let mut updated_dir = None;
    let mut updated_retention = None;

    if let Some(dir_str) = params["defaultDownloadDir"].as_str() {
        let dir = std::path::PathBuf::from(dir_str);
        // Validate directory is creatable & writable
        std::fs::create_dir_all(&dir)
            .map_err(|e| JsonRpcError::business(format!("Cannot create directory: {e}")))?;

        let probe = dir.join(format!(".mule_write_probe_{}", Uuid::new_v4()));
        match std::fs::write(&probe, b"probe") {
            Ok(_) => {
                let _ = std::fs::remove_file(&probe);
            }
            Err(e) => {
                return Err(JsonRpcError::business(format!("Directory is not writable: {e}")));
            }
        }

        state
            .downloads
            .lock()
            .await
            .set_setting("default_download_directory", dir_str)?;
        updated_dir = Some(dir_str.to_string());
    }

    if let Some(ret_days_val) = params.get("historyRetentionDays") {
        let ret_days = ret_days_val.as_u64().ok_or_else(|| {
            JsonRpcError::invalid_params("Invalid 'historyRetentionDays' parameter")
        })? as u32;
        let mut dls = state.downloads.lock().await;
        dls.set_setting("history_retention_days", &ret_days.to_string())?;
        dls.apply_retention_policy(ret_days)?;
        updated_retention = Some(ret_days);
    }

    Ok(json!({
        "success": true,
        "defaultDownloadDir": updated_dir,
        "historyRetentionDays": updated_retention
    }))
}

async fn handle_settings_set_retention_days(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let retention_days = params["retentionDays"]
        .as_u64()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'retentionDays' parameter"))? as u32;

    let mut dls = state.downloads.lock().await;
    dls.set_setting("history_retention_days", &retention_days.to_string())?;
    let pruned_count = dls.apply_retention_policy(retention_days)?;

    Ok(json!({
        "success": true,
        "retentionDays": retention_days,
        "prunedCount": pruned_count
    }))
}

async fn handle_history_list(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let filter = if let Some(p) = params {
        serde_json::from_value::<mule_core::HistoryFilter>(p)
            .unwrap_or_default()
    } else {
        mule_core::HistoryFilter::default()
    };

    let entries = state.downloads.lock().await.query_history(&filter)?;
    Ok(serde_json::to_value(entries).unwrap_or_default())
}

async fn handle_history_stats(state: &AppState) -> Result<Value, JsonRpcError> {
    let stats = state.downloads.lock().await.get_history_stats()?;
    Ok(serde_json::to_value(stats).unwrap_or_default())
}

async fn handle_history_clear(state: &AppState) -> Result<Value, JsonRpcError> {
    let cleared_count = state.downloads.lock().await.clear_history()?;
    Ok(json!({
        "success": true,
        "clearedCount": cleared_count
    }))
}

async fn handle_history_export(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let format = params["format"]
        .as_str()
        .unwrap_or("json");

    let filter = serde_json::from_value::<mule_core::HistoryFilter>(params.clone())
        .unwrap_or_default();

    let dls = state.downloads.lock().await;
    let content = dls.export_history(format, &filter)?;
    let entries = dls.query_history(&filter)?;

    Ok(json!({
        "format": format.to_ascii_lowercase(),
        "content": content,
        "count": entries.len()
    }))
}

async fn handle_settings_reset_download_dir(state: &AppState) -> Result<Value, JsonRpcError> {
    state
        .downloads
        .lock()
        .await
        .remove_setting("default_download_directory")?;

    let portable_default =
        crate::workdir::WorkDirectoryResolver::resolve(None, None).default_download_dir;

    Ok(json!({
        "success": true,
        "defaultDownloadDir": portable_default.to_string_lossy(),
        "isCustom": false
    }))
}

#[cfg(any(feature = "debug-rpc", feature = "lab-tools"))]
async fn handle_inject_range(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;
    let offset = params["offset"]
        .as_u64()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'offset' parameter"))?;

    let data_hex = params["dataHex"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'dataHex' parameter"))?;

    let data = hex::decode(data_hex).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let snapshot = state
        .downloads
        .lock()
        .await
        .inject_range(&id, offset, &data)?;
    state.broadcast_download_update(snapshot.clone());

    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

#[cfg(feature = "lab-loopback")]
async fn handle_lab_add_peer_source(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["downloadId"]
        .as_str()
        .or_else(|| params["id"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'downloadId' or 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let peer_addr_str = params["peerAddr"]
        .as_str()
        .or_else(|| params["source"].as_str())
        .or_else(|| params["endpoint"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'peerAddr' parameter"))?;
    let peer_addr: std::net::SocketAddr = peer_addr_str
        .parse()
        .map_err(|e| JsonRpcError::invalid_params(&format!("Invalid socket address: {e}")))?;

    // Spawn the lab downloader task
    let state_clone = state.clone();
    let handle = tokio::spawn(async move {
        if let Err(e) = crate::lab::run_lab_download(id, peer_addr, state_clone).await {
            tracing::error!(download_id = %id, error = %e, "Lab download worker failed");
        }
    });
    state.register_task(handle.abort_handle()).await;

    Ok(json!({
        "started": true,
        "downloadId": id.to_string(),
        "peerAddr": peer_addr.to_string(),
    }))
}

#[cfg(feature = "ed2k-loopback")]
async fn handle_ed2k_probe_source(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["downloadId"]
        .as_str()
        .or_else(|| params["id"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'downloadId' or 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let peer_addr_str = params["peerAddr"]
        .as_str()
        .or_else(|| params["peerAddress"].as_str())
        .or_else(|| params["source"].as_str())
        .or_else(|| params["endpoint"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'peerAddr' parameter"))?;
    let peer_addr: std::net::SocketAddr = peer_addr_str
        .parse()
        .map_err(|e| JsonRpcError::invalid_params(&format!("Invalid socket address: {e}")))?;

    if !peer_addr.ip().is_loopback() {
        return Err(JsonRpcError::invalid_params(
            "Non-loopback peer address is strictly forbidden in Phase 6.3",
        ));
    }

    let (temp_path, _, ed2k_hash, _total_size) = state
        .downloads
        .lock()
        .await
        .get_download_file_info(&id)
        .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

    let size_before = std::fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0);

    let local_hash = [0x11; 16];
    let (mut client, _info) = mule_protocol::ed2k::Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        local_hash,
        0x7F000001,
        4662,
        vec![
            mule_protocol::ed2k::Ed2kTag {
                name: mule_protocol::ed2k::TagName::Id(mule_protocol::ed2k::TAG_SPECIAL_NAME),
                value: mule_protocol::ed2k::TagValue::String("DaemonLoopbackClient".to_string()),
            },
            mule_protocol::ed2k::Ed2kTag {
                name: mule_protocol::ed2k::TagName::Id(mule_protocol::ed2k::TAG_SPECIAL_VERSION),
                value: mule_protocol::ed2k::TagValue::UInt32(60),
            },
        ],
        0x7F000001,
        4661,
        std::time::Duration::from_millis(1000),
    )
    .await
    .map_err(|e| JsonRpcError::business(format!("eD2K handshake failed: {e}")))?;

    let status = client
        .request_file_status(ed2k_hash.0, std::time::Duration::from_millis(1000))
        .await
        .map_err(|e| JsonRpcError::business(format!("eD2K file status request failed: {e}")))?;

    client.close().await;

    let snapshot = state
        .downloads
        .lock()
        .await
        .record_source_availability(&id, peer_addr, status.availability_mask.clone())?;

    let size_after = std::fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0);
    if size_after != size_before {
        return Err(JsonRpcError::business(
            "CRITICAL: .part file modified during file status probe!",
        ));
    }

    state.broadcast_download_update(snapshot.clone());

    let available_parts: Vec<u16> = status.availability_mask.available_parts().collect();

    Ok(json!({
        "success": true,
        "downloadId": id.to_string(),
        "peerAddr": peer_addr.to_string(),
        "sourceCount": snapshot.source_count,
        "partCount": status.part_count,
        "availableParts": available_parts,
    }))
}

async fn handle_get_sources(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["id"]
        .as_str()
        .or_else(|| params["downloadId"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'id' or 'downloadId' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let summaries = state
        .downloads
        .lock()
        .await
        .get_source_summaries(&id)
        .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

    Ok(json!({
        "downloadId": id.to_string(),
        "sources": summaries,
    }))
}

#[cfg(feature = "ed2k-loopback")]
async fn handle_ed2k_start_download(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let id_str = params["downloadId"]
        .as_str()
        .or_else(|| params["id"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'downloadId' or 'id' parameter"))?;
    let id = Uuid::parse_str(id_str).map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let peer_addr_str = params["peerAddr"]
        .as_str()
        .or_else(|| params["peerAddress"].as_str())
        .or_else(|| params["source"].as_str())
        .or_else(|| params["endpoint"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'peerAddr' parameter"))?;
    let peer_addr: std::net::SocketAddr = peer_addr_str
        .parse()
        .map_err(|e| JsonRpcError::invalid_params(&format!("Invalid socket address: {e}")))?;

    if !peer_addr.ip().is_loopback() {
        return Err(JsonRpcError::invalid_params(
            "Non-loopback peer address is strictly forbidden in Phase 6.4",
        ));
    }

    let state_clone = state.clone();
    let handle = tokio::spawn(async move {
        if let Err(e) = crate::ed2k::run_ed2k_download(id, peer_addr, state_clone).await {
            tracing::error!(download_id = %id, error = %e, "eD2K loopback download worker failed");
        }
    });
    state.register_task(handle.abort_handle()).await;

    Ok(json!({
        "started": true,
        "downloadId": id.to_string(),
        "peerAddr": peer_addr.to_string(),
    }))
}

#[cfg(feature = "lab-tools")]
async fn handle_lab_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let fixtures_dir = state.config.data_dir.join("fixtures");
    let mut fixtures = Vec::new();

    if fixtures_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&fixtures_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    if size > 0 {
                        if let Ok(mut f) = std::fs::File::open(&path) {
                            if let Ok(hash_res) = mule_format::hash_reader(&mut f) {
                                if let Ok(link) = mule_format::ed2k::Ed2kFileLink::create(
                                    &name,
                                    size,
                                    hash_res.root_hash,
                                ) {
                                    fixtures.push(json!({
                                        "fileName": name,
                                        "filePath": path.to_string_lossy(),
                                        "sizeBytes": size,
                                        "ed2kHash": hash_res.root_hash.to_hex(),
                                        "ed2kLink": link.to_string(),
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let active_peer_lock = state.active_lab_peer.lock().await;
    let active_peer_addr = active_peer_lock.as_ref().map(|(addr, _)| addr.to_string());

    Ok(json!({
        "workdirMode": format!("{:?}", state.config.workdir_mode),
        "workdirPath": state.config.data_dir.to_string_lossy(),
        "isPortable": state.config.workdir_mode == crate::workdir::WorkDirMode::Portable,
        "activeLoopbackPeer": active_peer_addr,
        "fixtures": fixtures,
    }))
}

#[cfg(feature = "lab-tools")]
async fn handle_lab_create_fixture(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let fixtures_dir = state.config.data_dir.join("fixtures");
    std::fs::create_dir_all(&fixtures_dir)
        .map_err(|e| JsonRpcError::internal(format!("Failed to create fixtures dir: {e}")))?;

    let fixture_type = params
        .as_ref()
        .and_then(|p| p["fixtureType"].as_str())
        .unwrap_or("sample_1mb");

    let (file_name, data): (String, Vec<u8>) = match fixture_type {
        "part_9728000" => {
            let size = 9_728_000usize;
            let mut v = Vec::with_capacity(size);
            for i in 0..size {
                v.push((i % 256) as u8);
            }
            ("fixture_part_9728000.bin".to_string(), v)
        }
        "pattern_9728001" | "part_9728001" => {
            let size = 9_728_001usize;
            let mut v = Vec::with_capacity(size);
            for i in 0..size {
                v.push((i % 256) as u8);
            }
            ("fixture_part_9728001.bin".to_string(), v)
        }
        "text" => ("hello-loopback.txt".to_string(), b"Emule-Sion local test\n".to_vec()),
        _ => {
            let size = 1_048_576usize;
            let mut v = Vec::with_capacity(size);
            for i in 0..size {
                v.push((i % 256) as u8);
            }
            ("sample_1mb.bin".to_string(), v)
        }
    };

    let target_path = fixtures_dir.join(&file_name);
    std::fs::write(&target_path, &data)
        .map_err(|e| JsonRpcError::internal(format!("Failed to write fixture: {e}")))?;

    let mut cursor = std::io::Cursor::new(&data);
    let hash_res = mule_format::hash_reader(&mut cursor)
        .map_err(|e| JsonRpcError::internal(format!("Failed to hash fixture: {e}")))?;

    let link = mule_format::ed2k::Ed2kFileLink::create(
        &file_name,
        data.len() as u64,
        hash_res.root_hash,
    )
    .map_err(|e| JsonRpcError::internal(format!("Failed to format ed2k link: {e}")))?;

    Ok(json!({
        "fileName": file_name,
        "filePath": target_path.to_string_lossy(),
        "sizeBytes": data.len(),
        "ed2kHash": hash_res.root_hash.to_hex(),
        "ed2kLink": link.to_string(),
        "status": "ready"
    }))
}

#[cfg(feature = "lab-tools")]
async fn handle_lab_start_loopback_peer(
    state: &AppState,
    _params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let test_peer =
        mule_protocol::ed2k::Ed2kTestPeer::bind(mule_protocol::ed2k::TestPeerMode::Nominal)
            .await
            .map_err(|e| JsonRpcError::internal(format!("Failed to bind loopback test peer: {e}")))?;
    let peer_addr = test_peer.local_addr();

    let fixtures_dir = state.config.data_dir.join("fixtures");
    let mut shared_count = 0;

    if fixtures_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&fixtures_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(file_data) = std::fs::read(&path) {
                        if !file_data.is_empty() {
                            let mut reader = std::io::Cursor::new(&file_data);
                            if let Ok(hash_res) = mule_format::hash_reader(&mut reader) {
                                let part_count =
                                    file_data.len().div_ceil(9_728_000).max(1) as u16;
                                let available_parts: Vec<u16> = (0..part_count).collect();
                                let mask = mule_protocol::ed2k::PartAvailabilityMask::from_parts(
                                    &available_parts,
                                    part_count,
                                );
                                test_peer
                                    .add_shared_file(hash_res.root_hash.0, part_count, mask)
                                    .await;
                                test_peer
                                    .add_shared_file_data(
                                        hash_res.root_hash.0,
                                        bytes::Bytes::from(file_data),
                                    )
                                    .await;
                                shared_count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    let (handle, _) = test_peer.spawn_service();

    let mut active_peer = state.active_lab_peer.lock().await;
    if let Some((_, old_handle)) = active_peer.take() {
        old_handle.abort();
    }
    *active_peer = Some((peer_addr, handle.abort_handle()));
    state.register_task(handle.abort_handle()).await;

    Ok(json!({
        "peerAddr": peer_addr.to_string(),
        "sharedCount": shared_count,
        "status": "running"
    }))
}

#[cfg(feature = "lab-tools")]
async fn handle_lab_stop_loopback_peer(state: &AppState) -> Result<Value, JsonRpcError> {
    let mut active_peer = state.active_lab_peer.lock().await;
    if let Some((_, handle)) = active_peer.take() {
        handle.abort();
    }
    Ok(json!({ "stopped": true }))
}

#[cfg(feature = "lab-tools")]
async fn handle_lab_cleanup_test_data(state: &AppState) -> Result<Value, JsonRpcError> {
    if state.config.workdir_mode == crate::workdir::WorkDirMode::InstalledDefault {
        return Err(JsonRpcError::forbidden_in_installed_mode(
            "Refusing to clean test data in InstalledDefault mode for safety",
        ));
    }

    let canonical_workdir = match state.config.data_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => state.config.data_dir.clone(),
    };

    // Strict allowlist: only fixtures, temp, and part are eligible for cleanup.
    // Downloads and incoming files are sanctified and never touched.
    let allowed_subfolders = ["fixtures", "temp", "part"];
    let mut cleaned_folders = Vec::new();
    let mut deleted_files_count: usize = 0;
    let mut freed_bytes: u64 = 0;

    for folder_name in allowed_subfolders {
        let target_dir = state.config.data_dir.join(folder_name);
        if !target_dir.exists() {
            continue;
        }

        // Verify target path does not escape canonical workdir
        if let Ok(canonical_target) = target_dir.canonicalize() {
            if !canonical_target.starts_with(&canonical_workdir) {
                tracing::warn!(
                    "Path traversal attempt rejected in lab cleanup: {:?}",
                    target_dir
                );
                return Err(JsonRpcError::unsafe_cleanup_target(format!(
                    "Path traversal attempt rejected in lab cleanup: {:?}",
                    target_dir
                )));
            }
        }

        // Check symlinks / reparse points on the directory itself (do not traverse)
        if let Ok(meta) = std::fs::symlink_metadata(&target_dir) {
            if meta.file_type().is_symlink() {
                tracing::warn!(
                    "Refusing to traverse symlinked lab directory: {:?}",
                    target_dir
                );
                return Err(JsonRpcError::unsafe_cleanup_target(format!(
                    "Refusing to traverse symlinked lab directory: {:?}",
                    target_dir
                )));
            }
        }

        if let Ok(entries) = std::fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if let Ok(meta) = std::fs::symlink_metadata(&entry_path) {
                    if meta.file_type().is_symlink() {
                        let _ = std::fs::remove_file(&entry_path);
                        deleted_files_count += 1;
                    } else if meta.is_file() {
                        freed_bytes += meta.len();
                        if std::fs::remove_file(&entry_path).is_ok() {
                            deleted_files_count += 1;
                        }
                    } else if meta.is_dir() {
                        // Subdirectory inside allowed folder
                        if let Ok(sub_entries) = std::fs::read_dir(&entry_path) {
                            for sub_entry in sub_entries.flatten() {
                                let sub_path = sub_entry.path();
                                if let Ok(sub_meta) = std::fs::symlink_metadata(&sub_path) {
                                    if sub_meta.is_file() {
                                        freed_bytes += sub_meta.len();
                                        if std::fs::remove_file(&sub_path).is_ok() {
                                            deleted_files_count += 1;
                                        }
                                    }
                                }
                            }
                        }
                        let _ = std::fs::remove_dir_all(&entry_path);
                    }
                }
            }
        }

        cleaned_folders.push(folder_name.to_string());
    }

    Ok(json!({
        "success": true,
        "cleanedFolders": cleaned_folders,
        "deletedFilesCount": deleted_files_count,
        "freedBytes": freed_bytes,
        "message": format!(
            "Test data cleanup complete: {deleted_files_count} file(s) removed, {freed_bytes} bytes freed."
        )
    }))
}

// --- Category Handlers ---

async fn handle_categories_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let list = state.downloads.lock().await.list_categories()?;
    serde_json::to_value(list)
        .map_err(|e| JsonRpcError::business(format!("Serialization error: {e}")))
}

async fn handle_categories_create(
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

    let created = state
        .downloads
        .lock()
        .await
        .create_category(name, color, default_dest, default_prio)?;

    serde_json::to_value(created)
        .map_err(|e| JsonRpcError::business(format!("Serialization error: {e}")))
}

async fn handle_categories_update(
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

async fn handle_categories_delete(
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

async fn handle_categories_reorder(
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

async fn handle_downloads_set_category(
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

async fn handle_network_get_policy(state: &AppState) -> Result<Value, JsonRpcError> {
    let profile = state.network_policy.profile();
    let net_state = state.network_policy.state();
    let budgets = state.network_policy.budgets();
    let peer_conns = state.network_policy.active_peer_connections_count();
    let server_conns = state.network_policy.active_server_connections_count();
    let listeners = state.network_policy.active_listeners_count();

    Ok(json!({
        "profile": profile.to_string(),
        "state": net_state.as_str(),
        "budgets": budgets,
        "activeConnections": {
            "peers": peer_conns,
            "servers": server_conns,
            "listeners": listeners
        }
    }))
}

async fn handle_network_set_profile(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let profile_str = params["profile"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'profile' string"))?;
    let profile: mule_network_policy::NetworkProfile = profile_str
        .parse()
        .map_err(|e: String| JsonRpcError::invalid_params(&e))?;

    state.network_policy.set_profile(profile);

    handle_network_get_policy(state).await
}

async fn handle_network_disconnect_all(state: &AppState) -> Result<Value, JsonRpcError> {
    state.network_policy.disconnect_all();
    state.abort_all_tasks().await;
    Ok(json!({
        "success": true,
        "state": state.network_policy.state().as_str()
    }))
}

async fn handle_network_enable(state: &AppState) -> Result<Value, JsonRpcError> {
    state.network_policy.enable();
    Ok(json!({
        "success": true,
        "state": state.network_policy.state().as_str()
    }))
}

async fn handle_network_get_audit_log(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let limit = params
        .as_ref()
        .and_then(|p| p["limit"].as_u64())
        .unwrap_or(50) as usize;
    let limit = limit.clamp(1, 500);

    let entries = state.network_policy.audit_entries(limit);
    Ok(json!({
        "entries": entries
    }))
}

async fn handle_network_set_budgets(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let budgets_val = if params.get("budgets").is_some() {
        &params["budgets"]
    } else {
        &params
    };
    let budgets: mule_network_policy::ResourceBudgets = serde_json::from_value(budgets_val.clone())
        .map_err(|e| JsonRpcError::invalid_params(&format!("Invalid budgets format: {e}")))?;

    state
        .network_policy
        .set_budgets(budgets)
        .map_err(|e| JsonRpcError::invalid_params(&format!("Failed to set budgets: {e}")))?;

    handle_network_get_policy(state).await
}



