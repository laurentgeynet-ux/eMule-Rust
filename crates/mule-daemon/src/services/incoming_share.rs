// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Incoming-folder sharing (eMule `CSharedFileList` semantics).
//!
//! eMule shares every file sitting in its "Incoming" directory, including
//! nested subdirectories. We mirror that: at startup, each regular file
//! under the configured download directory (recursively) is hashed with
//! the eD2K algorithm (MD4 part hashes + root hash) and registered as a
//! shared file so remote peers can request it.
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

/// Résultat d'un scan du dossier Incoming : entrées nouvellement
/// enregistrées et entrées devenues orphelines purgées.
#[derive(Debug, Default, Clone, Copy)]
pub struct ShareScanReport {
    pub added: usize,
    pub removed: usize,
}

/// Scans `dir` recursively and registers every regular file as a shared
/// file (eMule shares subdirectories too).
///
/// Hashing runs on the blocking pool (eD2K hashing is I/O + MD4 bound).
/// Returns the number of newly registered files.
pub async fn scan_download_dir_shares(
    downloads: &Arc<Mutex<DownloadService>>,
    dir: PathBuf,
) -> ShareScanReport {
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
        return ShareScanReport::default();
    }

    // eMule shares subdirectories recursively — collect all shareable
    // files under `dir` on the blocking pool (arbitrary depth, I/O bound).
    let walk_dir = dir.clone();
    let entries: Vec<PathBuf> = match tokio::task::spawn_blocking(move || {
        collect_shareable_files(&walk_dir)
    })
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("[INCOMING_SHARE] directory walk failed: {e}");
            return ShareScanReport::default();
        }
    };

    // Réconciliation façon `CSharedFileList::Reload` : les partages dont
    // le fichier sous le dossier Incoming a disparu du disque
    // (suppression physique hors daemon) sont retirés. Garde
    // `dir.is_dir()` : si le dossier entier est injoignable (lecteur
    // débranché) chaque fichier semble manquant — ne rien purger.
    let mut removed = 0usize;
    if dir.is_dir() {
        removed = prune_stale_incoming_shares(downloads, &dir).await;
    } else {
        tracing::warn!(
            "[INCOMING_SHARE] {} unreachable — stale-share prune skipped",
            dir.display()
        );
    }

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

        let mut shared = mule_core::model::SharedFile::new(
            &hash_hex,
            &file_name,
            hashed.total_bytes,
            path.to_string_lossy().as_ref(),
        );
        // eMule `UAPPref` : un nouveau partage démarre en priorité « auto ».
        if dl.auto_up_priority_enabled() {
            shared.up_priority = "auto".to_string();
        }
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
    ShareScanReport { added, removed }
}

/// Retire les entrées `shared_files` pointant sous `dir` dont le fichier
/// n'existe plus sur le disque. Les partages hors `dir` sont ignorés :
/// un chemin injoignable y est indiscernable d'un lecteur hors-ligne.
/// Le test `exists()` par fichier (et non l'appartenance à la liste
/// scannée) immunise contre les sous-dossiers illisibles.
async fn prune_stale_incoming_shares(
    downloads: &Arc<Mutex<DownloadService>>,
    dir: &Path,
) -> usize {
    let dir_norm = normalize_path_str(dir);
    let mut dl = downloads.lock().await;
    let shares = match dl.list_shared_files() {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("[INCOMING_SHARE] cannot list shares for prune: {e}");
            return 0;
        }
    };
    let mut removed = 0usize;
    for sf in shares {
        if sf.file_path.is_empty() || !path_is_under_dir(&sf.file_path, &dir_norm) {
            continue;
        }
        if Path::new(&sf.file_path).exists() {
            continue;
        }
        match dl.remove_shared_file(&sf.file_hash, false) {
            Ok(true) => {
                removed += 1;
                tracing::info!(
                    "[INCOMING_SHARE] unshared {} — fichier supprimé du disque",
                    sf.file_name
                );
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(
                    "[INCOMING_SHARE] failed to drop stale share {}: {e}",
                    sf.file_name
                );
            }
        }
    }
    if removed > 0 {
        tracing::info!(
            "[INCOMING_SHARE] {removed} stale share(s) removed under {}",
            dir.display()
        );
    }
    removed
}

/// Normalisation lexicale pour comparaison de chemins Windows :
/// insensible à la casse, séparateurs `/` vs `\` unifiés.
fn normalize_path_str(p: &Path) -> String {
    p.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

fn path_is_under_dir(file_path: &str, dir_norm: &str) -> bool {
    let p = normalize_path_str(Path::new(file_path));
    p == dir_norm || p.starts_with(&format!("{dir_norm}/"))
}

/// Depth-first walk collecting every shareable file under `dir`.
/// Subdirectories are recursed into (eMule semantics); hidden entries and
/// symlinks are skipped — the latter prevents filesystem cycles.
fn collect_shareable_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let rd = match std::fs::read_dir(&d) {
            Ok(rd) => rd,
            Err(e) => {
                tracing::debug!("[INCOMING_SHARE] cannot scan {}: {e}", d.display());
                continue;
            }
        };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }
            let path = entry.path();
            if ft.is_dir() {
                let hidden = entry
                    .file_name()
                    .to_str()
                    .map(|n| n.starts_with('.'))
                    .unwrap_or(true);
                if !hidden {
                    stack.push(path);
                }
                continue;
            }
            if is_shareable_file(&path) {
                out.push(path);
            }
        }
    }
    out
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
