use mule_core::AllocationPolicy;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::JsonRpcError;
use crate::rpc::types::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
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
        "filter.getStatus" | "filter.getStats" => handle_filter_get_status(state).await,
        "filter.importFile" => handle_filter_import_file(state, req.params).await,
        "filter.setEnabled" => handle_filter_set_enabled(state, req.params).await,
        "filter.clear" => handle_filter_clear(state).await,
        "limits.get" => handle_limits_get(state).await,
        "limits.set" => handle_limits_set(state, req.params).await,
        "servers.inspectMet" => handle_servers_inspect_met(state, req.params).await,
        "servers.importMet" => handle_servers_import_met(state, req.params).await,
        "servers.list" => handle_servers_list(state).await,
        "servers.setEnabled" => handle_servers_set_enabled(state, req.params).await,
        "servers.delete" => handle_servers_delete(state, req.params).await,
        "servers.clear" => handle_servers_clear(state).await,
        "servers.connect" => handle_servers_connect(state, req.params).await,
        "servers.disconnect" => handle_servers_disconnect(state).await,
        "servers.status" => handle_servers_status(state).await,
        "servers.search" => handle_servers_search(state, req.params).await,
        "servers.querySources" => handle_servers_query_sources(state, req.params).await,
        "servers.updateFromUrl" => handle_servers_update_from_url(state, req.params).await,
        "sources.list" => handle_sources_list(state, req.params).await,
        "sources.addManual" => handle_sources_add_manual(state, req.params).await,
        "sources.connect" => handle_sources_connect(state, req.params).await,
        "sources.disconnect" => handle_sources_disconnect(state, req.params).await,
        "sources.clear" => handle_sources_clear(state, req.params).await,
        "sources.status" | "sources.swarmStatus" | "swarm.status" => handle_sources_status(state, req.params).await,
        "shares.list" => handle_shares_list(state).await,
        "shares.add" => handle_shares_add(state, req.params).await,
        "shares.remove" => handle_shares_remove(state, req.params).await,
        "shares.setEnabled" => handle_shares_set_enabled(state, req.params).await,
        "listener.start" => handle_listener_start(state, req.params).await,
        "listener.stop" => handle_listener_stop(state).await,
        "listener.status" => handle_listener_status(state).await,
        "uploads.list" => handle_uploads_list(state).await,
        "uploads.status" => handle_uploads_status(state).await,
        "uploads.cancel" => handle_uploads_cancel(state, req.params).await,
        "identity.status" => handle_identity_status(state).await,
        "identity.rotate" => handle_identity_rotate(state).await,
        "credits.list" => handle_credits_list(state, req.params).await,
        "credits.stats" => handle_credits_stats(state).await,
        "upnp.status" => handle_upnp_status(state).await,
        "upnp.mapPorts" => handle_upnp_map_ports(state, req.params).await,
        "upnp.unmapPorts" => handle_upnp_unmap_ports(state).await,
        "upnp.setEnabled" => handle_upnp_set_enabled(state, req.params).await,
        "kad.status" => handle_kad_status(state).await,
        "kad.startUdp" => handle_kad_start_udp(state, req.params).await,
        "kad.stopUdp" => handle_kad_stop_udp(state).await,
        "kad.ping" | "kad.pingContact" => handle_kad_ping(state, req.params).await,
        "kad.bootstrap" => handle_kad_bootstrap(state, req.params).await,
        "kad.findNode" => handle_kad_find_node(state, req.params).await,
        "kad.inspectNodesDat" => handle_kad_inspect_nodes_dat(state, req.params).await,
        "kad.importNodesDat" => handle_kad_import_nodes_dat(state, req.params).await,
        "kad.listContacts" => handle_kad_list_contacts(state, req.params).await,
        "kad.clearContacts" => handle_kad_clear_contacts(state).await,
        "kad.searchSources" => handle_kad_search_sources(state, req.params).await,
        "kad.publishSource" => handle_kad_publish_source(state, req.params).await,
        "kad.searchKeywords" => handle_kad_search_keywords(state, req.params).await,
        "kad.publishKeywords" => handle_kad_publish_keywords(state, req.params).await,
        "kad.bootstrapFromUrl" => handle_kad_bootstrap_from_url(state, req.params).await,
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

    // Trigger immediate source query from server and Kad
    let file_info = {
        state.downloads.lock().await.get_download_file_info(&snapshot.id)
    };
    if let Some((_, _, hash, size)) = file_info {
        let hash_hex = hex::encode(hash.0);
        if state.server_session.snapshot().await.status == crate::services::server_session::ServerConnectionStatus::Connected {
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

    // Trigger immediate source query on resume
    let file_info = {
        state.downloads.lock().await.get_download_file_info(&snapshot.id)
    };
    if let Some((_, _, hash, size)) = file_info {
        let hash_hex = hex::encode(hash.0);
        if state.server_session.snapshot().await.status == crate::services::server_session::ServerConnectionStatus::Connected {
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

async fn handle_settings_set(
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
        39735,
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

    if state.network_policy.profile().is_loopback_only() && !peer_addr.ip().is_loopback() {
        return Err(JsonRpcError::invalid_params(
            "Non-loopback peer address is forbidden in LocalLab profile",
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
    let _ = state.server_session.disconnect().await;
    let _ = state.peer_session.disconnect().await;
    state.inbound_listener.stop().await;
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

async fn handle_filter_get_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let stats = state.network_policy.ip_filter().stats();
    Ok(serde_json::to_value(stats).unwrap_or_default())
}

async fn handle_filter_import_file(
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
        .map_err(|e| JsonRpcError::invalid_params(&format!("Failed to parse ipfilter file: {e}")))?;

    Ok(json!({
        "success": true,
        "report": report,
        "stats": state.network_policy.ip_filter().stats()
    }))
}

async fn handle_filter_set_enabled(
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

async fn handle_filter_clear(state: &AppState) -> Result<Value, JsonRpcError> {
    state.network_policy.ip_filter().clear();
    handle_filter_get_status(state).await
}

async fn handle_limits_get(state: &AppState) -> Result<Value, JsonRpcError> {
    let limiters = state.network_policy.rate_limiters();
    let download = limiters.download_bucket.rate_per_sec();
    let upload = limiters.upload_bucket.rate_per_sec();
    let conn_per_sec = limiters.conn_bucket.rate_per_sec();
    let server_req_per_sec = limiters.server_bucket.rate_per_sec();
    let kad_pkt_per_sec = limiters.kad_bucket.rate_per_sec();

    Ok(json!({
        "downloadBytesPerSec": download,
        "uploadBytesPerSec": upload,
        "connectionsPerMinute": conn_per_sec * 60,
        "serverRequestsPerSec": server_req_per_sec,
        "kadPacketsPerSec": kad_pkt_per_sec
    }))
}

async fn handle_limits_set(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let mut cfg = mule_filter::RateLimitsConfig::default();

    if let Some(dl) = params["downloadBytesPerSec"].as_u64() {
        cfg.download_bytes_per_sec = dl;
    }
    if let Some(ul) = params["uploadBytesPerSec"].as_u64() {
        cfg.upload_bytes_per_sec = ul;
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
    handle_limits_get(state).await
}

async fn handle_servers_inspect_met(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let file_path = params["filePath"]
        .as_str()
        .or_else(|| params["path"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'filePath' parameter"))?;

    let path = std::path::Path::new(file_path);
    if !path.exists() {
        return Err(JsonRpcError::invalid_params(&format!(
            "server.met file does not exist: {}",
            file_path
        )));
    }

    let parsed = mule_format::ServerMetFile::open(path)
        .map_err(|e| JsonRpcError::invalid_params(&format!("Failed to parse server.met: {e}")))?;

    let ip_filter = state.network_policy.ip_filter();
    let filter_active = ip_filter.is_enabled();

    let total_count = parsed.servers.len();
    let mut allowed_count = 0;
    let mut blocked_count = 0;
    let mut entries = Vec::new();

    for s in &parsed.servers {
        let is_blocked = filter_active && ip_filter.check_ip(std::net::IpAddr::V4(s.ip)).is_some();
        if is_blocked {
            blocked_count += 1;
        } else {
            allowed_count += 1;
        }

        entries.push(json!({
            "address": format!("{}:{}", s.ip, s.port),
            "ip": s.ip.to_string(),
            "port": s.port,
            "name": s.name,
            "description": s.description,
            "ping": s.ping,
            "preference": s.preference,
            "failedCount": s.failed_count,
            "dynIp": s.dyn_ip,
            "version": s.version,
            "filterStatus": if is_blocked { "blocked_by_filter" } else { "allowed" },
        }));
    }

    Ok(json!({
        "filePath": file_path,
        "totalCount": total_count,
        "allowedCount": allowed_count,
        "blockedCount": blocked_count,
        "servers": entries,
    }))
}

async fn handle_servers_import_met(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let file_path = params["filePath"]
        .as_str()
        .or_else(|| params["path"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'filePath' parameter"))?;

    let path = std::path::Path::new(file_path);
    if !path.exists() {
        return Err(JsonRpcError::invalid_params(&format!(
            "server.met file does not exist: {}",
            file_path
        )));
    }

    let parsed = mule_format::ServerMetFile::open(path)
        .map_err(|e| JsonRpcError::invalid_params(&format!("Failed to parse server.met: {e}")))?;

    let ip_filter = state.network_policy.ip_filter();
    let filter_active = ip_filter.is_enabled();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut allowed_count = 0;
    let mut blocked_count = 0;
    let mut to_upsert = Vec::with_capacity(parsed.servers.len());

    for s in parsed.servers {
        let is_blocked = filter_active && ip_filter.check_ip(std::net::IpAddr::V4(s.ip)).is_some();
        let filter_status = if is_blocked {
            blocked_count += 1;
            mule_core::model::ServerFilterStatus::BlockedByFilter
        } else {
            allowed_count += 1;
            mule_core::model::ServerFilterStatus::Allowed
        };

        to_upsert.push(mule_core::model::Ed2kServer {
            address: format!("{}:{}", s.ip, s.port),
            ip: s.ip.to_string(),
            port: s.port,
            name: s.name,
            description: s.description,
            ping: s.ping,
            preference: s.preference,
            failed_count: s.failed_count,
            dyn_ip: s.dyn_ip,
            version: s.version,
            filter_status,
            last_seen_unix_ms: None,
            created_at_unix_ms: now,
            updated_at_unix_ms: now,
        });
    }

    let total = to_upsert.len();
    let mut dl = state.downloads.lock().await;
    let imported = dl.upsert_servers(&to_upsert)?;

    Ok(json!({
        "success": true,
        "imported": imported,
        "totalCount": total,
        "allowedCount": allowed_count,
        "blockedCount": blocked_count,
    }))
}

async fn handle_servers_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let dl = state.downloads.lock().await;
    let servers = dl.list_servers()?;
    Ok(json!({
        "servers": servers,
        "total": servers.len(),
    }))
}

async fn handle_servers_set_enabled(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let address = params["address"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'address' string parameter"))?;
    let enabled = params["enabled"]
        .as_bool()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'enabled' boolean parameter"))?;

    let mut dl = state.downloads.lock().await;
    let updated = dl.set_server_enabled(address, enabled)?;
    Ok(json!({
        "address": address,
        "enabled": enabled,
        "updated": updated,
    }))
}

async fn handle_servers_delete(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let address = params["address"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'address' string parameter"))?;

    let mut dl = state.downloads.lock().await;
    let deleted = dl.delete_server(address)?;
    Ok(json!({
        "address": address,
        "deleted": deleted,
    }))
}

async fn handle_servers_clear(state: &AppState) -> Result<Value, JsonRpcError> {
    let mut dl = state.downloads.lock().await;
    let count = dl.clear_servers()?;
    Ok(json!({
        "deletedCount": count,
    }))
}

async fn handle_servers_connect(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let address_str = params["address"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'address' parameter (expected 'ip:port')"))?;

    let addr: std::net::SocketAddr = address_str.parse().map_err(|e| {
        JsonRpcError::invalid_params(&format!("Invalid socket address format '{address_str}': {e}"))
    })?;

    // Check if server is in local database and whether it is disabled
    let server_record = {
        let dl = state.downloads.lock().await;
        let servers = dl.list_servers()?;
        servers.into_iter().find(|s| s.address == address_str)
    };

    if let Some(ref record) = server_record {
        if record.filter_status == mule_core::model::ServerFilterStatus::Disabled {
            return Err(JsonRpcError::business(
                "Cannot connect to server: server is disabled in local server list",
            ));
        }
        if record.filter_status == mule_core::model::ServerFilterStatus::BlockedByFilter {
            return Err(JsonRpcError::business(
                "Cannot connect to server: server is blocked by IP filter",
            ));
        }
    }

    let server_name = server_record.map(|s| s.name);

    // Request TCP connect permit from NetworkPolicyManager
    let permit = state
        .network_policy
        .request_tcp_connect(addr, mule_network_policy::ConnectionPurpose::Ed2kServer)
        .map_err(|e| JsonRpcError::business(format!("Connection rejected by network policy: {e}")))?;

    let user_hash = state.identity.lock().await.user_hash();
    let ed2k_port = state.config.ed2k_port;

    // Start session with WAN-adapted timeouts (10s connect, 15s handshake)
    state
        .server_session
        .connect(
            addr,
            server_name,
            permit,
            10000,
            15000,
            ed2k_port,
            "http://emule-project.net".to_string(),
            user_hash,
        )
        .await?;

    let snapshot = state.server_session.snapshot().await;
    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

async fn handle_servers_disconnect(state: &AppState) -> Result<Value, JsonRpcError> {
    state.server_session.disconnect().await?;
    let snapshot = state.server_session.snapshot().await;
    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

async fn handle_servers_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let snapshot = state.server_session.snapshot().await;
    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

async fn handle_servers_search(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let term = params["term"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'term' parameter for search"))?;

    let mut query = mule_protocol::ed2k::SearchQuery::new(term);
    query.min_size = params["minSize"].as_u64();
    query.max_size = params["maxSize"].as_u64();
    query.extension = params["extension"].as_str().map(|s| s.to_string());

    tracing::info!(
        "RPC servers.search received: term='{}', min_size={:?}, max_size={:?}, extension={:?}",
        term,
        query.min_size,
        query.max_size,
        query.extension
    );

    state.server_session.search(query).await?;
    Ok(json!({ "sent": true }))
}

async fn handle_servers_query_sources(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let hash_str = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;

    let file_size = params["fileSize"]
        .as_u64()
        .or_else(|| params["size"].as_u64())
        .unwrap_or(0);

    let raw_bytes = hex::decode(hash_str)
        .map_err(|e| JsonRpcError::invalid_params(&format!("Invalid file hash hex: {e}")))?;
    if raw_bytes.len() != 16 {
        return Err(JsonRpcError::invalid_params("File hash must be exactly 16 bytes (32 hex characters)"));
    }
    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&raw_bytes);

    let req = mule_protocol::ed2k::GetSourcesRequest::with_obfuscation(
        file_hash,
        file_size,
        state.config.obfuscation_enabled,
    );
    state.server_session.get_sources(req).await?;
    Ok(json!({ "sent": true }))
}

// ── Peer Sources Handlers ───────────────────────────────────────────────────

async fn handle_sources_list(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let hash_str = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;

    let sources = state.downloads.lock().await.list_peer_sources(hash_str)?;
    serde_json::to_value(sources)
        .map_err(|e| JsonRpcError::business(format!("Serialization error: {e}")))
}

async fn handle_sources_add_manual(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let hash_str = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;
    let address = params["address"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'address' parameter (expected 'ip:port')"))?;

    let addr: std::net::SocketAddr = address.parse().map_err(|e| {
        JsonRpcError::invalid_params(&format!("Invalid socket address format '{address}': {e}"))
    })?;

    // Check against IP filter
    let filter = state.network_policy.ip_filter();
    let is_filtered = filter.check_ip(addr.ip()).is_some();

    let profile = state.network_policy.profile();
    let status = if is_filtered {
        mule_core::model::PeerSourceStatus::Filtered
    } else if profile == mule_network_policy::NetworkProfile::LocalLab && !addr.ip().is_loopback() {
        mule_core::model::PeerSourceStatus::Rejected
    } else {
        mule_core::model::PeerSourceStatus::Allowed
    };

    let ip_u32 = match addr {
        std::net::SocketAddr::V4(v4) => u32::from_be_bytes(v4.ip().octets()),
        _ => 0,
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let user_hash_param = params["userHash"]
        .as_str()
        .or_else(|| params["user_hash"].as_str())
        .map(|s| s.to_string());
    let crypt_options_param = params["cryptOptions"]
        .as_u64()
        .or_else(|| params["crypt_options"].as_u64())
        .unwrap_or(0) as u8;

    let source = mule_core::model::PeerSource {
        file_hash: hash_str.to_string(),
        address: address.to_string(),
        ip: ip_u32,
        port: addr.port(),
        origin: mule_core::model::PeerSourceOrigin::Manual,
        discovered_at_unix_ms: now,
        last_seen_unix_ms: now,
        status,
        fail_count: 0,
        last_error: if is_filtered {
            Some("Blocked by local IP filter".to_string())
        } else {
            None
        },
        cooldown_until_unix_ms: None,
        user_hash: user_hash_param,
        crypt_options: crypt_options_param,
    };

    state.downloads.lock().await.upsert_peer_sources(hash_str, std::slice::from_ref(&source))?;
    serde_json::to_value(source)
        .map_err(|e| JsonRpcError::business(format!("Serialization error: {e}")))
}

async fn handle_sources_connect(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let hash_str = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;
    let address = params["address"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'address' parameter (expected 'ip:port')"))?;

    let addr: std::net::SocketAddr = address.parse().map_err(|e| {
        JsonRpcError::invalid_params(&format!("Invalid socket address format '{address}': {e}"))
    })?;

    let user_hash_param = params["userHash"]
        .as_str()
        .or_else(|| params["user_hash"].as_str())
        .map(|s| s.to_string());
    let crypt_options_param = params["cryptOptions"]
        .as_u64()
        .or_else(|| params["crypt_options"].as_u64())
        .map(|v| v as u8);

    // Check if network is disabled (Kill Switch)
    if state.network_policy.state() == mule_network_policy::NetworkState::Disabled {
        return Err(JsonRpcError::business("Network is disabled by Kill Switch"));
    }

    // Check IP Filter
    let filter = state.network_policy.ip_filter();
    if filter.check_ip(addr.ip()).is_some() {
        let ip_u32 = match addr {
            std::net::SocketAddr::V4(v4) => u32::from_be_bytes(v4.ip().octets()),
            _ => 0,
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let filtered_src = mule_core::model::PeerSource {
            file_hash: hash_str.to_string(),
            address: address.to_string(),
            ip: ip_u32,
            port: addr.port(),
            origin: mule_core::model::PeerSourceOrigin::Manual,
            discovered_at_unix_ms: now,
            last_seen_unix_ms: now,
            status: mule_core::model::PeerSourceStatus::Filtered,
            fail_count: 1,
            last_error: Some("Blocked by local IP filter".to_string()),
            cooldown_until_unix_ms: None,
            user_hash: user_hash_param.clone(),
            crypt_options: crypt_options_param.unwrap_or(0),
        };
        let mut dl_svc = state.downloads.lock().await;
        let _ = dl_svc.upsert_peer_sources(hash_str, &[filtered_src]);
        return Err(JsonRpcError::business(format!(
            "Connection refused: peer IP {} is blocked by IP filter",
            addr.ip()
        )));
    }

    // Check Cooldown unless force is true
    let force = params["force"].as_bool().unwrap_or(false);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    {
        let mut dl_svc = state.downloads.lock().await;
        let sources = dl_svc.list_peer_sources(hash_str)?;
        if let Some(src) = sources.into_iter().find(|s| s.address == address) {
            if let Some(cooldown_until) = src.cooldown_until_unix_ms {
                if cooldown_until > now && !force {
                    let remaining_secs = (cooldown_until - now + 999) / 1000;
                    return Err(JsonRpcError::business(format!(
                        "Source is in cooldown after {} failed attempt(s). Retry in {}s or force reconnect.",
                        src.fail_count, remaining_secs
                    )));
                }
            }
        }
        if force {
            let _ = dl_svc.reset_peer_source_cooldown(hash_str, address);
        }

        // Ensure source exists in persistence
        let ip_u32 = match addr {
            std::net::SocketAddr::V4(v4) => u32::from_be_bytes(v4.ip().octets()),
            _ => 0,
        };
        let init_src = mule_core::model::PeerSource {
            file_hash: hash_str.to_string(),
            address: address.to_string(),
            ip: ip_u32,
            port: addr.port(),
            origin: mule_core::model::PeerSourceOrigin::Manual,
            discovered_at_unix_ms: now,
            last_seen_unix_ms: now,
            status: mule_core::model::PeerSourceStatus::Candidate,
            fail_count: 0,
            last_error: None,
            cooldown_until_unix_ms: None,
            user_hash: user_hash_param,
            crypt_options: crypt_options_param.unwrap_or(0),
        };
        let _ = dl_svc.upsert_peer_sources(hash_str, &[init_src]);
    }

    // Look up target peer source capabilities from database
    let (target_user_hash, peer_crypt_options) = {
        let dl_svc = state.downloads.lock().await;
        if let Ok(sources) = dl_svc.list_peer_sources(hash_str) {
            if let Some(src) = sources.into_iter().find(|s| s.address == address) {
                (src.user_hash_bytes(), src.crypt_options)
            } else {
                (None, 0)
            }
        } else {
            (None, 0)
        }
    };

    let peer_requires_crypt = (peer_crypt_options & 0x04) != 0;
    if (state.config.obfuscation_required || peer_requires_crypt) && target_user_hash.is_none() {
        return Err(JsonRpcError::business(
            "Peer requires protocol obfuscation but its user hash is unknown",
        ));
    }

    // Request TCP connect permit from NetworkPolicyManager
    let permit = state
        .network_policy
        .request_tcp_connect(addr, mule_network_policy::ConnectionPurpose::Ed2kPeer)
        .map_err(|e| JsonRpcError::business(format!("Connection rejected by network policy: {e}")))?;

    let client_port = state.config.ed2k_port;
    let user_hash = state.identity.lock().await.user_hash();

    let (client_id, server_ip, server_port) = {
        let s_snap = state.server_session.snapshot().await;
        if s_snap.status == crate::services::server_session::ServerConnectionStatus::Connected {
            let cid = s_snap.client_id.unwrap_or(0);
            let (s_ip, s_port) = if let Some(addr_str) = s_snap.server_address.as_deref() {
                if let Ok(sa) = addr_str.parse::<std::net::SocketAddr>() {
                    let ip_u32 = match sa.ip() {
                        std::net::IpAddr::V4(v4) => u32::from_le_bytes(v4.octets()),
                        _ => 0,
                    };
                    (ip_u32, sa.port())
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            };
            (cid, s_ip, s_port)
        } else {
            (0, 0, 0)
        }
    };

    // Start single controlled peer session
    state
        .peer_session
        .connect(
            hash_str,
            addr,
            permit,
            client_port,
            "Emule-Sion".to_string(),
            user_hash,
            std::sync::Arc::clone(&state.downloads),
            client_id,
            server_ip,
            server_port,
            state.config.kad_port,
            target_user_hash,
            peer_crypt_options,
            state.config.obfuscation_enabled,
            state.config.obfuscation_required,
            state.config.obfuscation_padding_max,
        )
        .await?;

    let snapshot = state.peer_session.snapshot().await;
    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

async fn handle_sources_disconnect(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let address = params.as_ref().and_then(|p| p["address"].as_str());
    state.peer_session.disconnect_peer(address).await?;
    let snapshot = state.peer_session.snapshot().await;
    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

async fn handle_sources_clear(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let hash_str = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;

    let count = state.downloads.lock().await.clear_peer_sources(hash_str)?;
    Ok(json!({ "cleared": count }))
}

async fn handle_sources_status(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let file_hash = params.as_ref().and_then(|p| p["fileHash"].as_str().or_else(|| p["hash"].as_str()));
    let swarm_snap = state.peer_session.swarm_snapshot(file_hash).await;
    
    // If specific file or swarm is requested, return full swarm object
    if params.as_ref().map(|p| p.get("fileHash").is_some() || p.get("swarm").is_some()).unwrap_or(false) {
        Ok(serde_json::to_value(swarm_snap).unwrap_or_default())
    } else {
        // Return first active peer snapshot or disconnected for legacy single peer callers, but augmented with swarm fields
        let mut val = serde_json::to_value(state.peer_session.snapshot().await).unwrap_or_default();
        if let Value::Object(ref mut map) = val {
            map.insert("activePeerCount".to_string(), json!(swarm_snap.active_peer_count));
            map.insert("peers".to_string(), json!(swarm_snap.peers));
            map.insert("inFlightBlocks".to_string(), json!(swarm_snap.in_flight_blocks));
        }
        Ok(val)
    }
}

async fn handle_shares_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let dl_svc = state.downloads.lock().await;
    let list = dl_svc.list_shared_files()?;
    Ok(json!({ "sharedFiles": list }))
}

async fn handle_shares_add(
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

async fn handle_shares_remove(
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

async fn handle_shares_set_enabled(
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

async fn handle_listener_start(
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

async fn handle_listener_stop(state: &AppState) -> Result<Value, JsonRpcError> {
    state.inbound_listener.stop().await;
    Ok(json!({ "stopped": true }))
}

async fn handle_listener_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let snap = state.inbound_listener.snapshot().await;
    Ok(serde_json::to_value(snap).unwrap_or_default())
}

async fn handle_uploads_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let active = state.inbound_listener.upload_manager().active_snapshot().await;
    let queue = state.inbound_listener.upload_manager().list_queue().await;
    let active_list = match active {
        Some(a) => vec![a],
        None => vec![],
    };
    Ok(json!({
        "active": active_list,
        "queued": queue,
    }))
}

async fn handle_uploads_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let active = state.inbound_listener.upload_manager().active_snapshot().await;
    Ok(serde_json::to_value(active).unwrap_or(Value::Null))
}

async fn handle_uploads_cancel(
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

async fn handle_identity_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let id_guard = state.identity.lock().await;
    let status = id_guard.status();
    Ok(serde_json::to_value(status).unwrap_or_default())
}

async fn handle_identity_rotate(state: &AppState) -> Result<Value, JsonRpcError> {
    let mut id_guard = state.identity.lock().await;
    let new_id = crate::services::IdentityService::rotate_key(&state.config.data_dir)
        .map_err(|e| JsonRpcError::business(format!("Failed to rotate identity key: {e}")))?;
    *id_guard = new_id;
    let status = id_guard.status();
    Ok(serde_json::to_value(status).unwrap_or_default())
}

async fn handle_credits_list(
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

async fn handle_credits_stats(state: &AppState) -> Result<Value, JsonRpcError> {
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

async fn handle_kad_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let status = state.kad.get_status().await;
    Ok(serde_json::to_value(status).unwrap_or_default())
}

async fn handle_kad_inspect_nodes_dat(
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
        return Err(JsonRpcError::invalid_params("Missing 'path' or 'dataBase64' parameter"));
    };

    let report = state
        .kad
        .inspect_nodes_dat(&bytes)
        .map_err(JsonRpcError::business)?;

    Ok(serde_json::to_value(report).unwrap_or_default())
}

async fn handle_kad_import_nodes_dat(
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
        return Err(JsonRpcError::invalid_params("Missing 'path' or 'dataBase64' parameter"));
    };

    let imported_count = state
        .kad
        .import_nodes_dat(&bytes)
        .await
        .map_err(JsonRpcError::business)?;

    let status = state.kad.get_status().await;

    Ok(json!({
        "importedCount": imported_count,
        "status": status,
    }))
}

async fn handle_kad_list_contacts(
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

async fn handle_kad_clear_contacts(state: &AppState) -> Result<Value, JsonRpcError> {
    let cleared = state
        .kad
        .clear_contacts()
        .await
        .map_err(JsonRpcError::business)?;

    Ok(json!({
        "cleared": cleared,
    }))
}

async fn handle_kad_start_udp(
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

    Ok(json!({
        "boundAddress": local_addr.to_string(),
        "status": status,
    }))
}

async fn handle_kad_stop_udp(state: &AppState) -> Result<Value, JsonRpcError> {
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

async fn handle_kad_ping(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let ip = params["ip"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'ip' parameter"))?;
    let port = params["port"]
        .as_u64()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'port' parameter"))? as u16;
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

async fn handle_kad_bootstrap(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let ip = params["ip"]
        .as_str()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'ip' parameter"))?;
    let port = params["port"]
        .as_u64()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'port' parameter"))? as u16;

    let received_count = state
        .kad
        .bootstrap_from_node(ip, port)
        .await
        .map_err(JsonRpcError::business)?;

    let status = state.kad.get_status().await;

    Ok(json!({
        "success": true,
        "receivedContacts": received_count,
        "status": status,
    }))
}

async fn handle_kad_find_node(
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

async fn handle_kad_search_sources(
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

async fn handle_kad_publish_source(
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

async fn handle_kad_search_keywords(
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

async fn handle_kad_publish_keywords(
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

async fn handle_upnp_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let snap = state.upnp.snapshot().await;
    Ok(serde_json::to_value(snap).unwrap_or_default())
}

async fn handle_upnp_map_ports(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let snap = state.upnp.snapshot().await;
    let tcp_port = params
        .as_ref()
        .and_then(|p| p["tcpPort"].as_u64())
        .map(|p| p as u16)
        .unwrap_or(snap.tcp_port);
    let udp_port = params
        .as_ref()
        .and_then(|p| p["udpPort"].as_u64())
        .map(|p| p as u16)
        .unwrap_or(snap.udp_port);

    if tcp_port < 1024 || udp_port < 1024 {
        return Err(JsonRpcError::invalid_params("Privileged ports (< 1024) are not allowed for UPnP mapping"));
    }

    let _ = state.upnp.map_ports(tcp_port, udp_port).await;
    let updated_snap = state.upnp.snapshot().await;

    let notif = JsonRpcNotification::new(
        "upnp.statusChanged",
        serde_json::to_value(&updated_snap).unwrap_or_default(),
    );
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = state.event_tx.send(json);
    }

    Ok(serde_json::to_value(updated_snap).unwrap_or_default())
}

async fn handle_upnp_unmap_ports(state: &AppState) -> Result<Value, JsonRpcError> {
    let _ = state.upnp.unmap_ports().await;
    let snap = state.upnp.snapshot().await;

    let notif = JsonRpcNotification::new(
        "upnp.statusChanged",
        serde_json::to_value(&snap).unwrap_or_default(),
    );
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = state.event_tx.send(json);
    }

    Ok(serde_json::to_value(snap).unwrap_or_default())
}

async fn handle_upnp_set_enabled(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let enabled = params["enabled"]
        .as_bool()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'enabled' boolean parameter"))?;

    state.upnp.set_enabled(enabled).await;
    if !enabled {
        let _ = state.upnp.unmap_ports().await;
    }
    let snap = state.upnp.snapshot().await;

    let notif = JsonRpcNotification::new(
        "upnp.statusChanged",
        serde_json::to_value(&snap).unwrap_or_default(),
    );
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = state.event_tx.send(json);
    }

    Ok(serde_json::to_value(snap).unwrap_or_default())
}

async fn handle_servers_update_from_url(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let url = params["url"]
        .as_str()
        .unwrap_or("http://upd.emule-security.org/server.met");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| JsonRpcError::business(format!("HTTP client build error: {e}")))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| JsonRpcError::business(format!("Failed to fetch server.met from {url}: {e}")))?;

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

    let mut cursor = std::io::Cursor::new(&bytes);
    let parsed = mule_format::ServerMetFile::read_from(&mut cursor)
        .map_err(|e| JsonRpcError::business(format!("Failed to parse server.met: {e}")))?;

    let ip_filter = state.network_policy.ip_filter();
    let filter_active = ip_filter.is_enabled();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut allowed_count = 0;
    let mut blocked_count = 0;
    let mut to_upsert = Vec::with_capacity(parsed.servers.len());

    for s in parsed.servers {
        let is_blocked = filter_active && ip_filter.check_ip(std::net::IpAddr::V4(s.ip)).is_some();
        let filter_status = if is_blocked {
            blocked_count += 1;
            mule_core::model::ServerFilterStatus::BlockedByFilter
        } else {
            allowed_count += 1;
            mule_core::model::ServerFilterStatus::Allowed
        };

        to_upsert.push(mule_core::model::Ed2kServer {
            address: format!("{}:{}", s.ip, s.port),
            ip: s.ip.to_string(),
            port: s.port,
            name: s.name,
            description: s.description,
            ping: s.ping,
            preference: s.preference,
            failed_count: s.failed_count,
            dyn_ip: s.dyn_ip,
            version: s.version,
            filter_status,
            last_seen_unix_ms: None,
            created_at_unix_ms: now,
            updated_at_unix_ms: now,
        });
    }

    let total = to_upsert.len();
    let mut dl = state.downloads.lock().await;
    let imported = dl
        .upsert_servers(&to_upsert)
        .map_err(|e| JsonRpcError::business(format!("Database error saving servers: {e}")))?;

    Ok(json!({
        "url": url,
        "totalCount": total,
        "imported": imported,
        "allowedCount": allowed_count,
        "blockedCount": blocked_count,
    }))
}

async fn handle_kad_bootstrap_from_url(
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

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| JsonRpcError::business(format!("Failed to fetch nodes.dat from {url}: {e}")))?;

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

    Ok(json!({
        "url": url,
        "importedCount": imported_count,
        "status": status,
    }))
}




