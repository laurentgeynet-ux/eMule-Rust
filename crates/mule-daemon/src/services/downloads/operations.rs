// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use mule_core::{AllocationPolicy, Database, Download, DownloadStatus, PartFile, RangeSet};
use mule_format::Ed2kFileLink;
use mule_protocol::ed2k::PartAvailabilityMask;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::db_err;
use super::history_categories::record_history_event;
use super::peers_servers;
use super::types::{
    compute_part_summaries, make_snapshot, now_ms, ActiveDownload, ActiveSource, DownloadDetail,
    DownloadSnapshot,
};
use crate::error::DaemonError;
use std::time::{Duration, Instant};

/// Taille d'une part eD2K (eMule `PARTSIZE`) — granularité du hachage MD4
/// incrémental et de la récupération AICH.
const ED2K_PART_SIZE_U64: u64 = 9_728_000;

pub fn add_ed2k_link(
    db: &mut Database,
    temp_dir: &Path,
    active: &mut HashMap<Uuid, ActiveDownload>,
    link_str: &str,
    target_dir: &Path,
    policy: AllocationPolicy,
    category_id: Option<String>,
) -> Result<DownloadSnapshot, DaemonError> {
    let link =
        Ed2kFileLink::parse(link_str).map_err(|e| DaemonError::InvalidParams(e.to_string()))?;

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
        // eMule `DAPPref` (`GetNewAutoDown`, défaut true) : un nouveau
        // download démarre en priorité « auto » — le flag est stocké et
        // l'allocateur résout low/normal/high à chaque tick selon le
        // nombre de sources (`UpdateAutoDownPriority`). Ce n'est pas une
        // assignation ponctuelle : « auto » reste actif tant que
        // l'utilisateur ne choisit pas une priorité manuelle.
        priority: if db
            .get_setting("auto_priority_new_downloads")
            .ok()
            .flatten()
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "on"))
            .unwrap_or(true)
        {
            "auto".to_string()
        } else {
            "normal".to_string()
        },
        max_sources: None,
    };

    let part_file = PartFile::open_or_create(&temp_path, link.size, policy)
        .map_err(|e| DaemonError::Business(format!("Storage error: {e}")))?;

    db.insert_download(&download)
        .map_err(db_err("persisting state"))?;

    if !link.part_hashes.is_empty() {
        let _ = db.store_part_hashes(&clean_hex, &link.part_hashes);
    }

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
            aich_pending: Default::default(),
            aich_requested: Default::default(),
            aich_votes: Default::default(),
            corruption_blackbox: Default::default(),
            pending_corruption_bans: Vec::new(),
        },
    );

    // eMule creates the .part.met companion as soon as the download is added.
    if let Some(act) = active.get(&id) {
        let _ = super::part_met::save_part_met(db, act);
    }

    Ok(snapshot)
}

pub fn get_download_detail(
    db: &Database,
    active: &HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
) -> Option<DownloadDetail> {
    let act = active.get(id)?;
    let snapshot = act.snapshot();

    let verified_parts_set: std::collections::HashSet<u32> = db
        .get_verified_parts(id)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let parts = compute_part_summaries(
        act.download.total_size,
        &act.ranges,
        act.sources.values(),
        &verified_parts_set,
    );
    let total_verified_bytes: u64 = parts.iter().filter(|p| p.is_verified).map(|p| p.size).sum();

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
        .map_err(db_err("persisting state"))?;

    // Persist .part.met on pause (eMule saves met on status transitions).
    let _ = super::part_met::save_part_met(db, act);

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

pub fn remove_download(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
    delete_files: bool,
) -> Result<(), DaemonError> {
    let mut temp_path_to_delete = None;
    let mut temp_path_to_keep = None;
    // Téléchargement terminé : `delete_files` couvre aussi le fichier final
    // dans le dossier de destination (retiré de la liste des partages).
    let mut completed_file_to_delete: Option<(PathBuf, String)> = None;
    let (was_active, history_data) = if let Some(mut act) = active.remove(id) {
        let _ = act.part_file.take();
        if delete_files {
            temp_path_to_delete = Some(act.download.temp_path.clone());
            if act.download.status == DownloadStatus::Completed {
                completed_file_to_delete = Some((
                    act.download.target_path.clone(),
                    act.download.ed2k_hash.to_hex(),
                ));
            }
        } else {
            temp_path_to_keep = Some(act.download.temp_path.clone());
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
        .map_err(db_err("deleting download"))?;

    if let Some(temp_path) = temp_path_to_delete {
        if temp_path.exists() {
            let _ = std::fs::remove_file(&temp_path);
        }
        super::part_met::delete_part_met(&temp_path);
    }

    // Fichier final partagé : supprimé du dossier de destination et retiré
    // de `shared_files` (enregistré automatiquement à la complétion —
    // `finalize_download`), sinon la liste des partages pointerait un
    // fichier inexistant.
    if let Some((target_path, hash_hex)) = completed_file_to_delete {
        if target_path.exists() {
            let _ = std::fs::remove_file(&target_path);
        }
        let _ = db.delete_shared_file(&hash_hex);
    }

    // Files kept on disk after removal: rename the companion met to
    // `.part.met.keep` so the startup orphan scan does not resurrect the
    // download the user deliberately removed. The pair stays importable via
    // `downloads.importPartMet` (explicit partPath).
    if let Some(temp_path) = temp_path_to_keep {
        let met_path = super::part_met::part_met_path_for(&temp_path);
        if met_path.exists() {
            let mut keep_name = met_path.as_os_str().to_os_string();
            keep_name.push(".keep");
            let keep_path = PathBuf::from(keep_name);
            let _ = std::fs::remove_file(&keep_path);
            let _ = std::fs::rename(&met_path, &keep_path);
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
    sender: Option<&str>,
) -> Result<DownloadSnapshot, DaemonError> {
    let act = active
        .get_mut(id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;

    if act.download.status == DownloadStatus::Completed {
        tracing::debug!(
            "Download {id} is already completed; ignoring incoming range {offset}..{}",
            offset + data.len() as u64
        );
        return Ok(act.snapshot());
    }
    if act.download.status == DownloadStatus::Verifying {
        tracing::debug!(
            "Download {id} is currently verifying; ignoring incoming range {offset}..{}",
            offset + data.len() as u64
        );
        return Ok(act.snapshot());
    }
    if act.download.status == DownloadStatus::Removed {
        tracing::debug!(
            "Download {id} is removed; ignoring incoming range {offset}..{}",
            offset + data.len() as u64
        );
        return Ok(act.snapshot());
    }
    if data.is_empty() {
        return Ok(act.snapshot());
    }

    let end = offset + data.len() as u64;

    // eMule `WriteToBuffer` : toute donnée touchant une part déjà complète
    // (sans gap, donc déjà hachée ou en attente de vérif) est ignorée —
    // « Received data touches already hashed chunk - ignored ». Sans cette
    // garde, un bloc en vol planifié avant la complétion de la part
    // écraserait silencieusement des données déjà vérifiées.
    let first_part = offset / ED2K_PART_SIZE_U64;
    let last_part = (end - 1) / ED2K_PART_SIZE_U64;
    for p in first_part..=last_part {
        let ps = p * ED2K_PART_SIZE_U64;
        let pe = ((p + 1) * ED2K_PART_SIZE_U64).min(act.download.total_size);
        if pe > ps && act.ranges.contains(ps..pe) {
            tracing::warn!(
                "Download {id}: received data {offset}..{end} touches already complete part {p} — ignored (sender: {sender:?})"
            );
            return Ok(act.snapshot());
        }
    }

    // eMule `m_CorruptionBlackBox.ReceivedData` : provenance du bloc par part.
    if let Some(sender) = sender {
        let mut s = offset;
        while s < end {
            let p = s / ED2K_PART_SIZE_U64;
            let pe = ((p + 1) * ED2K_PART_SIZE_U64).min(end);
            act.corruption_blackbox
                .received_data(p as u32, s, pe, sender);
            s = pe;
        }
    }

    // Re-open the .part file when it was closed (failed open at startup,
    // post-verify eviction): received bytes must reach disk — the range is
    // only recorded once the write succeeded, never as phantom coverage.
    if act.part_file.is_none() {
        act.part_file = Some(
            PartFile::open_or_create(
                &act.download.temp_path,
                act.download.total_size,
                AllocationPolicy::None,
            )
            .map_err(|e| DaemonError::Business(format!("Cannot open .part file: {e}")))?,
        );
    }
    if let Some(pf) = &mut act.part_file {
        pf.write_segment(offset, data)
            .map_err(|e| DaemonError::Business(format!("Write error: {e}")))?;
        pf.flush()
            .map_err(|e| DaemonError::Business(format!("Flush error: {e}")))?;
    }

    let was_complete =
        act.download.total_size > 0 && act.ranges.covered_len() == act.download.total_size;

    let part_idx = offset / ED2K_PART_SIZE_U64;
    let part_start = part_idx * ED2K_PART_SIZE_U64;
    let part_end = ((part_idx + 1) * ED2K_PART_SIZE_U64).min(act.download.total_size);
    let part_was_complete = part_end > part_start && act.ranges.contains(part_start..part_end);

    act.ranges
        .insert(offset..end)
        .map_err(|e| DaemonError::Business(format!("Range error: {e}")))?;

    let part_is_complete = part_end > part_start && act.ranges.contains(part_start..part_end);
    let mut met_dirty = false;
    if part_is_complete && !part_was_complete {
        met_dirty = true;
        let total_parts = act.download.total_size.div_ceil(ED2K_PART_SIZE_U64).max(1);
        let progress =
            (act.ranges.covered_len() as f64 / act.download.total_size.max(1) as f64) * 100.0;
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

        // Vérif d'intégrité incrémentale inconditionnelle : eMule exécute
        // `HashSinglePart` (MD4 + AICH) sur chaque part complétée,
        // indépendamment de la préférence ICH. `ich_enabled` ne gate que le
        // choix entre récupération AICH par blocs et éviction complète de la
        // part (`IsCorruptedPart && IsICHEnabled` dans FlushBuffer).
        let ich_enabled = db
            .get_setting("ich_enabled")
            .ok()
            .flatten()
            .map(|v| !matches!(v.trim().to_lowercase().as_str(), "0" | "false" | "off"))
            .unwrap_or(true);
        let hash_hex = act.download.ed2k_hash.to_hex().to_lowercase();
        if let Ok(part_hashes) = db.get_part_hashes(&hash_hex) {
            // For files strictly smaller than one part, the file hash IS the
            // part hash. For exact multiples of PART_SIZE eMule appends an
            // MD4(empty) trailing part, so part 0 hash != file hash there —
            // never fall back to the file hash in that case.
            let expected = part_hashes.get(part_idx as usize).copied().or({
                if part_hashes.is_empty() && act.download.total_size < ED2K_PART_SIZE_U64 {
                    Some(act.download.ed2k_hash.0)
                } else {
                    None
                }
            });
            if let Some(expected_hash) = expected {
                let part_len = (part_end - part_start) as usize;
                let verified = act
                    .part_file
                    .as_mut()
                    .and_then(|pf| pf.read_range(part_start, part_len).ok())
                    .map(|bytes| mule_core::verify_part_hash(&bytes, &expected_hash))
                    .unwrap_or(false);
                if verified {
                    let _ = db.mark_part_verified(id, part_idx as u32);
                    act.corruption_blackbox
                        .verified_data(part_idx as u32, part_start, part_end);
                } else {
                    // La part est corrompue : attribution aux expéditeurs
                    // (eMule CorruptedData + EvaluateData → Ban >32 %).
                    mark_part_corrupted(act, part_idx as u32, part_start, part_end);
                    // eMule AICH path: with a trusted master hash the corrupted
                    // part is kept and an OP_AICHREQUEST is queued so only the
                    // corrupted 180 KiB blocks get re-downloaded. Falls back to
                    // whole-part eviction when AICH is unavailable/exhausted,
                    // ou quand ICH est désactivé (re-téléchargement complet).
                    let attempts = act
                        .aich_requested
                        .get(&(part_idx as u32))
                        .map(|&(_, c)| c)
                        .unwrap_or(0);
                    let already_pending =
                        act.aich_pending.iter().any(|&(p, _)| p == part_idx as u32);
                    if ich_enabled
                        && act.download.aich_root.is_some()
                        && attempts < AICH_MAX_ATTEMPTS
                        && !already_pending
                    {
                        tracing::warn!(
                        "[PART_CORRUPTED] File '{}' (hash: {}): part {} failed MD4 check, queued for AICH block recovery",
                        act.download.display_name,
                        hash_hex,
                        part_idx
                    );
                        act.aich_pending
                            .push_back((part_idx as u32, Instant::now()));
                    } else {
                        tracing::warn!(
                        "[PART_CORRUPTED] File '{}' (hash: {}): part {} failed MD4 check, evicting range {}..{} for re-download",
                        act.download.display_name,
                        hash_hex,
                        part_idx,
                        part_start,
                        part_end
                    );
                        let _ = act.ranges.remove(part_start..part_end);
                        let _ = db.unmark_part_verified(id, part_idx as u32);
                    }
                }
            }
        }
    }

    db.sync_received_ranges(id, &act.ranges)
        .map_err(|e| DaemonError::Business(format!("DB sync error: {e}")))?;

    let is_now_complete =
        act.download.total_size > 0 && act.ranges.covered_len() == act.download.total_size;

    if is_now_complete && !was_complete {
        let _ = act.part_file.take();
        act.download.status = DownloadStatus::Verifying;
        act.download.updated_at = now_ms();
        let _ = db.update_status(id, DownloadStatus::Verifying);
        met_dirty = true;
    } else if matches!(
        act.download.status,
        DownloadStatus::Queued | DownloadStatus::WaitingForSources
    ) {
        // Only a pending download flips to Downloading on data arrival.
        // Paused/Failed keep their state: eMule finishes in-flight writes
        // without un-pausing the partfile.
        act.download.status = DownloadStatus::Downloading;
        act.download.updated_at = now_ms();
        let _ = db.update_status(id, act.download.status);
    }

    // Persist .part.met on part completion / corruption eviction
    // (eMule SavePartFile on state transitions).
    if met_dirty {
        let _ = super::part_met::save_part_met(db, act);
    }

    Ok(act.snapshot())
}

/// Marque les enregistrements de la part comme corrompus puis évalue la
/// responsabilité des expéditeurs (eMule `CorruptedData` + `EvaluateData`) :
/// tout expéditeur dépassant `CBB_BANTHRESHOLD` est empilé dans
/// `pending_corruption_bans` pour déconnexion/rejet par l'appelant.
fn mark_part_corrupted(
    act: &mut ActiveDownload,
    part_idx: u32,
    part_start: u64,
    part_end: u64,
) {
    act.corruption_blackbox
        .corrupted_data(part_idx, part_start, part_end);
    for guilty in act.corruption_blackbox.evaluate(part_idx) {
        if !act.pending_corruption_bans.contains(&guilty) {
            tracing::warn!(
                "[CORRUPTION_BLACKBOX] '{}' : {guilty} a dépassé le seuil de corruption (eMule CBB_BANTHRESHOLD) — bannissement demandé",
                act.download.display_name
            );
            act.pending_corruption_bans.push(guilty);
        }
    }
}

/// Evicts corrupted part ranges from the received RangeSet so they are re-downloaded,
/// and resets the download back to Downloading. Mirrors eMule `CPartFile` corrupted
/// part handling after a failed hashset check.
pub fn evict_corrupted_parts(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
    corrupted: &[u32],
) -> Result<DownloadSnapshot, DaemonError> {
    let act = active
        .get_mut(id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;

    const ED2K_PART_SIZE: u64 = ED2K_PART_SIZE_U64;
    let total_size = act.download.total_size;

    for &part_idx in corrupted {
        let start = part_idx as u64 * ED2K_PART_SIZE;
        let end = ((part_idx as u64 + 1) * ED2K_PART_SIZE).min(total_size);
        if end > start {
            let _ = act.ranges.remove(start..end);
            mark_part_corrupted(act, part_idx, start, end);
        }
        let _ = db.unmark_part_verified(id, part_idx);
    }

    db.sync_received_ranges(id, &act.ranges)
        .map_err(|e| DaemonError::Business(format!("DB sync error: {e}")))?;

    // Re-open the part file for further writes (it was closed when entering Verifying).
    if act.part_file.is_none() {
        act.part_file =
            PartFile::open_or_create(&act.download.temp_path, total_size, AllocationPolicy::None)
                .ok();
    }

    let now = now_ms();
    act.download.status = DownloadStatus::Downloading;
    act.download.updated_at = now;
    act.last_status_change_unix_ms = now;
    act.verification_in_progress = false;
    let _ = db.update_status(id, DownloadStatus::Downloading);

    // Clear AICH recovery state for evicted parts.
    for &part_idx in corrupted {
        act.aich_pending.retain(|&(p, _)| p != part_idx);
        act.aich_requested.remove(&part_idx);
    }

    tracing::warn!(
        "[CORRUPTION_RECOVERY] Download {id} ('{}'): {} corrupted part(s) evicted, back to Downloading ({:.1}% remaining)",
        act.download.display_name,
        corrupted.len(),
        (1.0 - act.ranges.covered_len() as f64 / total_size.max(1) as f64) * 100.0
    );

    let _ = super::part_met::save_part_met(db, act);
    Ok(act.snapshot())
}

// ── AICH recovery (eMule CAICHRecoveryHashSet semantics) ─────────────────────

/// Minimum unique masked IPs (`ip & 0x00F0FFFF`) required to trust a root.
pub const AICH_MIN_UNIQUE_IPS: usize = 10;
/// Minimum share of agreeing IPs to trust a root (percent).
pub const AICH_MIN_PERCENTAGE: usize = 92;
/// Maximum AICH recovery requests sent per corrupted part before falling back
/// to whole-part eviction.
pub const AICH_MAX_ATTEMPTS: u32 = 2;
/// A part pending AICH recovery for longer than this is evicted wholesale
/// (guards against stalls when no AICH-capable source ever answers).
pub const AICH_PENDING_TIMEOUT: Duration = Duration::from_secs(120);
/// Re-request delay before an already-requested part may be asked again.
const AICH_REQUEST_COOLDOWN: Duration = Duration::from_secs(60);

/// Outcome of recording an AICH master-hash vote received from a peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AichVoteOutcome {
    /// The root is trusted (already known, or consensus reached and persisted).
    Trusted,
    /// Vote recorded but consensus not reached yet.
    Untrusted,
    /// Peer reported a different root than the trusted one — dead source.
    Mismatch,
}

/// Records an AICH master hash received from a peer for a download.
/// `masked_ip` must be `ip_u32 & 0x00F0FFFF` (eMule CAICHUntrustedHash).
/// When consensus is reached the root is persisted on the download record.
pub fn record_aich_root_vote(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
    root: [u8; 20],
    masked_ip: u32,
) -> AichVoteOutcome {
    let act = match active.get_mut(id) {
        Some(a) => a,
        None => return AichVoteOutcome::Mismatch,
    };

    if let Some(trusted) = &act.download.aich_root {
        return if trusted.0 == root {
            AichVoteOutcome::Trusted
        } else {
            AichVoteOutcome::Mismatch
        };
    }

    act.aich_votes.entry(root).or_default().insert(masked_ip);

    // Find the most-signed candidate (eMule UntrustedHashReceived).
    let total_ips: usize = act.aich_votes.values().map(|s| s.len()).sum();
    let (best_root, best_ips) = act
        .aich_votes
        .iter()
        .max_by_key(|(_, ips)| ips.len())
        .map(|(r, ips)| (*r, ips.len()))
        .unwrap_or((root, 0));

    if best_ips >= AICH_MIN_UNIQUE_IPS && (100 * best_ips) / total_ips.max(1) >= AICH_MIN_PERCENTAGE
    {
        act.download.aich_root = Some(mule_format::AichHash(best_root));
        let _ = db.update_aich_root(id, &best_root);
        tracing::info!(
            "[AICH] Master hash for '{}' trusted by consensus ({best_ips}/{total_ips} unique IPs)",
            act.download.display_name
        );
        AichVoteOutcome::Trusted
    } else {
        AichVoteOutcome::Untrusted
    }
}

/// Returns the trusted AICH root for a download, if any.
pub fn trusted_aich_root(active: &HashMap<Uuid, ActiveDownload>, id: &Uuid) -> Option<[u8; 20]> {
    active.get(id)?.download.aich_root.map(|a| a.0)
}

/// Pops the next corrupted part awaiting AICH recovery data. Parts whose
/// pending state has expired are evicted wholesale (fallback so downloads
/// cannot stall when no AICH-capable source ever answers).
pub fn next_aich_recovery(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
) -> Option<(u32, [u8; 20])> {
    let act = active.get_mut(id)?;
    let master = act.download.aich_root.map(|a| a.0)?;

    let mut deferred = std::collections::VecDeque::new();
    let mut result = None;
    while let Some((part, enqueued)) = act.aich_pending.pop_front() {
        if enqueued.elapsed() > AICH_PENDING_TIMEOUT {
            tracing::warn!(
                "[AICH] Recovery for part {part} of '{}' expired, evicting whole part",
                act.download.display_name
            );
            let start = part as u64 * ED2K_PART_SIZE_U64;
            let end = ((part as u64 + 1) * ED2K_PART_SIZE_U64).min(act.download.total_size);
            if end > start {
                let _ = act.ranges.remove(start..end);
                mark_part_corrupted(act, part, start, end);
            }
            let _ = db.unmark_part_verified(id, part);
            continue;
        }
        let recent = act
            .aich_requested
            .get(&part)
            .map(|&(ts, _)| ts.elapsed() < AICH_REQUEST_COOLDOWN)
            .unwrap_or(false);
        if recent {
            deferred.push_back((part, enqueued));
            continue;
        }
        let count = act.aich_requested.get(&part).map(|&(_, c)| c).unwrap_or(0);
        act.aich_requested.insert(part, (Instant::now(), count + 1));
        result = Some((part, master));
        break;
    }
    act.aich_pending.extend(deferred);
    let _ = db.sync_received_ranges(id, &act.ranges);
    result
}

/// Marks a pending AICH recovery request as failed (peer could not serve it):
/// requeues the part, or evicts it wholesale once attempts are exhausted.
pub fn fail_aich_recovery(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
    part: u32,
) {
    let Some(act) = active.get_mut(id) else {
        return;
    };
    let attempts = act.aich_requested.get(&part).map(|&(_, c)| c).unwrap_or(0);
    if attempts >= AICH_MAX_ATTEMPTS {
        tracing::warn!(
            "[AICH] Recovery for part {part} of '{}' failed {attempts} times, evicting whole part",
            act.download.display_name
        );
        evict_part_range(db, act, id, part);
    } else {
        act.aich_pending.push_back((part, Instant::now()));
    }
}

/// Report produced after applying received AICH recovery data.
#[derive(Debug)]
pub struct AichRecoveryReport {
    pub part: u32,
    pub corrupted_blocks: u32,
    pub evicted_ranges: Vec<(u64, u64)>,
}

fn evict_part_range(db: &mut Database, act: &mut ActiveDownload, id: &Uuid, part: u32) {
    let start = part as u64 * ED2K_PART_SIZE_U64;
    let end = ((part as u64 + 1) * ED2K_PART_SIZE_U64).min(act.download.total_size);
    if end > start {
        let _ = act.ranges.remove(start..end);
        mark_part_corrupted(act, part, start, end);
    }
    let _ = db.unmark_part_verified(id, part);
    let _ = db.sync_received_ranges(id, &act.ranges);
    if act.part_file.is_none() {
        act.part_file = PartFile::open_or_create(
            &act.download.temp_path,
            act.download.total_size,
            AllocationPolicy::None,
        )
        .ok();
    }
    act.download.status = DownloadStatus::Downloading;
    act.download.updated_at = now_ms();
    let _ = db.update_status(id, DownloadStatus::Downloading);
}

/// Applies received AICH recovery data for a corrupted part: verifies the hash
/// tree against the trusted master hash, then hashes each 180 KiB block of the
/// downloaded part and evicts only corrupted block ranges.
pub fn apply_aich_recovery(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
    part: u32,
    master_hash: [u8; 20],
    entries: Vec<mule_format::AichRecoveryEntry>,
) -> Result<AichRecoveryReport, DaemonError> {
    let act = active
        .get_mut(id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;

    let trusted = act.download.aich_root.map(|a| a.0);
    if trusted != Some(master_hash) {
        tracing::warn!(
            "[AICH] Recovery answer for part {part} of '{}' carries an untrusted master hash, ignoring",
            act.download.display_name
        );
        return Err(DaemonError::Business(
            "untrusted AICH master hash".to_string(),
        ));
    }

    let block_hashes = match mule_format::read_recovery_data(
        act.download.total_size,
        part,
        mule_format::AichHash(master_hash),
        &entries,
    ) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(
                "[AICH] Invalid recovery data for part {part} of '{}': {e}; evicting whole part",
                act.download.display_name
            );
            act.aich_requested.remove(&part);
            evict_part_range(db, act, id, part);
            return Ok(AichRecoveryReport {
                part,
                corrupted_blocks: u32::MAX,
                evicted_ranges: vec![],
            });
        }
    };

    // The part file may have been closed when the download entered Verifying;
    // reopen it so block hashes can be computed.
    if act.part_file.is_none() {
        act.part_file = PartFile::open_or_create(
            &act.download.temp_path,
            act.download.total_size,
            AllocationPolicy::None,
        )
        .ok();
    }

    let part_start = part as u64 * 9_728_000;
    let part_end = ((part as u64 + 1) * 9_728_000).min(act.download.total_size);
    let mut corrupted_blocks = 0u32;
    let mut evicted_ranges = Vec::new();

    for (i, expected) in block_hashes.iter().enumerate() {
        let block_start = part_start + i as u64 * 184_320;
        let block_end = (block_start + 184_320).min(part_end);
        if block_end <= block_start {
            continue;
        }
        let ok = act
            .part_file
            .as_mut()
            .and_then(|pf| {
                pf.read_range(block_start, (block_end - block_start) as usize)
                    .ok()
            })
            .map(|bytes| mule_format::aich_block_hash(&bytes) == *expected)
            .unwrap_or(false);
        if ok {
            // eMule `m_CorruptionBlackBox.VerifiedData` sur le bloc récupéré.
            act.corruption_blackbox
                .verified_data(part, block_start, block_end);
        } else {
            // eMule `CorruptedData` : attribution du bloc fautif à
            // l'expéditeur avant l'évaluation des bannissements.
            act.corruption_blackbox
                .corrupted_data(part, block_start, block_end);
            corrupted_blocks += 1;
            evicted_ranges.push((block_start, block_end));
            let _ = act.ranges.remove(block_start..block_end);
        }
    }
    // eMule `m_CorruptionBlackBox.EvaluateData` après le balayage des blocs.
    for guilty in act.corruption_blackbox.evaluate(part) {
        if !act.pending_corruption_bans.contains(&guilty) {
            tracing::warn!(
                "[CORRUPTION_BLACKBOX] '{}' : {guilty} a dépassé le seuil de corruption — bannissement demandé",
                act.download.display_name
            );
            act.pending_corruption_bans.push(guilty);
        }
    }

    act.aich_pending.retain(|&(p, _)| p != part);
    act.aich_requested.remove(&part);

    if corrupted_blocks == 0 {
        let _ = db.mark_part_verified(id, part);
        tracing::info!(
            "[AICH] Part {part} of '{}' fully verified by recovery data",
            act.download.display_name
        );
    } else {
        let _ = db.unmark_part_verified(id, part);
        if act.part_file.is_none() {
            act.part_file = PartFile::open_or_create(
                &act.download.temp_path,
                act.download.total_size,
                AllocationPolicy::None,
            )
            .ok();
        }
        if act.download.status == DownloadStatus::Verifying {
            act.download.status = DownloadStatus::Downloading;
            act.download.updated_at = now_ms();
            let _ = db.update_status(id, DownloadStatus::Downloading);
        }
        tracing::warn!(
            "[AICH] Part {part} of '{}': {corrupted_blocks} corrupted block(s) evicted for re-download",
            act.download.display_name
        );
    }

    db.sync_received_ranges(id, &act.ranges)
        .map_err(|e| DaemonError::Business(format!("DB sync error: {e}")))?;

    let _ = super::part_met::save_part_met(db, act);
    Ok(AichRecoveryReport {
        part,
        corrupted_blocks,
        evicted_ranges,
    })
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
        .map_err(db_err("persisting state"))?;

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

    // eMule `PerformFileComplete` : le fichier de destination n'est jamais
    // écrasé — collision résolue en `name(1).ext`, `name(2).ext`, …
    let final_path = mule_core::finalize_file(
        &temp_path,
        &target_path,
        mule_core::CollisionStrategy::Rename,
    )
    .map_err(|e| DaemonError::Business(format!("Finalize error: {e}")))?;

    // eMule deletes the .part.met companion once the file is complete.
    super::part_met::delete_part_met(&temp_path);

    let now = now_ms();
    {
        let act = active.get_mut(id).unwrap();
        act.download.status = DownloadStatus::Completed;
        act.download.target_path = final_path.clone();
        // eMule `m_fullname` suit le nom renommé ; le nom affiché reflète
        // le fichier réellement produit sur disque.
        if let Some(name) = final_path.file_name().and_then(|n| n.to_str()) {
            act.download.display_name = name.to_string();
        }
        act.verification_in_progress = false;
        act.download.updated_at = now;
        act.last_status_change_unix_ms = now;
        act.current_rate_bps = 0;
        let _ = act.part_file.take();
        // Le blackbox de corruption est libéré à la complétion du fichier.
        act.corruption_blackbox.clear();
    }
    db.update_status(id, DownloadStatus::Completed)
        .map_err(db_err("persisting state"))?;
    db.update_target_path(id, &final_path)
        .map_err(db_err("persisting final path"))?;

    let mut finalized_dl = dl;
    finalized_dl.status = DownloadStatus::Completed;
    finalized_dl.target_path = final_path.clone();
    if let Some(name) = final_path.file_name().and_then(|n| n.to_str()) {
        finalized_dl.display_name = name.to_string();
    }
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

    let mut shared = mule_core::model::SharedFile::new(
        finalized_dl.ed2k_hash.to_hex(),
        &finalized_dl.display_name,
        finalized_dl.total_size,
        final_path.to_string_lossy().as_ref(),
    );
    // eMule `UAPPref` : le fichier complété devient un partage « auto ».
    if peers_servers::auto_up_priority_enabled(db) {
        shared.up_priority = "auto".to_string();
    }
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
        .map_err(db_err("updating target path"))?;

    get_download_detail(db, active, id)
        .ok_or_else(|| DaemonError::NotFound(format!("Download {id} detail not found")))
}

/// Valide un nom de fichier fourni par l'utilisateur (`downloads.rename`,
/// `shares.rename`) : non vide, sans séparateurs ni caractères interdits
/// Windows, ni `.`/`..`, ni point/espace terminal (NTFS).
fn validate_new_file_name(name: &str) -> Result<&str, DaemonError> {
    let trimmed = name.trim();
    let illegal = |c: char| {
        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    };
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.ends_with(['.', ' '])
        || trimmed.chars().any(illegal)
    {
        return Err(DaemonError::InvalidParams(format!(
            "Invalid file name: '{name}'"
        )));
    }
    Ok(trimmed)
}

/// `downloads.rename` — renomme un téléchargement (nom affiché + chemin
/// de destination dérivé). Le `.part`/`.part.met` temporaire garde son
/// nom UUID ; pour un download terminé, le fichier final est renommé sur
/// disque et l'entrée `shared_files` suit. (eMule : renommage F2 sur la
/// liste des téléchargements.)
pub fn rename_download(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &Uuid,
    new_name: &str,
) -> Result<DownloadSnapshot, DaemonError> {
    let new_name = validate_new_file_name(new_name)?;

    let (old_target, new_target, completed, hash_hex) = {
        let act = active
            .get(id)
            .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;
        if act.download.display_name == new_name {
            return Ok(act.snapshot());
        }
        let parent = act
            .download
            .target_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        (
            act.download.target_path.clone(),
            parent.join(new_name),
            act.download.status == DownloadStatus::Completed,
            act.download.ed2k_hash.to_hex(),
        )
    };
    if active
        .values()
        .any(|o| o.download.id != *id && o.download.target_path == new_target)
    {
        return Err(DaemonError::Business(format!(
            "Another download already targets '{new_name}'"
        )));
    }

    // Download terminé : le fichier final existe sur disque → renommage
    // réel + mise à jour de l'entrée de partage (sinon elle pointerait
    // un chemin inexistant).
    if completed && old_target != new_target && old_target.exists() {
        if new_target.exists() {
            return Err(DaemonError::Business(format!(
                "A file named '{new_name}' already exists in the destination directory"
            )));
        }
        std::fs::rename(&old_target, &new_target)
            .map_err(|e| DaemonError::Business(format!("Cannot rename file on disk: {e}")))?;
        if let Ok(Some(mut shared)) = db.get_shared_file(&hash_hex) {
            shared.file_name = new_name.to_string();
            shared.file_path = new_target.to_string_lossy().to_string();
            let _ = db.upsert_shared_file(&shared);
        }
    }

    let act = active.get_mut(id).unwrap();
    act.download.display_name = new_name.to_string();
    act.download.target_path = new_target.clone();
    act.download.updated_at = now_ms();
    db.update_download_name_and_target(id, new_name, &new_target)
        .map_err(db_err("renaming download"))?;
    Ok(act.snapshot())
}

/// `shares.rename` — renomme un fichier partagé complet (disque +
/// `shared_files`). Quand le hash adosse un download encore listé, on
/// délègue à `rename_download` pour garder les deux vues cohérentes.
pub fn rename_shared_file(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    file_hash: &str,
    new_name: &str,
) -> Result<(), DaemonError> {
    let new_name = validate_new_file_name(new_name)?;

    if let Some(dl_id) = active
        .values()
        .find(|a| a.download.ed2k_hash.to_hex() == file_hash)
        .map(|a| a.download.id)
    {
        rename_download(db, active, &dl_id, new_name)?;
        return Ok(());
    }

    let mut shared = db
        .get_shared_file(file_hash)
        .map_err(db_err("persisting state"))?
        .ok_or_else(|| DaemonError::NotFound(format!("Shared file {file_hash} not found")))?;

    let old_path = PathBuf::from(&shared.file_path);
    let parent = old_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let new_path = parent.join(new_name);

    if old_path != new_path && old_path.exists() {
        if new_path.exists() {
            return Err(DaemonError::Business(format!(
                "A file named '{new_name}' already exists in the directory"
            )));
        }
        std::fs::rename(&old_path, &new_path)
            .map_err(|e| DaemonError::Business(format!("Cannot rename file on disk: {e}")))?;
    }

    shared.file_name = new_name.to_string();
    shared.file_path = new_path.to_string_lossy().to_string();
    db.upsert_shared_file(&shared)
        .map_err(db_err("persisting state"))?;
    Ok(())
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
