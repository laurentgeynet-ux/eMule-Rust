use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::JsonRpcError;
use crate::rpc::methods::DAEMON_VERSION;
use crate::state::AppState;

pub async fn handle_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let dl_count = state.downloads.lock().await.list_downloads().len();
    let capabilities = vec![
        "downloads",
        "download-events",
        "history",
        "categories",
        "import_part_met",
        "network-policy",
        "ip-filter",
        "rate-limits",
        "servers",
        "server-connection",
        "server-search",
        "server-sources",
        "peer-sources",
        "peer-connection",
        "peer-robustness",
        "swarm-dual-source",
        "shared-files",
        "upload-single-slot",
        "secure-ident",
        "peer-credits",
        "kad-contacts",
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

pub async fn handle_stop(state: &AppState) -> Result<Value, JsonRpcError> {
    let notify = state.shutdown_notify.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        notify.notify_waiters();
    });
    Ok(json!({ "stopping": true }))
}

pub async fn handle_settings_get(state: &AppState) -> Result<Value, JsonRpcError> {
    let (custom_dir, retention_days, max_global, max_per_file, min_per_file) = {
        let dls = state.downloads.lock().await;
        let dir = dls.get_setting("default_download_directory")?;
        let ret = dls
            .get_setting("history_retention_days")?
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(30);
        let max_g = dls
            .get_setting("max_global_download_peers")?
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(state.config.max_global_download_peers);
        let max_f = dls
            .get_setting("max_peers_per_download")?
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(state.config.max_peers_per_download);
        let min_f = dls
            .get_setting("min_peers_per_download")?
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(state.config.min_peers_per_download);
        (dir, ret, max_g, max_f, min_f)
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
        "historyRetentionDays": retention_days,
        "maxGlobalDownloadPeers": max_global,
        "maxPeersPerDownload": max_per_file,
        "minPeersPerDownload": min_per_file,
    }))
}

pub async fn handle_settings_set(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let mut updated_dir = None;
    let mut updated_retention = None;
    let mut updated_max_global = None;
    let mut updated_max_per_file = None;
    let mut updated_min_per_file = None;

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

    if let Some(val) = params.get("maxGlobalDownloadPeers") {
        let max_g = val.as_u64().ok_or_else(|| {
            JsonRpcError::invalid_params("Invalid 'maxGlobalDownloadPeers' parameter")
        })? as usize;
        let mut dls = state.downloads.lock().await;
        dls.set_setting("max_global_download_peers", &max_g.to_string())?;
        updated_max_global = Some(max_g);
    }

    if let Some(val) = params.get("maxPeersPerDownload") {
        let max_f = val.as_u64().ok_or_else(|| {
            JsonRpcError::invalid_params("Invalid 'maxPeersPerDownload' parameter")
        })? as usize;
        let mut dls = state.downloads.lock().await;
        dls.set_setting("max_peers_per_download", &max_f.to_string())?;
        updated_max_per_file = Some(max_f);
    }

    if let Some(val) = params.get("minPeersPerDownload") {
        let min_f = val.as_u64().ok_or_else(|| {
            JsonRpcError::invalid_params("Invalid 'minPeersPerDownload' parameter")
        })? as usize;
        let mut dls = state.downloads.lock().await;
        dls.set_setting("min_peers_per_download", &min_f.to_string())?;
        updated_min_per_file = Some(min_f);
    }

    if updated_max_global.is_some() || updated_max_per_file.is_some() {
        let max_g = updated_max_global.unwrap_or(state.config.max_global_download_peers);
        let max_f = updated_max_per_file.unwrap_or(state.config.max_peers_per_download);
        state.peer_session.set_swarm_limits(max_f, max_g).await;
    }

    Ok(json!({
        "success": true,
        "defaultDownloadDir": updated_dir,
        "historyRetentionDays": updated_retention,
        "maxGlobalDownloadPeers": updated_max_global,
        "maxPeersPerDownload": updated_max_per_file,
        "minPeersPerDownload": updated_min_per_file,
    }))
}

pub async fn handle_settings_set_retention_days(
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

pub async fn handle_settings_reset_download_dir(state: &AppState) -> Result<Value, JsonRpcError> {
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
