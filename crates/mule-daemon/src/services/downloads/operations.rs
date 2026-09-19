use mule_core::{AllocationPolicy, Database, Download, DownloadStatus, PartFile, RangeSet};
use mule_format::Ed2kFileLink;
use mule_protocol::ed2k::PartAvailabilityMask;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use uuid::Uuid;

use crate::error::DaemonError;
use super::history_categories::record_history_event;
use super::types::{
    make_snapshot, now_ms, ActiveDownload, ActiveSource, DownloadDetail, DownloadSnapshot,
    PartProgressSummary,
};

pub fn add_ed2k_link(
    db: &mut Database,
    temp_dir: &Path,
    active: &mut HashMap<Uuid, ActiveDownload>,
    link_str: &str,
    target_dir: &Path,
    policy: AllocationPolicy,
    category_id: Option<String>,
) -> Result<DownloadSnapshot, DaemonError> {
    let link = Ed2kFileLink::parse(link_str)
        .map_err(|e| DaemonError::InvalidParams(e.to_string()))?;

    let clean_hex = link.ed2k_hash.to_hex().to_lowercase();
    if let Some(existing) = active
        .values()
        .find(|a| a.download.ed2k_hash.to_hex().to_lowercase() == clean_hex)
    {
        return Err(DaemonError::Business(format!(
            "Ce fichier est déjà dans la liste des téléchargements ('{}', statut: {:?})",
            existing.download.display_name, existing.download.status
        )));
    }

    let id = Uuid::new_v4();
    let temp_filename = format!("{id}.part");
    let temp_path = temp_dir.join(temp_filename);
    let target_path = target_dir.join(&link.file_name);

    let download = Download {
        id,
        display_name: link.file_name.clone(),
        total_size: link.size,
        ed2k_hash: link.ed2k_hash,
        aich_root: link.aich_root.map(mule_format::AichHash),
        target_path,
        temp_path: temp_path.clone(),
        status: DownloadStatus::Queued,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64,
        updated_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64,
        category_id,
    };

    let part_file = PartFile::open_or_create(&temp_path, link.size, policy)
        .map_err(|e| DaemonError::Business(format!("Storage error: {e}")))?;

    db.insert_download(&download)
        .map_err(|e| DaemonError::Business(format!("DB error: {e}")))?;

    let ranges = RangeSet::new();
    let snapshot = make_snapshot(&download, &ranges, 0, 0);
    let now = now_ms();

    active.insert(
        id,
        ActiveDownload {
            download,
            ranges,
            part_file: Some(part_file),
            sources: HashMap::new(),
            connection_attempts: 0,
            resume_count: 0,
            last_error: None,
            last_status_change_unix_ms: now,
            session_start_ms: now,
            session_bytes_start: 0,
            current_rate_bps: 0,
            last_rate_update: None,
            source_count: 0,
            verification_in_progress: false,
        },
    );

    Ok(snapshot)
}

pub fn get_download_detail(
    db: &Database,
    active: &HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
) -> Option<DownloadDetail> {
    let act = active.get(id)?;
    let snapshot = act.snapshot();

    let total_size = act.download.total_size;
    let part_size_const = mule_format::ed2k::hash::ED2K_PART_SIZE as u64;
    let part_count = if total_size == 0 {
        1
    } else {
        total_size.div_ceil(part_size_const) as u32
    };

    let verified_parts_set: std::collections::HashSet<u32> = db
        .get_verified_parts(id)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let all_ranges = act.ranges.ranges();

    let mut parts = Vec::with_capacity(part_count as usize);
    let mut total_verified_bytes = 0u64;

    for i in 0..part_count {
        let start_offset = i as u64 * part_size_const;
        let end_offset = ((i as u64 + 1) * part_size_const).min(total_size);
        let size = end_offset.saturating_sub(start_offset);

        let received_in_part: u64 = all_ranges
            .iter()
            .filter_map(|r| {
                let s = r.start.max(start_offset);
                let e = r.end.min(end_offset);
                if s < e {
                    Some(e - s)
                } else {
                    None
                }
            })
            .sum();

        let is_verified = verified_parts_set.contains(&i);
        if is_verified {
            total_verified_bytes += size;
        }

        let state = if is_verified {
            "verified"
        } else if received_in_part >= size && size > 0 {
            "completed"
        } else if received_in_part > 0 {
            "downloading"
        } else {
            "missing"
        };

        parts.push(PartProgressSummary {
            part_index: i,
            start_offset,
            end_offset,
            size,
            received_bytes: received_in_part,
            is_verified,
            state: state.to_string(),
        });
    }

    let aich_part = act
        .download
        .aich_root
        .as_ref()
        .map(|a| format!("&h={}", a.to_base32()))
        .unwrap_or_default();

    let ed2k_link = format!(
        "ed2k://|file|{}|{}|{}|{}/",
        act.download.display_name,
        act.download.total_size,
        hex::encode(act.download.ed2k_hash.0),
        aich_part
    );

    let covered = act.ranges.covered_len();
    let remaining_bytes = act.download.total_size.saturating_sub(covered);

    let effective_rate = snapshot.download_rate_bps;
    let average_rate_bps = if act.session_start_ms > 0 {
        let now_time = now_ms();
        let elapsed_secs = ((now_time - act.session_start_ms) / 1000).max(1) as u64;
        let transferred_session = covered.saturating_sub(act.session_bytes_start);
        transferred_session / elapsed_secs
    } else {
        0
    };

    let eta_seconds = if effective_rate > 0 && remaining_bytes > 0 {
        Some(remaining_bytes / effective_rate)
    } else {
        None
    };

    let mut enriched_snapshot = snapshot;
    enriched_snapshot.verified_bytes = total_verified_bytes;

    Some(DownloadDetail {
        snapshot: enriched_snapshot,
        ed2k_hash: hex::encode(act.download.ed2k_hash.0),
        aich_root: act.download.aich_root.as_ref().map(|a| a.to_base32()),
        ed2k_link,
        temp_path: act.download.temp_path.to_string_lossy().to_string(),
        target_path: act.download.target_path.to_string_lossy().to_string(),
        remaining_bytes,
        average_rate_bps,
        eta_seconds,
        connection_attempts: act.connection_attempts,
        last_error: act.last_error.clone(),
        last_status_change_unix_ms: act.last_status_change_unix_ms,
        parts,
    })
}

pub fn pause_download(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
) -> Result<DownloadSnapshot, DaemonError> {
    let act = active
        .get_mut(id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;

    if act.download.status == DownloadStatus::Completed {
        return Ok(act.snapshot());
    }

    let now = now_ms();
    act.download.status = DownloadStatus::Paused;
    act.download.updated_at = now;
    act.last_status_change_unix_ms = now;
    db.update_status(id, DownloadStatus::Paused)
        .map_err(|e| DaemonError::Business(format!("DB error: {e}")))?;

    Ok(act.snapshot())
}

pub fn resume_download(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
) -> Result<DownloadSnapshot, DaemonError> {
    let (dl, ranges, conn_att, res_cnt) = {
        let act = active
            .get_mut(id)
            .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;

        let now = now_ms();
        if act.download.status == DownloadStatus::Failed
            && act.ranges.covered_len() == act.download.total_size
        {
            act.ranges = RangeSet::new();
            let _ = db.sync_received_ranges(id, &act.ranges);
        }
        if act.part_file.is_none() {
            act.part_file = PartFile::open_or_create(
                &act.download.temp_path,
                act.download.total_size,
                AllocationPolicy::None,
            )
            .ok();
        }
        act.download.status = DownloadStatus::Downloading;
        act.download.updated_at = now;
        act.last_status_change_unix_ms = now;
        act.resume_count = act.resume_count.saturating_add(1);
        act.verification_in_progress = false;
        let _ = db.update_status(id, DownloadStatus::Downloading);

        (
            act.download.clone(),
            act.ranges.clone(),
            act.connection_attempts,
            act.resume_count,
        )
    };

    record_history_event(
        db,
        &dl,
        ranges.covered_len(),
        conn_att,
        res_cnt,
        "resumed",
        "unverified",
        None,
    );

    let act = active.get(id).unwrap();
    Ok(act.snapshot())
}

pub fn cancel_download(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
) -> Result<DownloadSnapshot, DaemonError> {
    let act = active
        .get_mut(id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;

    let now = now_ms();
    act.download.status = DownloadStatus::Removed;
    act.download.updated_at = now;
    act.last_status_change_unix_ms = now;
    db.update_status(id, DownloadStatus::Removed)
        .map_err(|e| DaemonError::Business(format!("DB error: {e}")))?;

    let snapshot = act.snapshot();
    let dl = act.download.clone();
    let covered = act.ranges.covered_len();
    let conn_att = act.connection_attempts;
    let res_cnt = act.resume_count;

    record_history_event(
        db,
        &dl,
        covered,
        conn_att,
        res_cnt,
        "cancelled",
        "unverified",
        None,
    );

    Ok(snapshot)
}

pub fn remove_download(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
    delete_files: bool,
) -> Result<(), DaemonError> {
    let mut temp_path_to_delete = None;
    let (was_active, history_data) = if let Some(mut act) = active.remove(id) {
        let _ = act.part_file.take();
        if delete_files {
            temp_path_to_delete = Some(act.download.temp_path.clone());
        }
        let verif = if act.download.status == DownloadStatus::Completed {
            "verified_ok"
        } else {
            "unverified"
        };
        (
            true,
            Some((
                act.download,
                act.ranges.covered_len(),
                act.connection_attempts,
                act.resume_count,
                verif,
            )),
        )
    } else {
        (false, None)
    };

    if let Some((dl, transferred, attempts, resumes, verif)) = history_data {
        record_history_event(
            db,
            &dl,
            transferred,
            attempts,
            resumes,
            "removed",
            verif,
            None,
        );
    }

    let db_deleted = db
        .delete_download(id)
        .map_err(|e| DaemonError::Business(format!("DB error deleting download: {e}")))?;

    if let Some(temp_path) = temp_path_to_delete {
        if temp_path.exists() {
            let _ = std::fs::remove_file(temp_path);
        }
    }

    if was_active || db_deleted {
        Ok(())
    } else {
        Err(DaemonError::NotFound(format!("Download {id} not found")))
    }
}

pub fn inject_range(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
    offset: u64,
    data: &[u8],
) -> Result<DownloadSnapshot, DaemonError> {
    let act = active
        .get_mut(id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;

    if act.download.status == DownloadStatus::Completed {
        tracing::debug!("Download {id} is already completed; ignoring incoming range {offset}..{}", offset + data.len() as u64);
        return Ok(act.snapshot());
    }
    if act.download.status == DownloadStatus::Verifying {
        tracing::debug!("Download {id} is currently verifying; ignoring incoming range {offset}..{}", offset + data.len() as u64);
        return Ok(act.snapshot());
    }

    let end = offset + data.len() as u64;

    if let Some(pf) = &mut act.part_file {
        pf.write_segment(offset, data)
            .map_err(|e| DaemonError::Business(format!("Write error: {e}")))?;
        pf.flush()
            .map_err(|e| DaemonError::Business(format!("Flush error: {e}")))?;
    }

    let was_complete = act.download.total_size > 0
        && act.ranges.covered_len() == act.download.total_size;

    const ED2K_PART_SIZE: u64 = 9_728_000;
    let part_idx = offset / ED2K_PART_SIZE;
    let part_start = part_idx * ED2K_PART_SIZE;
    let part_end = ((part_idx + 1) * ED2K_PART_SIZE).min(act.download.total_size);
    let part_was_complete = part_end > part_start && act.ranges.contains(part_start..part_end);

    act.ranges
        .insert(offset..end)
        .map_err(|e| DaemonError::Business(format!("Range error: {e}")))?;

    let part_is_complete = part_end > part_start && act.ranges.contains(part_start..part_end);
    if part_is_complete && !part_was_complete {
        let total_parts = ((act.download.total_size + ED2K_PART_SIZE - 1) / ED2K_PART_SIZE).max(1);
        let progress = (act.ranges.covered_len() as f64 / act.download.total_size.max(1) as f64) * 100.0;
        tracing::info!(
            "[PART_COMPLETED] File '{}' (hash: {}): Part {}/{} (offset {}..{}) downloaded & written to disk. Progress: {:.1}%",
            act.download.display_name,
            act.download.ed2k_hash.to_hex(),
            part_idx + 1,
            total_parts,
            part_start,
            part_end,
            progress
        );
    }

    db.sync_received_ranges(id, &act.ranges)
        .map_err(|e| DaemonError::Business(format!("DB sync error: {e}")))?;

    let is_now_complete = act.download.total_size > 0
        && act.ranges.covered_len() == act.download.total_size;

    if is_now_complete && !was_complete {
        let _ = act.part_file.take();
        act.download.status = DownloadStatus::Verifying;
        act.download.updated_at = now_ms();
        let _ = db.update_status(id, DownloadStatus::Verifying);
    } else if act.download.status != DownloadStatus::Verifying
        && act.download.status != DownloadStatus::Completed
    {
        act.download.status = DownloadStatus::Downloading;
        act.download.updated_at = now_ms();
        let _ = db.update_status(id, act.download.status);
    }

    Ok(act.snapshot())
}

pub fn update_status(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
    status: DownloadStatus,
) -> Result<DownloadSnapshot, DaemonError> {
    let (old_status, dl, transferred, attempts, resumes, last_error) = {
        let act = active
            .get_mut(id)
            .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;

        let old = act.download.status;
        act.download.status = status;
        act.download.updated_at = now_ms();
        if status != DownloadStatus::Verifying {
            act.verification_in_progress = false;
        }
        (
            old,
            act.download.clone(),
            act.ranges.covered_len(),
            act.connection_attempts,
            act.resume_count,
            act.last_error.clone(),
        )
    };

    db.update_status(id, status)
        .map_err(|e| DaemonError::Business(format!("DB error: {e}")))?;

    if status == DownloadStatus::Failed {
        let verif = if old_status == DownloadStatus::Verifying {
            "hash_mismatch"
        } else {
            "unverified"
        };
        record_history_event(
            db,
            &dl,
            transferred,
            attempts,
            resumes,
            "failed",
            verif,
            last_error,
        );
    } else if status == DownloadStatus::Verifying {
        record_history_event(
            db,
            &dl,
            transferred,
            attempts,
            resumes,
            "verified_pending_promotion",
            "unverified",
            None,
        );
    }

    let act = active.get(id).unwrap();
    Ok(act.snapshot())
}

pub fn finalize_download(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
) -> Result<DownloadSnapshot, DaemonError> {
    let (temp_path, target_path, dl, transferred, attempts, resumes) = {
        let act = active
            .get_mut(id)
            .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;
        let _ = act.part_file.take();
        (
            act.download.temp_path.clone(),
            act.download.target_path.clone(),
            act.download.clone(),
            act.ranges.covered_len(),
            act.connection_attempts,
            act.resume_count,
        )
    };

    mule_core::finalize_file(
        &temp_path,
        &target_path,
        mule_core::CollisionStrategy::Overwrite,
    )
    .map_err(|e| DaemonError::Business(format!("Finalize error: {e}")))?;

    let now = now_ms();
    {
        let act = active.get_mut(id).unwrap();
        act.download.status = DownloadStatus::Completed;
        act.verification_in_progress = false;
        act.download.updated_at = now;
        act.last_status_change_unix_ms = now;
        act.current_rate_bps = 0;
        let _ = act.part_file.take();
    }
    db.update_status(id, DownloadStatus::Completed)
        .map_err(|e| DaemonError::Business(format!("DB error: {e}")))?;

    let mut finalized_dl = dl;
    finalized_dl.status = DownloadStatus::Completed;
    record_history_event(
        db,
        &finalized_dl,
        transferred,
        attempts,
        resumes,
        "completed",
        "verified_ok",
        None,
    );

    let shared = mule_core::model::SharedFile::new(
        &finalized_dl.ed2k_hash.to_hex(),
        &finalized_dl.display_name,
        finalized_dl.total_size,
        target_path.to_string_lossy().as_ref(),
    );
    let _ = db.upsert_shared_file(&shared);

    let act = active.get(id).unwrap();
    Ok(act.snapshot())
}

pub fn set_destination(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
    target_dir: &Path,
) -> Result<DownloadDetail, DaemonError> {
    let act = active
        .get_mut(id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;

    std::fs::create_dir_all(target_dir)
        .map_err(|e| DaemonError::Business(format!("Cannot create target directory: {e}")))?;

    let probe_file = target_dir.join(format!(".mule_write_probe_{}", Uuid::new_v4()));
    match std::fs::write(&probe_file, b"probe") {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe_file);
        }
        Err(e) => {
            return Err(DaemonError::Business(format!(
                "Target directory is not writable: {e}"
            )));
        }
    }

    let new_target_path = target_dir.join(&act.download.display_name);
    act.download.target_path = new_target_path.clone();
    act.download.updated_at = now_ms();

    db.update_target_path(id, &new_target_path)
        .map_err(|e| DaemonError::Business(format!("DB error updating target path: {e}")))?;

    get_download_detail(db, active, id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} detail not found")))
}

pub fn retry_connection(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
) -> Result<DownloadDetail, DaemonError> {
    let act = active
        .get_mut(id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;

    let now = now_ms();
    act.connection_attempts = act.connection_attempts.saturating_add(1);
    act.last_error = None;
    act.last_status_change_unix_ms = now;
    act.download.updated_at = now;

    if act.download.status == DownloadStatus::Failed
        || act.download.status == DownloadStatus::Paused
    {
        if act.download.status == DownloadStatus::Failed
            && act.ranges.covered_len() == act.download.total_size
        {
            act.ranges = RangeSet::new();
            let _ = db.sync_received_ranges(id, &act.ranges);
        }
        if act.part_file.is_none() {
            act.part_file = PartFile::open_or_create(
                &act.download.temp_path,
                act.download.total_size,
                AllocationPolicy::None,
            )
            .ok();
        }
        act.download.status = DownloadStatus::Downloading;
        act.verification_in_progress = false;
        let _ = db.update_status(id, DownloadStatus::Downloading);
    }

    get_download_detail(db, active, id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} detail not found")))
}

pub fn record_source_availability(
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
    source: SocketAddr,
    mask: PartAvailabilityMask,
) -> Result<DownloadSnapshot, DaemonError> {
    let act = active
        .get_mut(id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;

    let now = now_ms();
    let source_id = format!("src_{:08x}", rand::random::<u32>());
    let entry = act.sources.entry(source).or_insert_with(|| ActiveSource {
        source_id,
        _addr: source,
        mask: mask.clone(),
        last_seen_unix_ms: now,
    });
    entry.mask = mask;
    entry.last_seen_unix_ms = now;
    act.download.updated_at = now;

    Ok(act.snapshot())
}
