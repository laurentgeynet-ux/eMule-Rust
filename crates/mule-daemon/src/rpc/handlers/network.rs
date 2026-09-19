use serde_json::{json, Value};

use crate::error::JsonRpcError;
use crate::state::AppState;

pub async fn handle_network_get_policy(state: &AppState) -> Result<Value, JsonRpcError> {
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

pub async fn handle_network_set_profile(
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

pub async fn handle_network_disconnect_all(state: &AppState) -> Result<Value, JsonRpcError> {
    state.network_policy.disconnect_all();
    let _ = state.server_session.disconnect().await;
    let _ = state.peer_session.disconnect().await;
    state.inbound_listener.stop().await;
    state.abort_all_tasks().await;
    Ok(json!({
        "success": true,
        "state": state.network_policy.state().as_str()
    }))
}

pub async fn handle_network_enable(state: &AppState) -> Result<Value, JsonRpcError> {
    state.network_policy.enable();
    Ok(json!({
        "success": true,
        "state": state.network_policy.state().as_str()
    }))
}

pub async fn handle_network_get_audit_log(
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

pub async fn handle_network_set_budgets(
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

pub async fn handle_filter_get_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let stats = state.network_policy.ip_filter().stats();
    Ok(serde_json::to_value(stats).unwrap_or_default())
}

pub async fn handle_filter_import_file(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let path_str = params["filePath"]
        .as_str()
        .or_else(|| params["path"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'filePath' or 'path'"))?;

    let path = std::path::Path::new(path_str);
    if !path.exists() {
        return Err(JsonRpcError::invalid_params(&format!(
            "Specified file does not exist: {}",
            path_str
        )));
    }

    let report = state
        .network_policy
        .ip_filter()
        .load_from_file(path)
        .map_err(|e| {
            JsonRpcError::invalid_params(&format!("Failed to parse ipfilter file: {e}"))
        })?;

    Ok(json!({
        "success": true,
        "report": report,
        "stats": state.network_policy.ip_filter().stats()
    }))
}

pub async fn handle_filter_set_enabled(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let enabled = params["enabled"]
        .as_bool()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'enabled' boolean"))?;

    state.network_policy.ip_filter().set_enabled(enabled);
    handle_filter_get_status(state).await
}

pub async fn handle_filter_clear(state: &AppState) -> Result<Value, JsonRpcError> {
    state.network_policy.ip_filter().clear();
    handle_filter_get_status(state).await
}

pub async fn handle_limits_get(state: &AppState) -> Result<Value, JsonRpcError> {
    let limiters = state.network_policy.rate_limiters();
    let download = limiters.download_bucket.rate_per_sec();
    let upload = limiters.upload_bucket.rate_per_sec();
    let conn_per_sec = limiters.conn_bucket.rate_per_sec();
    let server_req_per_sec = limiters.server_bucket.rate_per_sec();
    let kad_pkt_per_sec = limiters.kad_bucket.rate_per_sec();

    let uss = state.uss.snapshot();
    let uss_cfg = state.uss.config();

    Ok(json!({
        "downloadBytesPerSec": download,
        "uploadBytesPerSec": upload,
        "connectionsPerMinute": conn_per_sec * 60,
        "serverRequestsPerSec": server_req_per_sec,
        "kadPacketsPerSec": kad_pkt_per_sec,
        "uss": {
            "enabled": uss.enabled,
            "state": uss.state,
            "currentLimitBytesPerSec": uss.current_limit_bps,
            "latencyMs": uss.latency_ms,
            "lowestPingMs": uss.lowest_ping_ms,
            "acceptNewClient": uss.accept_new_client,
            "pingTarget": uss.ping_target,
            "config": uss_cfg,
        }
    }))
}

pub async fn handle_limits_set(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let mut cfg = mule_filter::RateLimitsConfig::default();

    let dl_bytes = params["downloadBytesPerSec"]
        .as_u64()
        .or_else(|| params["downloadLimit"].as_u64().map(|kb| kb * 1024));
    if let Some(dl) = dl_bytes {
        cfg.download_bytes_per_sec = dl;
    }

    let ul_bytes = params["uploadBytesPerSec"]
        .as_u64()
        .or_else(|| params["uploadLimit"].as_u64().map(|kb| kb * 1024));
    if let Some(ul) = ul_bytes {
        cfg.upload_bytes_per_sec = ul;
        state.uss.note_manual_limit(ul);
    }
    if let Some(cpm) = params["connectionsPerMinute"].as_u64() {
        cfg.connections_per_minute = cpm as u32;
    }
    if let Some(s) = params["serverRequestsPerSec"].as_u64() {
        cfg.server_requests_per_sec = s as u32;
    }
    if let Some(k) = params["kadPacketsPerSec"].as_u64() {
        cfg.kad_packets_per_sec = k as u32;
    }

    state.network_policy.rate_limiters().apply_config(&cfg);

    // Optional USS (Upload Speed Sense) config update.
    if let Some(uss_params) = params.get("uss") {
        let enabled = uss_params["enabled"].as_bool();
        let ping_host = uss_params["pingHost"].as_str().map(|s| s.to_string());
        let min_up = uss_params["minUploadBytesPerSec"].as_u64();
        let max_up = uss_params["maxUploadBytesPerSec"].as_u64();
        let use_ms = uss_params["useMillisecondTolerance"].as_bool();
        let tol_pct = uss_params["pingTolerancePercent"].as_u64();
        let tol_ms = uss_params["pingToleranceMs"].as_u64();
        let up_div = uss_params["goingUpDivider"].as_u64();
        let down_div = uss_params["goingDownDivider"].as_u64();
        let n_pings = uss_params["numberOfPings"].as_u64();
        state
            .uss
            .update_config(|cfg| {
                if let Some(v) = enabled {
                    cfg.enabled = v;
                }
                if uss_params.get("pingHost").is_some() {
                    cfg.ping_host = ping_host;
                }
                if let Some(v) = min_up {
                    cfg.min_upload_bps = v;
                }
                if let Some(v) = max_up {
                    cfg.max_upload_bps = v;
                }
                if let Some(v) = use_ms {
                    cfg.use_ms_tolerance = v;
                }
                if let Some(v) = tol_pct {
                    cfg.ping_tolerance_percent = v as u32;
                }
                if let Some(v) = tol_ms {
                    cfg.ping_tolerance_ms = v as u32;
                }
                if let Some(v) = up_div {
                    cfg.going_up_divider = v as u32;
                }
                if let Some(v) = down_div {
                    cfg.going_down_divider = v as u32;
                }
                if let Some(v) = n_pings {
                    cfg.number_of_pings = (v as usize).max(1);
                }
            })
            .await;
    }

    handle_limits_get(state).await
}
