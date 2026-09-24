// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! `known.met` import — migration helper applying eMule's shared-file
//! index (`CKnownFileList::LoadKnownFiles`) to our `shared_files` table.
//!
//! `known.met` stores no filesystem path, so records are joined to
//! files found under a directory by `(file_name, file_size)` — the only
//! key both sides share. Matched files are shared under eMule's hash;
//! part hashes, AICH root, upload priority and transferred statistics
//! are imported as `CKnownFile::LoadTagsFromFile` exposes them.
//!
//! `verify=false` trusts eMule's hash records (the migration fast path
//! — re-hashing the whole incoming directory is what the tool exists
//! to avoid); `verify=true` re-hashes each candidate and only imports
//! on root-hash equality, eMule's own behaviour at share time
//! (`CAddFileThread::Run` always rehashes).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::Mutex;

use crate::error::DaemonError;
use crate::services::downloads::DownloadService;
use crate::services::incoming_share::collect_shareable_files;

/// Outcome of one `import_known_met` run.
#[derive(Debug, Default, Serialize)]
pub struct KnownMetImportReport {
    /// Valid records read from the file (post-verification).
    pub records: usize,
    /// Records eMule would drop (failed hash-set verification).
    pub dropped_records: usize,
    /// Shareable files found under the scanned directory.
    pub scanned_files: usize,
    /// New shares registered from matching records.
    pub imported: usize,
    /// Already-shared hashes enriched (aich_root / stats only).
    pub enriched: usize,
    /// `verify=true` candidates whose real hash differs from the record.
    pub mismatched: usize,
    /// Records that matched no file on disk.
    pub unmatched_records: usize,
    /// Directory that was scanned.
    pub dir: String,
}

/// eMule `PR_*` → `SharedFile::up_priority` strings.
fn pr_to_up_priority(pr: u8) -> &'static str {
    match pr {
        mule_format::PR_VERYLOW => "verylow",
        mule_format::PR_LOW => "low",
        mule_format::PR_HIGH => "high",
        mule_format::PR_VERYHIGH => "veryhigh",
        _ => "normal",
    }
}

/// Imports `met_path` (eMule `known.met`) for shareable files under `dir`.
pub async fn import_known_met(
    downloads: &Arc<Mutex<DownloadService>>,
    met_path: PathBuf,
    dir: PathBuf,
    verify: bool,
) -> Result<KnownMetImportReport, DaemonError> {
    let met = tokio::task::spawn_blocking(move || mule_format::KnownMetFile::open(&met_path))
        .await
        .map_err(|e| DaemonError::Internal(format!("known.met read task failed: {e}")))?
        .map_err(|e| DaemonError::Business(format!("failed to parse known.met: {e}")))?;

    let walk_dir = dir.clone();
    let files = tokio::task::spawn_blocking(move || collect_shareable_files(&walk_dir))
        .await
        .map_err(|e| DaemonError::Internal(format!("directory walk failed: {e}")))?;

    // (lowercase name, size) → record — the only join key available.
    let mut by_name_size: HashMap<(String, u64), usize> = HashMap::new();
    for (i, rec) in met.records.iter().enumerate() {
        if let (Some(name), Some(size)) = (&rec.file_name, rec.file_size) {
            by_name_size.entry((name.to_lowercase(), size)).or_insert(i);
        }
    }
    let mut matched_records: HashSet<usize> = HashSet::new();

    let mut report = KnownMetImportReport {
        records: met.records.len(),
        dropped_records: met.dropped_records,
        scanned_files: files.len(),
        dir: dir.to_string_lossy().to_string(),
        ..Default::default()
    };

    for path in files {
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
            continue;
        };
        let size = match std::fs::metadata(&path) {
            Ok(m) => m.len(),
            Err(_) => continue,
        };
        let Some(&rec_idx) = by_name_size.get(&(file_name.to_lowercase(), size)) else {
            continue;
        };
        let rec = &met.records[rec_idx];
        let hash_hex = rec.file_hash.to_hex().to_lowercase();

        if verify {
            let path_for_hash = path.clone();
            let hashed = tokio::task::spawn_blocking(move || {
                let mut f = std::fs::File::open(&path_for_hash)?;
                mule_format::ed2k::hash_reader(&mut f)
            })
            .await;
            match hashed {
                Ok(Ok(h)) if h.root_hash.as_bytes() == rec.file_hash.as_bytes() => {}
                _ => {
                    report.mismatched += 1;
                    tracing::warn!(
                        "[KNOWNMET] hash mismatch for {} — record skipped",
                        path.display()
                    );
                    continue;
                }
            }
        }
        matched_records.insert(rec_idx);

        let mut dl = downloads.lock().await;
        if let Some(mut sf) = dl.get_shared_file(&hash_hex)? {
            // Existing share: enrich only — never reset user config.
            if sf.aich_root.is_none() {
                sf.aich_root = rec.aich_root.map(|a| a.0);
                dl.add_shared_file(&sf)?;
            }
            if rec.transferred_bytes > 0 {
                dl.add_share_uploaded_bytes(&hash_hex, rec.transferred_bytes)?;
            }
            report.enriched += 1;
            continue;
        }

        let mut sf = mule_core::model::SharedFile::new(
            &hash_hex,
            &file_name,
            size,
            path.to_string_lossy().as_ref(),
        );
        sf.aich_root = rec.aich_root.map(|a| a.0);
        if let Some(pr) = rec.up_priority {
            sf.up_priority = pr_to_up_priority(pr).to_string();
        } else if dl.auto_up_priority_enabled() {
            sf.up_priority = "auto".to_string();
        }
        dl.add_shared_file(&sf)?;
        if !rec.part_hashes.is_empty() {
            let parts: Vec<[u8; 16]> = rec.part_hashes.iter().map(|p| *p.as_bytes()).collect();
            let _ = dl.store_part_hashes_by_hash(&hash_hex, &parts);
        }
        if rec.transferred_bytes > 0 {
            dl.add_share_uploaded_bytes(&hash_hex, rec.transferred_bytes)?;
        }
        report.imported += 1;
        tracing::info!(
            "[KNOWNMET] imported {} ({}) as {hash_hex}",
            file_name,
            size
        );
    }

    report.unmatched_records = met.records.len().saturating_sub(matched_records.len());
    Ok(report)
}
