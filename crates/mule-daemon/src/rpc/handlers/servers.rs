use serde_json::{json, Value};

use crate::error::JsonRpcError;
use crate::state::AppState;

pub async fn handle_servers_inspect_met(
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

pub async fn handle_servers_import_met(
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

pub async fn handle_servers_list(state: &AppState) -> Result<Value, JsonRpcError> {
    let dl = state.downloads.lock().await;
    let servers = dl.list_servers()?;
    Ok(json!({
        "servers": servers,
        "total": servers.len(),
    }))
}

pub async fn handle_servers_set_enabled(
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

pub async fn handle_servers_delete(
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

pub async fn handle_servers_clear(state: &AppState) -> Result<Value, JsonRpcError> {
    let mut dl = state.downloads.lock().await;
    let count = dl.clear_servers()?;
    Ok(json!({
        "deletedCount": count,
    }))
}

pub async fn handle_servers_connect(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let address_str = params["address"].as_str().ok_or_else(|| {
        JsonRpcError::invalid_params("Missing 'address' parameter (expected 'ip:port')")
    })?;

    let addr: std::net::SocketAddr = address_str.parse().map_err(|e| {
        JsonRpcError::invalid_params(&format!(
            "Invalid socket address format '{address_str}': {e}"
        ))
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
        .map_err(|e| {
            JsonRpcError::business(format!("Connection rejected by network policy: {e}"))
        })?;

    let user_hash = state.identity.lock().await.user_hash();
    let ed2k_port = state.ed2k_port();

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
            state.current_nickname().await,
            user_hash,
        )
        .await?;

    let snapshot = state.server_session.snapshot().await;
    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

pub async fn handle_servers_disconnect(state: &AppState) -> Result<Value, JsonRpcError> {
    state.server_session.disconnect().await?;
    let snapshot = state.server_session.snapshot().await;
    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

pub async fn handle_servers_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let snapshot = state.server_session.snapshot().await;
    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

pub async fn handle_servers_search(
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

    let payload = {
        let limits = mule_protocol::ed2k::Ed2kLimits::default();
        query
            .encode(&limits)
            .map(|p| p.payload.to_vec())
            .unwrap_or_default()
    };

    // `udpOnly` (eMule "Global search" scope): skip the TCP search on the
    // connected server and only fan out over UDP to the server list.
    let udp_only = params["udpOnly"].as_bool().unwrap_or(false);
    if !udp_only {
        state.server_session.search(query).await?;
    }

    // Global UDP search (OP_GLOBSEARCHREQ2/3): eMule also queries servers over
    // UDP — the connected server plus other enabled known servers, on their
    // UDP port (TCP+4). Fire-and-forget; answers arrive as OP_GLOBSEARCHRES.
    if !payload.is_empty() {
        let mut targets: Vec<std::net::SocketAddr> = Vec::new();
        if let Some(addr_str) = state.server_session.snapshot().await.server_address {
            if let Ok(mut a) = addr_str.parse::<std::net::SocketAddr>() {
                a.set_port(a.port().saturating_add(4));
                targets.push(a);
            }
        }
        let extra: Vec<std::net::SocketAddr> = {
            let dl_guard = state.downloads.lock().await;
            dl_guard
                .list_servers()
                .unwrap_or_default()
                .into_iter()
                .filter(|s| s.filter_status == mule_core::model::ServerFilterStatus::Allowed)
                .filter_map(|s| s.address.parse::<std::net::SocketAddr>().ok())
                .map(|mut a| {
                    a.set_port(a.port().saturating_add(4));
                    a
                })
                .filter(|a| !targets.contains(a))
                .take(9)
                .collect()
        };
        targets.extend(extra);
        let mut udp_targets = Vec::new();
        for (i, target) in targets.iter().enumerate() {
            // eMule paces global UDP sends at ~50 ms per server
            // (SearchResultsWnd TimerServerTimeout) — blasting all servers
            // at once gets dropped by rate-limiting servers.
            if i > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            let form = state.server_udp.send_glob_search(*target, &payload).await;
            udp_targets.push(json!({
                "address": target.to_string(),
                "request": form,
            }));
        }
        return Ok(json!({ "sent": true, "udpTargets": udp_targets }));
    }

    Ok(json!({ "sent": true }))
}

pub async fn handle_servers_query_sources(
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
        return Err(JsonRpcError::invalid_params(
            "File hash must be exactly 16 bytes (32 hex characters)",
        ));
    }
    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&raw_bytes);

    let req = mule_protocol::ed2k::GetSourcesRequest::with_obfuscation(
        file_hash,
        file_size,
        state.effective_obfuscation().await.0,
    );
    state.server_session.get_sources(req).await?;
    Ok(json!({ "sent": true }))
}

pub async fn handle_servers_update_from_url(
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

    let response = client.get(url).send().await.map_err(|e| {
        JsonRpcError::business(format!("Failed to fetch server.met from {url}: {e}"))
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
