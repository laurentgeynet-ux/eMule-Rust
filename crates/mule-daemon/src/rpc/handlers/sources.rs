// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use serde_json::{json, Value};

use crate::error::JsonRpcError;
use crate::state::AppState;

pub async fn handle_sources_list(
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

pub async fn handle_sources_add_manual(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let hash_str = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;
    let address = params["address"].as_str().ok_or_else(|| {
        JsonRpcError::invalid_params("Missing 'address' parameter (expected 'ip:port')")
    })?;

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
        buddy_id: None,
software: None,
        availability: None,
    };

    state
        .downloads
        .lock()
        .await
        .upsert_peer_sources(hash_str, std::slice::from_ref(&source))?;
    serde_json::to_value(source)
        .map_err(|e| JsonRpcError::business(format!("Serialization error: {e}")))
}

pub async fn handle_sources_connect(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let hash_str = params["fileHash"]
        .as_str()
        .or_else(|| params["hash"].as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'fileHash' parameter"))?;
    let address = params["address"].as_str().ok_or_else(|| {
        JsonRpcError::invalid_params("Missing 'address' parameter (expected 'ip:port')")
    })?;

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
software: None,
            availability: None,
            crypt_options: crypt_options_param.unwrap_or(0),
            buddy_id: None,
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
software: None,
            availability: None,
            cooldown_until_unix_ms: None,
            user_hash: user_hash_param,
            crypt_options: crypt_options_param.unwrap_or(0),
            buddy_id: None,
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
    let (obf_enabled, obf_required) = state.effective_obfuscation().await;
    if (obf_required || peer_requires_crypt) && target_user_hash.is_none() {
        return Err(JsonRpcError::business(
            "Peer requires protocol obfuscation but its user hash is unknown",
        ));
    }

    // Request TCP connect permit from NetworkPolicyManager
    let permit = state
        .network_policy
        .request_tcp_connect(addr, mule_network_policy::ConnectionPurpose::Ed2kPeer)
        .map_err(|e| {
            JsonRpcError::business(format!("Connection rejected by network policy: {e}"))
        })?;

    let client_port = state.ed2k_port();
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
            state.current_nickname().await,
            user_hash,
            std::sync::Arc::clone(&state.downloads),
            client_id,
            server_ip,
            server_port,
            state.kad_port(),
            target_user_hash,
            peer_crypt_options,
            obf_enabled,
            obf_required,
            state.config.obfuscation_padding_max,
            state.sx_env.clone(),
        )
        .await?;

    let snapshot = state.peer_session.snapshot().await;
    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

pub async fn handle_sources_disconnect(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let address = params.as_ref().and_then(|p| p["address"].as_str());
    state.peer_session.disconnect_peer(address).await?;
    let snapshot = state.peer_session.snapshot().await;
    Ok(serde_json::to_value(snapshot).unwrap_or_default())
}

pub async fn handle_sources_clear(
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

pub async fn handle_sources_status(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let file_hash = params
        .as_ref()
        .and_then(|p| p["fileHash"].as_str().or_else(|| p["hash"].as_str()));
    let swarm_snap = state.peer_session.swarm_snapshot(file_hash).await;

    // If specific file or swarm is requested, return full swarm object
    if params
        .as_ref()
        .map(|p| p.get("fileHash").is_some() || p.get("swarm").is_some())
        .unwrap_or(false)
    {
        Ok(serde_json::to_value(swarm_snap).unwrap_or_default())
    } else {
        // Return first active peer snapshot or disconnected for legacy single peer callers, but augmented with swarm fields
        let mut val = serde_json::to_value(state.peer_session.snapshot().await).unwrap_or_default();
        if let Value::Object(ref mut map) = val {
            map.insert(
                "activePeerCount".to_string(),
                json!(swarm_snap.active_peer_count),
            );
            map.insert("peers".to_string(), json!(swarm_snap.peers));
            map.insert(
                "inFlightBlocks".to_string(),
                json!(swarm_snap.in_flight_blocks),
            );
        }
        Ok(val)
    }
}
