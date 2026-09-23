// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use mule_core::{AllocationPolicy, Database, Download, DownloadStatus, PartFile, RangeSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::types::{
    make_snapshot, now_ms, ActiveDownload, ImportPartMetResult, PartMetInspection,
    MAX_IMPORT_PART_COUNT, MAX_IMPORT_TOTAL_SIZE,
};
use crate::error::DaemonError;

pub fn inspect_part_met(
    met_path: &Path,
    part_path: Option<&Path>,
) -> Result<PartMetInspection, DaemonError> {
    if !met_path.exists() {
        return Err(DaemonError::NotFound(format!(
            ".part.met file not found: {}",
            met_path.display()
        )));
    }

    let met = mule_format::PartMetFile::open(met_path)
        .map_err(|e| DaemonError::Business(format!("Invalid .part.met file: {e}")))?;

    let file_name = met.file_name.clone().unwrap_or_else(|| {
        met_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown_file")
            .to_string()
    });

    let total_size = met.file_size.unwrap_or(0);
    let part_count = met.part_count() as u32;

    let resolved_part_path = if let Some(p) = part_path {
        p.to_path_buf()
    } else {
        let met_name = met_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(prefix) = met_name.strip_suffix(".part.met") {
            let part_name = format!("{prefix}.part");
            met_path.with_file_name(part_name)
        } else if let Some(prefix) = met_name.strip_suffix(".met") {
            let part_name = format!("{prefix}.part");
            met_path.with_file_name(part_name)
        } else {
            met_path.with_extension("part")
        }
    };

    let part_file_exists = resolved_part_path.exists();
    let part_file_size = if part_file_exists {
        std::fs::metadata(&resolved_part_path).ok().map(|m| m.len())
    } else {
        None
    };

    let estimated_transferred_bytes = met
        .compute_completed_ranges()
        .map(|ranges| ranges.iter().map(|(s, e)| e - s).sum())
        .unwrap_or(met.transferred);

    Ok(PartMetInspection {
        file_name,
        total_size,
        ed2k_hash: met.file_hash.to_hex(),
        aich_hash: met.aich_hash.map(|a| a.to_base32()),
        part_count,
        part_met_path: met_path.display().to_string(),
        part_path: Some(resolved_part_path.display().to_string()),
        part_file_exists,
        part_file_size,
        estimated_transferred_bytes,
    })
}

/// Returns the `.part.met` companion path for a given `.part` temp path
/// (`{id}.part` → `{id}.part.met`).
pub fn part_met_path_for(temp_path: &Path) -> PathBuf {
    let mut name = temp_path.as_os_str().to_os_string();
    name.push(".met");
    PathBuf::from(name)
}

/// Writes the `.part.met` companion file for an active download, mirroring
/// eMule `CPartFile::SavePartFile`: version header, file hash + part hashset,
/// filename/size/transferred/AICH tags, and missing ranges as gap tags.
/// Best-effort persistence for crash-resume and eMule import compatibility.
pub fn save_part_met(db: &Database, act: &ActiveDownload) -> Result<PathBuf, DaemonError> {
    let temp_path = &act.download.temp_path;
    let met_path = part_met_path_for(temp_path);

    let mod_date = std::fs::metadata(temp_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);

    let hash_hex = act.download.ed2k_hash.to_hex().to_lowercase();
    let part_hashes = db.get_part_hashes(&hash_hex).unwrap_or_default();

    let gaps: Vec<(u64, u64)> = act
        .ranges
        .missing_within(0..act.download.total_size)
        .into_iter()
        .map(|r| (r.start, r.end))
        .collect();

    let met = mule_format::PartMetFile {
        version: mule_format::PARTFILE_VERSION,
        mod_date,
        file_hash: act.download.ed2k_hash,
        part_hashes,
        file_name: Some(act.download.display_name.clone()),
        file_size: Some(act.download.total_size),
        transferred: act.ranges.covered_len(),
        aich_hash: act.download.aich_root,
        gaps,
        // Persist the per-file source limit as eMule FT_MAXSOURCES so a
        // re-imported .part.met (or eMule itself) restores the cap.
        tags: act
            .download
            .max_sources
            .map(|ms| {
                vec![mule_format::met::tags::Tag::new_id(
                    mule_format::met::tags::FT_MAXSOURCES,
                    mule_format::met::tags::TagValue::Uint64(ms as u64),
                )]
            })
            .unwrap_or_default(),
    };

    met.save(&met_path)
        .map_err(|e| DaemonError::Business(format!("Failed to save .part.met: {e}")))?;
    Ok(met_path)
}

/// Removes the `.part.met` companion file for a temp path, if present.
/// Called after successful finalization (eMule deletes it on completion).
pub fn delete_part_met(temp_path: &Path) {
    let _ = std::fs::remove_file(part_met_path_for(temp_path));
}

/// `adopt_source`: when true the source `.part` is renamed (moved) into the
/// staged `{uuid}.part` path instead of copied — only safe when the source
/// already lives inside `temp_dir` (startup orphan recovery), where a move
/// is atomic and avoids duplicating gigabytes. External imports always copy.
#[allow(clippy::too_many_arguments)]
pub fn import_part_met(
    db: &mut Database,
    temp_dir: &Path,
    active_map: &std::collections::HashMap<Uuid, ActiveDownload>,
    met_path: &Path,
    part_path: Option<&Path>,
    target_dir: &Path,
    category_id: Option<String>,
    adopt_source: bool,
) -> Result<(ImportPartMetResult, ActiveDownload), DaemonError> {
    let inspection = inspect_part_met(met_path, part_path)?;
    if !inspection.part_file_exists {
        return Err(DaemonError::NotFound(format!(
            "Matching .part file not found for {}",
            met_path.display()
        )));
    }

    let met = mule_format::PartMetFile::open(met_path)
        .map_err(|e| DaemonError::Business(format!("Invalid .part.met file: {e}")))?;

    let ed2k_hex = met.file_hash.to_hex().to_lowercase();
    if let Some(existing) = active_map
        .values()
        .find(|a| a.download.ed2k_hash.to_hex().to_lowercase() == ed2k_hex)
    {
        return Err(DaemonError::Business(format!(
            "Ce fichier .part.met est déjà importé ('{}', statut: {:?})",
            existing.download.display_name, existing.download.status
        )));
    }

    let total_size = met
        .file_size
        .ok_or_else(|| DaemonError::Business("File size missing in .part.met file".into()))?;

    if total_size > MAX_IMPORT_TOTAL_SIZE {
        let max = MAX_IMPORT_TOTAL_SIZE;
        return Err(DaemonError::Business(format!(
            "File size ({total_size} bytes) exceeds maximum import safety limit of {max} bytes"
        )));
    }

    let total_parts = met.part_count() as u32;
    if total_parts > MAX_IMPORT_PART_COUNT {
        let max = MAX_IMPORT_PART_COUNT;
        return Err(DaemonError::Business(format!(
            "Part count ({total_parts}) exceeds maximum import safety limit of {max} parts"
        )));
    }

    let actual_part_path = PathBuf::from(inspection.part_path.as_ref().unwrap());
    if !actual_part_path.exists() {
        return Err(DaemonError::NotFound(format!(
            "Associated .part file not found: {}",
            actual_part_path.display()
        )));
    }

    let metadata = std::fs::metadata(&actual_part_path)
        .map_err(|e| DaemonError::Business(format!("Failed to read .part metadata: {e}")))?;
    if metadata.len() > total_size {
        return Err(DaemonError::Business(format!(
            "Physical .part file size ({} bytes) exceeds declared file size ({} bytes)",
            metadata.len(),
            total_size
        )));
    }

    let completed_ranges = met
        .compute_completed_ranges()
        .map_err(|e| DaemonError::Business(format!("Failed to compute completed ranges: {e}")))?;

    let mut range_set = RangeSet::new();
    for (s, e) in completed_ranges {
        let _ = range_set.insert(s..e);
    }

    let id = Uuid::new_v4();
    let internal_temp_filename = format!("{id}.part");
    let internal_temp_path = temp_dir.join(internal_temp_filename);
    let target_path = target_dir.join(&inspection.file_name);

    if adopt_source {
        // Orphan already inside our own temp dir: adopt in place via rename
        // (atomic move), falling back to copy+delete on exotic filesystems.
        if std::fs::rename(&actual_part_path, &internal_temp_path).is_err() {
            std::fs::copy(&actual_part_path, &internal_temp_path).and_then(|_| {
                std::fs::remove_file(&actual_part_path)
            }).map_err(|e| {
                DaemonError::Business(format!(
                    "Failed to adopt orphan .part file into sandbox: {e}"
                ))
            })?;
        }
    } else {
        std::fs::copy(&actual_part_path, &internal_temp_path).map_err(|e| {
            DaemonError::Business(format!("Failed to stage .part file into sandbox: {e}"))
        })?;
    }

    let mut part_file =
        PartFile::open_or_create(&internal_temp_path, total_size, AllocationPolicy::None)
            .map_err(|e| DaemonError::Business(format!("Failed to open staged .part file: {e}")))?;

    let part_size_const = mule_format::ed2k::hash::ED2K_PART_SIZE as u64;
    let mut verified_parts = 0u32;
    let mut corrupted_parts = 0u32;
    let mut verified_indices = Vec::new();

    for i in 0..total_parts {
        let start = i as u64 * part_size_const;
        let end = ((i as u64 + 1) * part_size_const).min(total_size);
        let part_len = (end - start) as usize;

        if range_set.contains(start..end) {
            if let Some(expected_hash) = met.part_hashes.get(i as usize) {
                match part_file.read_range(start, part_len) {
                    Ok(bytes) => {
                        if mule_core::verify_part_hash(&bytes, expected_hash) {
                            verified_parts += 1;
                            verified_indices.push(i);
                        } else {
                            corrupted_parts += 1;
                            let _ = range_set.remove(start..end);
                        }
                    }
                    Err(_) => {
                        corrupted_parts += 1;
                        let _ = range_set.remove(start..end);
                    }
                }
            } else if total_parts == 1 {
                match part_file.read_range(start, part_len) {
                    Ok(bytes) => {
                        if mule_core::verify_part_hash(&bytes, &met.file_hash.0) {
                            verified_parts += 1;
                            verified_indices.push(i);
                        } else {
                            corrupted_parts += 1;
                            let _ = range_set.remove(start..end);
                        }
                    }
                    Err(_) => {
                        corrupted_parts += 1;
                        let _ = range_set.remove(start..end);
                    }
                }
            }
        }
    }

    // eMule per-file source limit tag (FT_MAXSOURCES / CPartFile::GetMaxSources).
    let max_sources = met
        .tags
        .iter()
        .find(|t| t.name_id == mule_format::met::tags::FT_MAXSOURCES)
        .and_then(|t| t.as_u64())
        .and_then(|v| u32::try_from(v).ok())
        .filter(|&v| v > 0);

    let now = now_ms();
    let download = Download {
        id,
        display_name: inspection.file_name.clone(),
        total_size,
        ed2k_hash: met.file_hash,
        aich_root: met.aich_hash,
        target_path,
        temp_path: internal_temp_path.clone(),
        status: DownloadStatus::Paused,
        created_at: now,
        updated_at: now,
        category_id: category_id.clone(),
        priority: "normal".to_string(),
        max_sources,
    };

    db.insert_download(&download)
        .map_err(|e| DaemonError::Business(format!("DB error: {e}")))?;

    db.sync_received_ranges(&id, &range_set)
        .map_err(|e| DaemonError::Business(format!("Failed to sync ranges: {e}")))?;

    if !met.part_hashes.is_empty() {
        let _ = db.store_part_hashes(&ed2k_hex, &met.part_hashes);
    }

    for idx in verified_indices {
        let _ = db.mark_part_verified(&id, idx);
    }

    let snapshot = make_snapshot(&download, &range_set, 0, 0);

    let active_download = ActiveDownload {
        download,
        ranges: range_set,
        part_file: Some(part_file),
        sources: std::collections::HashMap::new(),
        connection_attempts: 0,
        resume_count: 0,
        last_error: None,
        last_status_change_unix_ms: now,
        session_start_ms: now,
        session_bytes_start: snapshot.received_bytes,
        current_rate_bps: 0,
        last_rate_update: None,
        source_count: 0,
        verification_in_progress: false,
        aich_pending: Default::default(),
        aich_requested: Default::default(),
        aich_votes: Default::default(),
    };

    let met_version_str = match met.version {
        mule_format::PARTFILE_VERSION => "0xE0 (Standard eMule)",
        mule_format::PARTFILE_SPLITTEDVERSION => "0xE1 (Splitted eMule)",
        mule_format::PARTFILE_VERSION_LARGEFILE => "0xE2 (Large Files 64-bit)",
        _ => "Unknown/Custom",
    };

    let report = format!(
        "=== EMule-Rust .part.met Import Audit Report ===\n\
         Format Version: {met_version_str}\n\
         File Name: {}\n\
         Declared Total Size: {total_size} bytes\n\
         Announced Parts: {total_parts}\n\
         Verified Valid Parts: {verified_parts}\n\
         Rejected/Corrupted Parts: {corrupted_parts}\n\
         Rejection Detail: {}\n\
         Network Status: STRICTLY PAUSED (sourceCount: 0, zero network activity)\n\
         Confinement: Isolated under data/temp/{id}.part (Source file untouched)",
        inspection.file_name,
        if corrupted_parts > 0 {
            "Checksum mismatch against declared part hashes (evicted to gaps)"
        } else {
            "None (All complete parts passed integrity check)"
        }
    );

    Ok((
        ImportPartMetResult {
            download: snapshot.clone(),
            total_parts,
            verified_parts,
            corrupted_parts,
            transferred_bytes: snapshot.received_bytes,
            total_size,
            report: Some(report),
        },
        active_download,
    ))
}
