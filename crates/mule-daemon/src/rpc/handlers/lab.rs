// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::JsonRpcError;
use crate::state::AppState;

#[cfg(any(feature = "debug-rpc", feature = "lab-tools"))]
pub async fn handle_inject_range(
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
        .inject_range(&id, offset, &data, None)?;
    state.broadcast_download_update(snapshot.clone());

    Ok(crate::rpc::handlers::json_value(&snapshot))
}

pub async fn handle_get_sources(
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

#[cfg(feature = "lab-loopback")]
pub async fn handle_lab_add_peer_source(
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
pub async fn handle_ed2k_probe_source(
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

    let snapshot = state.downloads.lock().await.record_source_availability(
        &id,
        peer_addr,
        status.availability_mask.clone(),
    )?;

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

#[cfg(feature = "ed2k-loopback")]
pub async fn handle_ed2k_start_download(
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
pub async fn handle_lab_status(state: &AppState) -> Result<Value, JsonRpcError> {
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
pub async fn handle_lab_create_fixture(
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
        "text" => (
            "hello-loopback.txt".to_string(),
            b"eMule-Rust local test\n".to_vec(),
        ),
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

    let link =
        mule_format::ed2k::Ed2kFileLink::create(&file_name, data.len() as u64, hash_res.root_hash)
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
pub async fn handle_lab_start_loopback_peer(
    state: &AppState,
    _params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let test_peer =
        mule_protocol::ed2k::Ed2kTestPeer::bind(mule_protocol::ed2k::TestPeerMode::Nominal)
            .await
            .map_err(|e| {
                JsonRpcError::internal(format!("Failed to bind loopback test peer: {e}"))
            })?;
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
                                let part_count = file_data.len().div_ceil(9_728_000).max(1) as u16;
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
pub async fn handle_lab_stop_loopback_peer(state: &AppState) -> Result<Value, JsonRpcError> {
    let mut active_peer = state.active_lab_peer.lock().await;
    if let Some((_, handle)) = active_peer.take() {
        handle.abort();
    }
    Ok(json!({ "stopped": true }))
}

#[cfg(feature = "lab-tools")]
pub async fn handle_lab_cleanup_test_data(state: &AppState) -> Result<Value, JsonRpcError> {
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
