//! Loopback-confined eD2K block transfer download coordinator.
//! Phase 6.4: Zero public network, strictly TCP 127.0.0.1.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::Duration;

use mule_core::DownloadStatus;
use mule_protocol::ed2k::{
    Ed2kLoopbackClient, Ed2kSessionError, Ed2kTag, TagName, TagValue, TAG_SPECIAL_NAME,
    TAG_SPECIAL_VERSION,
};
use tracing::{error, info};
use uuid::Uuid;

use crate::state::AppState;

/// Executes an active eD2K block download over loopback.
pub async fn run_ed2k_download(
    download_id: Uuid,
    peer_addr: SocketAddr,
    state: AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 0. Enforce network policy confinement
    if state.network_policy.profile().is_loopback_only() && !peer_addr.ip().is_loopback() {
        return Err(
            format!("Non-loopback address {peer_addr} is forbidden in LocalLab profile").into(),
        );
    }

    // 1. Transition to WaitingForSources
    let (temp_path, _target_path, expected_hash, total_size) = {
        let mut dls = state.downloads.lock().await;
        let snap = dls.update_status(&download_id, DownloadStatus::WaitingForSources)?;
        state.broadcast_download_update(snap);
        match dls.get_download_file_info(&download_id) {
            Some(info) => info,
            None => return Err(format!("Download {download_id} not found").into()),
        }
    };

    // 2. Connect & Handshake with peer on loopback
    let local_hash = [0x11; 16];
    let tags = vec![
        Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_NAME),
            value: TagValue::String("eMuleRustDaemon".to_string()),
        },
        Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_VERSION),
            value: TagValue::UInt32(60),
        },
    ];

    let (mut client, _info) = match Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        local_hash,
        0x7F000001,
        39735,
        tags,
        0x7F000001,
        4661,
        Duration::from_millis(1500),
    )
    .await
    {
        Ok(res) => res,
        Err(e) => {
            let mut dls = state.downloads.lock().await;
            let snap = dls.update_status(&download_id, DownloadStatus::Failed)?;
            state.broadcast_download_update(snap);
            return Err(Box::new(e));
        }
    };

    // 3. Transition to Downloading
    {
        let mut dls = state.downloads.lock().await;
        let snap = dls.update_status(&download_id, DownloadStatus::Downloading)?;
        state.broadcast_download_update(snap);
    }

    // 4. Request file availability via OP_FILESTATUSREQ
    let status = client
        .request_file_status(expected_hash.0, Duration::from_millis(1500))
        .await?;

    // Record source availability in download service
    {
        let mut dls = state.downloads.lock().await;
        let snap = dls.record_source_availability(
            &download_id,
            peer_addr,
            status.availability_mask.clone(),
        )?;
        state.broadcast_download_update(snap);
    }

    // 5. Calculate missing blocks (gap-only planning)
    let missing_ranges = {
        let dls = state.downloads.lock().await;
        dls.get_missing_ranges(&download_id).unwrap_or_default()
    };

    let mut pending_blocks: VecDeque<(u64, u64)> = VecDeque::new();
    for range in missing_ranges {
        let mut offset = range.start;
        while offset < range.end {
            let chunk_len = std::cmp::min(184_320, range.end - offset);
            pending_blocks.push_back((offset, offset + chunk_len));
            offset += chunk_len;
        }
    }

    // 6. Pipeline block requests
    while !pending_blocks.is_empty() || client.session().pending_block_request_count() > 0 {
        // Check if download was paused or removed
        {
            let dls = state.downloads.lock().await;
            if let Some(snap) = dls.get_download(&download_id) {
                if snap.status == DownloadStatus::Paused.as_str() {
                    break;
                }
            } else {
                break;
            }
        }

        // Fill pipeline up to MAX_IN_FLIGHT_BLOCK_REQUESTS (8)
        while client.session().block_tracker().can_request() {
            if let Some((start, end)) = pending_blocks.pop_front() {
                client
                    .send_block_request(expected_hash.0, start, end, total_size)
                    .await?;
            } else {
                break;
            }
        }

        if pending_blocks.is_empty() && client.session().pending_block_request_count() == 0 {
            break;
        }

        // Receive block response within timeout
        match client
            .receive_sending_part(Duration::from_millis(2000))
            .await
        {
            Ok(part) => {
                if part.file_hash != expected_hash.0 {
                    let mut dls = state.downloads.lock().await;
                    let snap = dls.update_status(&download_id, DownloadStatus::Failed)?;
                    state.broadcast_download_update(snap);
                    return Err(format!(
                        "Protocol violation: block for unexpected file hash {}",
                        hex::encode(part.file_hash)
                    )
                    .into());
                }

                let snap = {
                    let mut dls = state.downloads.lock().await;
                    dls.inject_range(&download_id, part.start_offset as u64, &part.data)?
                };
                state.broadcast_download_update(snap);
            }
            Err(Ed2kSessionError::Timeout(d)) => {
                // Re-queue timed out requests
                let expired = client
                    .session_mut()
                    .expire_block_requests(d, std::time::Instant::now());
                for req in expired {
                    pending_blocks.push_front((req.key.start, req.key.end));
                }
            }
            Err(e) => {
                let mut dls = state.downloads.lock().await;
                let snap = dls.update_status(&download_id, DownloadStatus::Failed)?;
                state.broadcast_download_update(snap);
                return Err(Box::new(e));
            }
        }
    }

    // 7. Check if download is complete
    let is_complete = {
        let dls = state.downloads.lock().await;
        dls.get_missing_ranges(&download_id)
            .map(|m| m.is_empty())
            .unwrap_or(false)
    };

    if !is_complete {
        client.close().await;
        return Ok(());
    }

    // 8. Transition to Verifying & verify root eD2K hash
    {
        let mut dls = state.downloads.lock().await;
        let snap = dls.update_status(&download_id, DownloadStatus::Verifying)?;
        state.broadcast_download_update(snap);
    }

    let mut file = std::fs::File::open(&temp_path)?;
    let computed_result = mule_format::ed2k::hash_reader(&mut file)?;

    client.close().await;

    if computed_result.root_hash == expected_hash {
        let mut dls = state.downloads.lock().await;
        let snap = dls.finalize_download(&download_id)?;
        state.broadcast_download_update(snap);
        info!(download_id = %download_id, "eD2K download successfully verified and finalized");
        Ok(())
    } else {
        let mut dls = state.downloads.lock().await;
        let snap = dls.update_status(&download_id, DownloadStatus::Failed)?;
        state.broadcast_download_update(snap);
        error!(download_id = %download_id, "eD2K root hash verification failed");
        Err("eD2K hash mismatch".into())
    }
}
