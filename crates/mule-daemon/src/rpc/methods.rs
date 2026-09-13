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
        "downloads.list" => handle_downloads_list(state).await,
        "downloads.get" => handle_downloads_get(state, req.params).await,
        "downloads.addEd2kLink" => handle_add_ed2k_link(state, req.params).await,
        "downloads.pause" => handle_pause(state, req.params).await,
        "downloads.resume" => handle_resume(state, req.params).await,
        "downloads.remove" => handle_remove(state, req.params).await,
        "events.subscribe" => Ok(json!({ "subscribed": true })),
        "debug.injectRange" => handle_inject_range(state, req.params).await,
        "downloads.getSources" | "ed2k.getSources" => {
            handle_get_sources(state, req.params).await
        }
        #[cfg(feature = "lab-loopback")]
        "lab.addPeerSource" | "downloads.addSource" => {
            handle_lab_add_peer_source(state, req.params).await
        }
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

    Ok(json!({
        "daemonVersion": DAEMON_VERSION,
        "protocolVersion": PROTOCOL_VERSION,
        "instanceId": state.instance_id.to_string(),
        "sessionId": state.session_id.to_string(),
        "capabilities": [
            "downloads",
            "download-events",
            "local-simulation"
        ]
    }))
}

async fn handle_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let dl_count = state.downloads.lock().await.list_downloads().len();
    Ok(json!({
        "version": DAEMON_VERSION,
        "status": "healthy",
        "downloadsCount": dl_count,
        "instanceId": state.instance_id.to_string()
    }))
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
        .get_download(&id)
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
    let target_dir_str = params["targetDirectory"].as_str().unwrap_or(".");
    let target_dir = std::path::Path::new(target_dir_str);

    let policy = match params["allocationPolicy"].as_str() {
        Some("preallocate") => AllocationPolicy::Preallocate,
        Some("none") => AllocationPolicy::None,
        _ => AllocationPolicy::SparseIfSupported,
    };

    let snapshot = state
        .downloads
        .lock()
        .await
        .add_ed2k_link(link, target_dir, policy)?;

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

