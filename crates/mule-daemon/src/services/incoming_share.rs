// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Incoming-folder sharing (eMule `CSharedFileList` semantics).
//!
//! eMule shares every file sitting in its "Incoming" directory. We mirror
//! that: at startup, each regular file in the configured download directory
//! is hashed with the eD2K algorithm (MD4 part hashes + root hash) and
//! registered as a shared file so remote peers can request it.
//!
//! Disabled via the `share_download_dir` setting (`"0"`). Files that are
//! already shared keep their `enabled` flag and upload priority — the scan
//! only *adds* missing entries, it never resets user configuration.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::services::downloads::DownloadService;

/// Setting key (persisted in the settings table): `"0"` disables the scan.
pub const SETTING_SHARE_DOWNLOAD_DIR: &str = "share_download_dir";

/// File extensions skipped during the incoming-folder scan (download
/// artifacts, never shareable payloads).
const SKIP_EXTENSIONS: &[&str] = &["part", "met", "part.met", "bak", "tmp", "ts", "emuletd"];

/// Scans `dir` once and registers every regular file as a shared file.
///
/// Hashing runs on the blocking pool (eD2K hashing is I/O + MD4 bound).
/// Returns the number of newly registered files.
pub async fn scan_download_dir_shares(
    downloads: &Arc<Mutex<DownloadService>>,
    dir: PathBuf,
) -> usize {
    // Opt-out setting; defaults to enabled like eMule's Incoming folder.
    let enabled = {
        let dl = downloads.lock().await;
        dl.get_setting(SETTING_SHARE_DOWNLOAD_DIR)
            .ok()
            .flatten()
            .map(|v| !matches!(v.trim().to_lowercase().as_str(), "0" | "false" | "off"))
            .unwrap_or(true)
    };
    if !enabled {
        tracing::info!("[INCOMING_SHARE] download-dir sharing disabled by setting");
        return 0;
    }

    let entries: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| is_shareable_file(p))
            .collect(),
        Err(e) => {
            tracing::debug!("[INCOMING_SHARE] cannot scan {}: {e}", dir.display());
            return 0;
        }
    };

    let mut added = 0usize;
    for path in entries {
        let Some(file_name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
        else {
            continue;
        };
        let path_for_hash = path.clone();
        let hashed = match tokio::task::spawn_blocking(move || {
            let mut f = std::fs::File::open(&path_for_hash)?;
            mule_format::ed2k::hash_reader(&mut f)
        })
        .await
        {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => {
                tracing::warn!(
                    "[INCOMING_SHARE] hashing failed for {}: {e}",
                    path.display()
                );
                continue;
            }
            Err(e) => {
                tracing::warn!("[INCOMING_SHARE] hash task failed: {e}");
                continue;
            }
        };

        let hash_hex = hashed.root_hash.to_hex().to_lowercase();
        let mut dl = downloads.lock().await;

        // Already shared (completed download or manual share): keep the
        // existing entry and its user settings untouched.
        if dl.get_shared_file(&hash_hex).ok().flatten().is_some() {
            continue;
        }

        let shared = mule_core::model::SharedFile::new(
            &hash_hex,
            &file_name,
            hashed.total_bytes,
            path.to_string_lossy().as_ref(),
        );
        if let Err(e) = dl.add_shared_file(&shared) {
            tracing::warn!(
                "[INCOMING_SHARE] failed to register {}: {e}",
                path.display()
            );
            continue;
        }
        if !hashed.part_hashes.is_empty() {
            let _ = dl.store_part_hashes_by_hash(&hash_hex, &hashed.part_hashes);
        }
        added += 1;
        tracing::info!(
            "[INCOMING_SHARE] sharing {} ({}) as {hash_hex}",
            file_name,
            hashed.total_bytes
        );
    }

    if added > 0 {
        tracing::info!(
            "[INCOMING_SHARE] {added} file(s) auto-shared from {}",
            dir.display()
        );
    }
    added
}

/// A file is shareable when it is a regular, non-hidden file whose
/// extension is not a download artifact.
fn is_shareable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    if name.starts_with('.') {
        return false;
    }
    if path.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return false;
    }
    let lower = name.to_lowercase();
    !SKIP_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}
