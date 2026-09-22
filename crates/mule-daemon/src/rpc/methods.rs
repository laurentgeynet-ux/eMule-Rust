use serde_json::{json, Value};

use crate::error::JsonRpcError;
use crate::rpc::handlers::{
    categories::*, downloads::*, friends::*, history::*, kad::*, lab::*, network::*, servers::*,
    settings::*, shares::*, sources::*, upnp::*,
};
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
        "downloads.rename" => handle_rename(state, req.params).await,
        "downloads.setPriority" => handle_set_priority(state, req.params).await,
        "downloads.setMaxSources" => handle_set_max_sources(state, req.params).await,
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
        "stats.resetLifetimeTotals" => handle_stats_reset_lifetime_totals(state).await,
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
        "sources.status" | "sources.swarmStatus" | "swarm.status" => {
            handle_sources_status(state, req.params).await
        }
        "shares.list" => handle_shares_list(state).await,
        "shares.add" => handle_shares_add(state, req.params).await,
        "shares.remove" => handle_shares_remove(state, req.params).await,
        "shares.rename" => handle_shares_rename(state, req.params).await,
        "shares.rescan" => handle_shares_rescan(state).await,
        "shares.setEnabled" => handle_shares_set_enabled(state, req.params).await,
        "shares.setUploadPriority" => {
            handle_shares_set_upload_priority(state, req.params).await
        }
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
        "friends.list" => handle_friends_list(state).await,
        "friends.add" => handle_friends_add(state, req.params).await,
        "friends.remove" => handle_friends_remove(state, req.params).await,
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
        "kad.searchNotes" => handle_kad_search_notes(state, req.params).await,
        "kad.publishNote" => handle_kad_publish_note(state, req.params).await,
        "kad.bootstrapFromUrl" => handle_kad_bootstrap_from_url(state, req.params).await,
        // "events.subscribe" is intercepted in rpc::server::handle_socket —
        // it mutates the per-session topic filter, not daemon state.
        #[cfg(any(feature = "debug-rpc", feature = "lab-tools"))]
        "debug.injectRange" => handle_inject_range(state, req.params).await,
        "downloads.getSources" | "ed2k.getSources" => handle_get_sources(state, req.params).await,
        #[cfg(feature = "lab-loopback")]
        "lab.addPeerSource" | "downloads.addSource" => {
            handle_lab_add_peer_source(state, req.params).await
        }
        #[cfg(feature = "lab-loopback")]
        "lab.status" => handle_lab_status(state).await,
        #[cfg(feature = "lab-loopback")]
        "lab.createFixture" => handle_lab_create_fixture(state, req.params).await,
        #[cfg(feature = "lab-loopback")]
        "lab.startLoopbackPeer" => handle_lab_start_loopback_peer(state, req.params).await,
        #[cfg(feature = "lab-loopback")]
        "lab.stopLoopbackPeer" => handle_lab_stop_loopback_peer(state).await,
        #[cfg(feature = "lab-loopback")]
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
