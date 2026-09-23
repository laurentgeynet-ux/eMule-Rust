// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use mule_core::{AllocationPolicy, Database, DownloadCategory, DownloadStatus, PartFile};
use mule_protocol::ed2k::PartAvailabilityMask;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::error::DaemonError;
use crate::services::DaemonEvent;

pub mod history_categories;
pub mod operations;
pub mod part_met;
pub mod peers_servers;
pub mod types;

pub use part_met::{import_part_met, inspect_part_met};
pub use types::{
    make_snapshot, now_ms, ActiveDownload, ActiveSource, DownloadDetail, DownloadSnapshot,
    ImportPartMetResult, PartMetInspection, PartProgressSummary, SourceAvailabilitySummary,
    MAX_IMPORT_PART_COUNT, MAX_IMPORT_TOTAL_SIZE,
};

pub struct DownloadService {
    db: Database,
    temp_dir: PathBuf,
    active: HashMap<Uuid, ActiveDownload>,
    session_start_ms: i64,
    /// Event channel installed by `AppState` for lifecycle events
    /// (`event.downloadAdded/Removed/Completed/Failed`).
    event_tx: Option<tokio::sync::broadcast::Sender<String>>,
    sequence: Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
}

impl DownloadService {
    pub fn new(db_path: &Path, temp_dir: PathBuf) -> Result<Self, DaemonError> {
        let mut db = Database::open(db_path)?;
        std::fs::create_dir_all(&temp_dir)?;

        let session_start_ms = now_ms();
        let existing = db.get_all_downloads().unwrap_or_default();
        let mut active = HashMap::new();
        for mut dl in existing {
            let ranges = db.load_received_ranges(&dl.id).unwrap_or_default();
            let covered = ranges.covered_len();

            // Self-healing
            let is_finalized_on_disk = dl.target_path.exists()
                && std::fs::metadata(&dl.target_path)
                    .map(|m| m.len())
                    .unwrap_or(0)
                    == dl.total_size;

            if (dl.status == DownloadStatus::Verifying || dl.status == DownloadStatus::Downloading)
                && is_finalized_on_disk
            {
                tracing::info!(
                    "Healing download {} ('{}') from {:?} to Completed (target file exists on disk)",
                    dl.id,
                    dl.display_name,
                    dl.status
                );
                dl.status = DownloadStatus::Completed;
                let _ = db.update_status(&dl.id, DownloadStatus::Completed);
            }

            let part_file = if dl.status == DownloadStatus::Completed {
                None
            } else {
                PartFile::open_or_create(&dl.temp_path, dl.total_size, AllocationPolicy::None).ok()
            };

            let source_count = db
                .list_peer_sources(&dl.ed2k_hash.to_hex())
                .map(|s| s.iter().filter(|x| x.status.is_srclist_visible()).count() as u32)
                .unwrap_or(0);
            active.insert(
                dl.id,
                ActiveDownload {
                    last_status_change_unix_ms: dl.updated_at,
                    session_start_ms,
                    session_bytes_start: covered,
                    connection_attempts: 0,
                    resume_count: 0,
                    last_error: None,
                    download: dl,
                    ranges,
                    part_file,
                    sources: HashMap::new(),
                    current_rate_bps: 0,
                    last_rate_update: None,
                    source_count,
                    verification_in_progress: false,
                    aich_pending: Default::default(),
                    aich_requested: Default::default(),
                    aich_votes: Default::default(),
                },
            );
        }

        // Apply retention policy on startup
        let retention_days = db
            .get_setting("history_retention_days")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(30);
        let _ = db.apply_retention_policy(retention_days);

        Ok(Self {
            db,
            temp_dir,
            active,
            session_start_ms,
            event_tx: None,
            sequence: None,
        })
    }

    pub fn in_memory(temp_dir: PathBuf) -> Result<Self, DaemonError> {
        let db = Database::open_in_memory()?;
        std::fs::create_dir_all(&temp_dir)?;

        Ok(Self {
            db,
            temp_dir,
            active: HashMap::new(),
            session_start_ms: now_ms(),
            event_tx: None,
            sequence: None,
        })
    }

    /// Installs the broadcast channel used to emit download lifecycle
    /// events. Called once by `AppState::new` before the service is shared.
    pub fn set_event_channel(
        &mut self,
        tx: tokio::sync::broadcast::Sender<String>,
        seq: std::sync::Arc<std::sync::atomic::AtomicU64>,
    ) {
        self.event_tx = Some(tx);
        self.sequence = Some(seq);
    }

    fn emit(&self, make: impl FnOnce(u64) -> DaemonEvent) {
        let (Some(tx), Some(seq)) = (&self.event_tx, &self.sequence) else {
            return;
        };
        make(seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst)).send(tx);
    }

    /// Effective temp directory: persisted `temp_dir` setting wins over the
    /// construction-time directory (eMule "Répertoire temporaire").
    fn effective_temp_dir(&self) -> PathBuf {
        self.db
            .get_setting("temp_dir")
            .ok()
            .flatten()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.temp_dir.clone())
    }

    pub fn add_ed2k_link(
        &mut self,
        link_str: &str,
        target_dir: &Path,
        policy: AllocationPolicy,
        category_id: Option<String>,
    ) -> Result<DownloadSnapshot, DaemonError> {
        let temp_dir = self.effective_temp_dir();
        std::fs::create_dir_all(&temp_dir)?;
        let snapshot = operations::add_ed2k_link(
            &mut self.db,
            &temp_dir,
            &mut self.active,
            link_str,
            target_dir,
            policy,
            category_id,
        )?;
        self.emit(|sequence| DaemonEvent::DownloadAdded {
            sequence,
            download: snapshot.clone(),
        });
        self.emit(|sequence| DaemonEvent::HistoryEntryAdded { sequence });
        Ok(snapshot)
    }

    pub fn inspect_part_met(
        &self,
        met_path: &Path,
        part_path: Option<&Path>,
    ) -> Result<PartMetInspection, DaemonError> {
        part_met::inspect_part_met(met_path, part_path)
    }

    pub fn import_part_met(
        &mut self,
        met_path: &Path,
        part_path: Option<&Path>,
        target_dir: &Path,
        category_id: Option<String>,
    ) -> Result<ImportPartMetResult, DaemonError> {
        let temp_dir = self.effective_temp_dir();
        std::fs::create_dir_all(&temp_dir)?;
        let (result, active_dl) = part_met::import_part_met(
            &mut self.db,
            &temp_dir,
            &self.active,
            met_path,
            part_path,
            target_dir,
            category_id,
            false,
        )?;

        let id = active_dl.download.id;
        let verif_status = if result.corrupted_parts > 0 {
            "partial_corruption"
        } else if result.verified_parts == result.total_parts && result.total_parts > 0 {
            "verified_ok"
        } else {
            "unverified"
        };
        history_categories::record_history_event(
            &mut self.db,
            &active_dl.download,
            active_dl.ranges.covered_len(),
            0,
            0,
            "imported",
            verif_status,
            None,
        );

        self.active.insert(id, active_dl);
        Ok(result)
    }

    /// Recovers orphaned `.part.met` files found in the effective temp
    /// directory at startup (eMule `CPartFile` auto-load semantics): any met
    /// whose file hash is not already tracked is imported through the same
    /// sandboxed path as `downloads.importPartMet` (part-hash verification,
    /// staged `{uuid}.part` copy, Paused status). Returns the number of
    /// recovered downloads.
    pub fn recover_orphan_part_mets(&mut self, target_dir: &Path) -> usize {
        let temp_dir = self.effective_temp_dir();
        let entries = match std::fs::read_dir(&temp_dir) {
            Ok(e) => e,
            Err(_) => return 0,
        };

        let mut recovered = 0usize;
        for entry in entries.flatten() {
            let met_path = entry.path();
            let is_met = met_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".part.met"))
                .unwrap_or(false);
            if !is_met {
                continue;
            }

            // Skip companions already owned by an active download
            // (`{uuid}.part.met` written by save_part_met).
            let already_tracked = match mule_format::PartMetFile::open(&met_path) {
                Ok(met) => self.active.values().any(|a| {
                    a.download
                        .ed2k_hash
                        .to_hex()
                        .eq_ignore_ascii_case(&met.file_hash.to_hex())
                }),
                Err(_) => false,
            };
            if already_tracked {
                continue;
            }

            match part_met::import_part_met(
                &mut self.db,
                &temp_dir,
                &self.active,
                &met_path,
                None,
                target_dir,
                None,
                true,
            ) {
                Ok((result, active_dl)) => {
                    // The orphan was adopted in place ({stem}.part renamed to
                    // {uuid}.part): drop the source met so it is not
                    // reprocessed on the next startup.
                    let _ = std::fs::remove_file(&met_path);
                    let verif_status = if result.corrupted_parts > 0 {
                        "partial_corruption"
                    } else if result.verified_parts == result.total_parts && result.total_parts > 0
                    {
                        "verified_ok"
                    } else {
                        "unverified"
                    };
                    history_categories::record_history_event(
                        &mut self.db,
                        &active_dl.download,
                        active_dl.ranges.covered_len(),
                        0,
                        0,
                        "imported",
                        verif_status,
                        None,
                    );
                    tracing::info!(
                        "Recovered orphaned .part.met '{}' ({}/{} parts verified) as paused download {}",
                        result.download.file_name,
                        result.verified_parts,
                        result.total_parts,
                        active_dl.download.id
                    );
                    self.active.insert(active_dl.download.id, active_dl);
                    recovered += 1;
                }
                Err(e) => {
                    tracing::warn!("Skipping orphaned .part.met {}: {e}", met_path.display());
                }
            }
        }
        recovered
    }

    /// Persists `.part.met` companions for every non-terminal download —
    /// eMule `SavePartFile` cadence, invoked periodically by the swarm
    /// coordinator heartbeat and once on graceful shutdown so a crash never
    /// leaves gap state staler than the last flush.
    pub fn save_active_part_mets(&self) {
        for act in self.active.values() {
            if matches!(
                act.download.status,
                DownloadStatus::Completed | DownloadStatus::Removed
            ) {
                continue;
            }
            let _ = part_met::save_part_met(&self.db, act);
        }
    }

    pub fn list_downloads(&self) -> Vec<DownloadSnapshot> {
        self.active.values().map(|a| a.snapshot()).collect()
    }

    /// Octets utiles reçus depuis le démarrage du daemon : somme, sur tous
    /// les downloads actifs, de `covered_len() - session_bytes_start`
    /// (le point de départ est fixé à l'hydratation / à l'ajout).
    pub fn session_downloaded_bytes(&self) -> u64 {
        self.active
            .values()
            .map(|a| a.ranges.covered_len().saturating_sub(a.session_bytes_start))
            .sum()
    }

    /// Full details (incl. per-part state and source availability) so the
    /// downloads list can render an eMule-style part map per row.
    pub fn list_download_details(&self) -> Vec<DownloadDetail> {
        self.active
            .values()
            .filter_map(|a| operations::get_download_detail(&self.db, &self.active, &a.download.id))
            .collect()
    }

    pub fn get_download(&self, id: &Uuid) -> Option<DownloadSnapshot> {
        self.active.get(id).map(|a| a.snapshot())
    }

    pub fn get_download_detail(&self, id: &Uuid) -> Option<DownloadDetail> {
        operations::get_download_detail(&self.db, &self.active, id)
    }

    pub fn pause_download(&mut self, id: &Uuid) -> Result<DownloadSnapshot, DaemonError> {
        let snap = operations::pause_download(&mut self.db, &mut self.active, id)?;
        self.emit(|sequence| DaemonEvent::HistoryEntryAdded { sequence });
        Ok(snap)
    }

    pub fn resume_download(&mut self, id: &Uuid) -> Result<DownloadSnapshot, DaemonError> {
        let snap = operations::resume_download(&mut self.db, &mut self.active, id)?;
        self.emit(|sequence| DaemonEvent::HistoryEntryAdded { sequence });
        Ok(snap)
    }

    pub fn remove_download(&mut self, id: &Uuid, delete_files: bool) -> Result<(), DaemonError> {
        operations::remove_download(&mut self.db, &mut self.active, id, delete_files)?;
        self.emit(|sequence| DaemonEvent::DownloadRemoved {
            sequence,
            id: id.to_string(),
        });
        self.emit(|sequence| DaemonEvent::HistoryEntryAdded { sequence });
        Ok(())
    }

    pub fn inject_range(
        &mut self,
        id: &Uuid,
        offset: u64,
        data: &[u8],
    ) -> Result<DownloadSnapshot, DaemonError> {
        operations::inject_range(&mut self.db, &mut self.active, id, offset, data)
    }

    pub fn update_status(
        &mut self,
        id: &Uuid,
        status: DownloadStatus,
    ) -> Result<DownloadSnapshot, DaemonError> {
        let snapshot = operations::update_status(&mut self.db, &mut self.active, id, status)?;
        match status {
            DownloadStatus::Completed => self.emit(|sequence| DaemonEvent::DownloadCompleted {
                sequence,
                download: snapshot.clone(),
            }),
            DownloadStatus::Failed => {
                let reason = self.active.get(id).and_then(|a| a.last_error.clone());
                self.emit(|sequence| DaemonEvent::DownloadFailed {
                    sequence,
                    download: snapshot.clone(),
                    reason: reason.clone(),
                });
                self.emit(|sequence| DaemonEvent::HistoryEntryAdded { sequence });
            }
            _ => {}
        }
        Ok(snapshot)
    }

    /// Updates the eMule-style file priority ("low" | "normal" | "high").
    /// The swarm allocator uses it to bias the slot quota per file.
    pub fn set_priority(
        &mut self,
        id: &Uuid,
        priority: &str,
    ) -> Result<DownloadSnapshot, DaemonError> {
        let act = self
            .active
            .get_mut(id)
            .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;
        if !matches!(priority, "low" | "normal" | "high") {
            return Err(DaemonError::Business(format!(
                "Invalid priority '{priority}' (expected low|normal|high)"
            )));
        }
        act.download.priority = priority.to_string();
        self.db.update_priority(id, priority)?;
        Ok(act.snapshot())
    }

    /// Updates the per-file source limit (eMule FT_MAXSOURCES). `None`
    /// restores the global `max_peers_per_file` default.
    pub fn set_max_sources(
        &mut self,
        id: &Uuid,
        max_sources: Option<u32>,
    ) -> Result<DownloadSnapshot, DaemonError> {
        let act = self
            .active
            .get_mut(id)
            .ok_or_else(|| DaemonError::NotFound(format!("Download {id} not found")))?;
        if let Some(v) = max_sources {
            if v == 0 || v > 10_000 {
                return Err(DaemonError::Business(format!(
                    "Invalid maxSources '{v}' (expected 1..=10000 or null)"
                )));
            }
        }
        act.download.max_sources = max_sources;
        self.db.update_max_sources(id, max_sources)?;
        Ok(act.snapshot())
    }

    pub fn get_missing_ranges(&self, id: &Uuid) -> Option<Vec<std::ops::Range<u64>>> {
        self.active
            .get(id)
            .map(|a| a.ranges.missing_within(0..a.download.total_size))
    }

    pub fn get_covered_len(&self, id: &Uuid) -> Option<u64> {
        self.active.get(id).map(|a| a.ranges.covered_len())
    }

    pub fn get_download_file_info(
        &self,
        id: &Uuid,
    ) -> Option<(PathBuf, PathBuf, mule_format::Ed2kHash, u64)> {
        self.active.get(id).map(|a| {
            (
                a.download.temp_path.clone(),
                a.download.target_path.clone(),
                a.download.ed2k_hash,
                a.download.total_size,
            )
        })
    }

    pub fn record_source_availability(
        &mut self,
        id: &Uuid,
        source: SocketAddr,
        mask: PartAvailabilityMask,
    ) -> Result<DownloadSnapshot, DaemonError> {
        operations::record_source_availability(&mut self.active, id, source, mask)
    }

    pub fn get_source_summaries(&self, id: &Uuid) -> Option<Vec<SourceAvailabilitySummary>> {
        self.active.get(id).map(|a| {
            a.sources
                .values()
                .map(|s| SourceAvailabilitySummary {
                    source_id: s.source_id.clone(),
                    available_part_count: s.mask.available_parts().count() as u16,
                    total_part_count: s.mask.part_count(),
                    last_seen_unix_ms: s.last_seen_unix_ms,
                })
                .collect()
        })
    }

    pub fn get_sources(&self, id: &Uuid) -> Option<HashMap<SocketAddr, PartAvailabilityMask>> {
        self.active.get(id).map(|a| {
            a.sources
                .iter()
                .map(|(addr, s)| (*addr, s.mask.clone()))
                .collect()
        })
    }

    pub fn finalize_download(&mut self, id: &Uuid) -> Result<DownloadSnapshot, DaemonError> {
        operations::finalize_download(&mut self.db, &mut self.active, id)
    }

    pub fn set_destination(
        &mut self,
        id: &Uuid,
        target_dir: &Path,
    ) -> Result<DownloadDetail, DaemonError> {
        operations::set_destination(&mut self.db, &mut self.active, id, target_dir)
    }

    /// `downloads.rename` — renomme le fichier (display_name + target ;
    /// disque + `shared_files` pour un download terminé).
    pub fn rename_download(
        &mut self,
        id: &Uuid,
        new_name: &str,
    ) -> Result<DownloadSnapshot, DaemonError> {
        operations::rename_download(&mut self.db, &mut self.active, id, new_name)
    }

    /// `shares.rename` — renomme un fichier partagé complet sur disque
    /// et dans `shared_files` (délègue à `rename_download` quand le hash
    /// adosse un download listé).
    pub fn rename_shared_file(
        &mut self,
        file_hash: &str,
        new_name: &str,
    ) -> Result<(), DaemonError> {
        operations::rename_shared_file(&mut self.db, &mut self.active, file_hash, new_name)
    }

    pub fn retry_connection(&mut self, id: &Uuid) -> Result<DownloadDetail, DaemonError> {
        operations::retry_connection(&mut self.db, &mut self.active, id)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, DaemonError> {
        self.db
            .get_setting(key)
            .map_err(|e| DaemonError::Business(format!("DB error reading setting: {e}")))
    }

    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<(), DaemonError> {
        self.db
            .set_setting(key, value)
            .map_err(|e| DaemonError::Business(format!("DB error saving setting: {e}")))
    }

    pub fn remove_setting(&mut self, key: &str) -> Result<bool, DaemonError> {
        self.db
            .remove_setting(key)
            .map_err(|e| DaemonError::Business(format!("DB error removing setting: {e}")))
    }

    pub fn query_history(
        &self,
        filter: &mule_core::HistoryFilter,
    ) -> Result<Vec<mule_core::HistoryEntry>, DaemonError> {
        history_categories::query_history(&self.db, filter)
    }

    pub fn get_history_stats(&self) -> Result<mule_core::HistoryStats, DaemonError> {
        history_categories::get_history_stats(&self.db, self.session_start_ms)
    }

    pub fn clear_history(&mut self) -> Result<usize, DaemonError> {
        history_categories::clear_history(&mut self.db)
    }

    pub fn apply_retention_policy(&mut self, retention_days: u32) -> Result<usize, DaemonError> {
        history_categories::apply_retention_policy(&mut self.db, retention_days)
    }

    pub fn export_history(
        &self,
        format: &str,
        filter: &mule_core::HistoryFilter,
    ) -> Result<String, DaemonError> {
        history_categories::export_history(&self.db, format, filter)
    }

    // --- Category Management ---

    pub fn list_categories(&self) -> Result<Vec<DownloadCategory>, DaemonError> {
        history_categories::list_categories(&self.db)
    }

    pub fn get_category(&self, id: &str) -> Result<Option<DownloadCategory>, DaemonError> {
        history_categories::get_category(&self.db, id)
    }

    pub fn create_category(
        &mut self,
        name: &str,
        color: &str,
        default_destination: Option<String>,
        default_priority: Option<String>,
    ) -> Result<DownloadCategory, DaemonError> {
        history_categories::create_category(
            &mut self.db,
            name,
            color,
            default_destination,
            default_priority,
        )
    }

    pub fn update_category(
        &mut self,
        id: &str,
        name: Option<String>,
        color: Option<String>,
        default_destination: Option<Option<String>>,
        default_priority: Option<Option<String>>,
        sort_order: Option<i32>,
    ) -> Result<DownloadCategory, DaemonError> {
        history_categories::update_category(
            &mut self.db,
            id,
            name,
            color,
            default_destination,
            default_priority,
            sort_order,
        )
    }

    pub fn delete_category(&mut self, id: &str) -> Result<usize, DaemonError> {
        history_categories::delete_category(&mut self.db, &mut self.active, id)
    }

    pub fn reorder_categories(&mut self, category_ids: &[String]) -> Result<(), DaemonError> {
        history_categories::reorder_categories(&mut self.db, category_ids)
    }

    pub fn set_download_category(
        &mut self,
        download_id: &Uuid,
        category_id: Option<String>,
    ) -> Result<DownloadDetail, DaemonError> {
        if !self.active.contains_key(download_id) {
            return Err(DaemonError::Business(format!(
                "Download {download_id} not found"
            )));
        }

        let cat_opt = category_id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if let Some(ref cid) = cat_opt {
            let cat_exists = self.get_category(cid)?.is_some();
            if !cat_exists {
                return Err(DaemonError::Business(format!("Category '{cid}' not found")));
            }
        }

        self.db
            .set_download_category(download_id, cat_opt.as_deref())
            .map_err(|e| {
                DaemonError::Business(format!("DB error setting download category: {e}"))
            })?;

        let now = now_ms();
        let active = self.active.get_mut(download_id).unwrap();
        active.download.category_id = cat_opt;
        active.download.updated_at = now;

        self.get_download_detail(download_id).ok_or_else(|| {
            DaemonError::Business(format!("Download {download_id} detail not found"))
        })
    }

    pub fn upsert_servers(
        &mut self,
        servers: &[mule_core::model::Ed2kServer],
    ) -> Result<usize, DaemonError> {
        peers_servers::upsert_servers(&mut self.db, servers)
    }

    pub fn list_servers(&self) -> Result<Vec<mule_core::model::Ed2kServer>, DaemonError> {
        peers_servers::list_servers(&self.db)
    }

    pub fn set_server_enabled(
        &mut self,
        address: &str,
        enabled: bool,
    ) -> Result<bool, DaemonError> {
        peers_servers::set_server_enabled(&mut self.db, address, enabled)
    }

    pub fn delete_server(&mut self, address: &str) -> Result<bool, DaemonError> {
        peers_servers::delete_server(&mut self.db, address)
    }

    pub fn clear_servers(&mut self) -> Result<usize, DaemonError> {
        peers_servers::clear_servers(&mut self.db)
    }

    pub fn upsert_peer_sources(
        &mut self,
        file_hash: &str,
        sources: &[mule_core::model::PeerSource],
    ) -> Result<usize, DaemonError> {
        peers_servers::upsert_peer_sources(&mut self.db, &mut self.active, file_hash, sources)
    }

    pub fn set_download_rate(&mut self, id: &Uuid, rate_bps: u64) -> Option<DownloadSnapshot> {
        if let Some(active) = self.active.get_mut(id) {
            active.current_rate_bps = rate_bps;
            active.last_rate_update = if rate_bps > 0 {
                Some(Instant::now())
            } else {
                None
            };
            Some(active.snapshot())
        } else {
            None
        }
    }

    pub fn set_download_rate_by_hash(
        &mut self,
        hex_hash: &str,
        rate_bps: u64,
    ) -> Option<DownloadSnapshot> {
        let clean_hex = hex_hash.trim().to_lowercase();
        for active in self.active.values_mut() {
            if active.download.ed2k_hash.to_hex().to_lowercase() == clean_hex {
                active.current_rate_bps = rate_bps;
                active.last_rate_update = if rate_bps > 0 {
                    Some(Instant::now())
                } else {
                    None
                };
                return Some(active.snapshot());
            }
        }
        None
    }

    pub fn decay_stale_rates(&mut self) -> Vec<DownloadSnapshot> {
        let mut decayed = Vec::new();
        for active in self.active.values_mut() {
            if active.current_rate_bps > 0 {
                let is_stale = match active.last_rate_update {
                    Some(inst) => inst.elapsed() > Duration::from_secs(2),
                    None => true,
                };
                if is_stale {
                    active.current_rate_bps = 0;
                    active.last_rate_update = None;
                    decayed.push(active.snapshot());
                }
            }
        }
        decayed
    }

    pub fn get_pending_verifications(&mut self) -> Vec<(Uuid, PathBuf, PathBuf, [u8; 16], u64)> {
        let mut pending = Vec::new();
        let now = now_ms();
        for (id, active) in self.active.iter_mut() {
            if active.download.status == DownloadStatus::Completed
                || active.download.status == DownloadStatus::Failed
                || active.download.status == DownloadStatus::Paused
            {
                continue;
            }
            if active.verification_in_progress {
                continue;
            }
            // Do not enter final verification while AICH block recovery is
            // pending — corrupted parts are still covered by the RangeSet.
            if !active.aich_pending.is_empty() {
                continue;
            }
            let total_size = active.download.total_size;
            let covered_len = active.ranges.covered_len();
            let is_complete = total_size > 0 && covered_len == total_size;

            if is_complete {
                tracing::info!(
                    "Download {id} ('{}') completed all byte ranges ({covered_len}/{total_size} bytes). Queuing for eD2K hash verification.",
                    active.download.display_name
                );
                active.verification_in_progress = true;
                active.download.status = DownloadStatus::Verifying;
                active.download.updated_at = now;
                let _ = self.db.update_status(id, DownloadStatus::Verifying);
                let _ = active.part_file.take();
                pending.push((
                    *id,
                    active.download.temp_path.clone(),
                    active.download.target_path.clone(),
                    active.download.ed2k_hash.0,
                    total_size,
                ));
            }
        }
        pending
    }

    pub fn list_peer_sources(
        &self,
        file_hash: &str,
    ) -> Result<Vec<mule_core::model::PeerSource>, DaemonError> {
        peers_servers::list_peer_sources(&self.db, file_hash)
    }

    pub fn update_peer_source_status(
        &mut self,
        file_hash: &str,
        address: &str,
        status: mule_core::model::PeerSourceStatus,
        error: Option<&str>,
        cooldown_until_unix_ms: Option<i64>,
    ) -> Result<bool, DaemonError> {
        let res = peers_servers::update_peer_source_status(
            &mut self.db,
            file_hash,
            address,
            status,
            error,
            cooldown_until_unix_ms,
        )?;
        self.refresh_source_count(file_hash);
        Ok(res)
    }

    /// Compteur « srclist » façon eMule `GetSourceCount()` : seules les
    /// sources vivantes comptent — les entrées deadlist (échec, FNF,
    /// filtre IP) restent persistées mais ne gonflent pas le compteur.
    /// Rafraîchi à chaque transition de statut ou suppression.
    fn refresh_source_count(&mut self, file_hash: &str) {
        let clean_hex = file_hash.trim().to_lowercase();
        if let Ok(list) = self.db.list_peer_sources(file_hash) {
            let count = list
                .iter()
                .filter(|s| s.status.is_srclist_visible())
                .count() as u32;
            for a in self.active.values_mut() {
                if a.download.ed2k_hash.to_hex().to_lowercase() == clean_hex {
                    a.source_count = count;
                }
            }
        }
    }

    pub fn update_peer_source_details(
        &mut self,
        file_hash: &str,
        address: &str,
        software: Option<&str>,
        availability: Option<&[u8]>,
    ) -> Result<bool, DaemonError> {
        peers_servers::update_peer_source_details(
            &mut self.db,
            file_hash,
            address,
            software,
            availability,
        )
    }

    pub fn reset_peer_source_cooldown(
        &mut self,
        file_hash: &str,
        address: &str,
    ) -> Result<bool, DaemonError> {
        peers_servers::reset_peer_source_cooldown(&mut self.db, file_hash, address)
    }

    pub fn delete_peer_source(
        &mut self,
        file_hash: &str,
        address: &str,
    ) -> Result<bool, DaemonError> {
        let res = peers_servers::delete_peer_source(&mut self.db, file_hash, address)?;
        self.refresh_source_count(file_hash);
        Ok(res)
    }

    pub fn clear_peer_sources(&mut self, file_hash: &str) -> Result<usize, DaemonError> {
        let res = peers_servers::clear_peer_sources(&mut self.db, file_hash)?;
        self.refresh_source_count(file_hash);
        Ok(res)
    }

    pub fn find_by_ed2k_hash(&self, hex_hash: &str) -> Option<DownloadSnapshot> {
        let clean_hex = hex_hash.trim().to_lowercase();
        for active in self.active.values() {
            if active.download.ed2k_hash.to_hex().to_lowercase() == clean_hex {
                return Some(active.snapshot());
            }
        }
        None
    }

    /// Nombre de sources connues couvrant chaque part d'un download actif —
    /// indice de rareté pour le swarm healing (`swarm_healing` setting).
    /// `None` si le hash n'est pas un download actif.
    ///
    /// Convention eD2K : une source complète annonce `part_count = 0` (masque
    /// vide) — elle couvre alors toutes les parts. Les sources dont le masque
    /// n'a pas été rafraîchi depuis `SOURCE_FRESHNESS_MS` sont ignorées pour
    /// ne pas compter une disponibilité périmée.
    pub fn part_source_counts(&self, hex_hash: &str) -> Option<Vec<u32>> {
        const ED2K_PART_SIZE: u64 = 9_728_000;
        use types::SOURCE_FRESHNESS_MS;
        let clean = hex_hash.trim().to_lowercase();
        let act = self
            .active
            .values()
            .find(|a| a.download.ed2k_hash.to_hex().to_lowercase() == clean)?;
        let total_parts = (act.download.total_size.div_ceil(ED2K_PART_SIZE)).max(1) as usize;
        let max_part = total_parts.min(u16::MAX as usize);
        let now = now_ms();
        let mut counts = vec![0u32; total_parts];
        for src in act.sources.values() {
            if now - src.last_seen_unix_ms > SOURCE_FRESHNESS_MS {
                continue;
            }
            if src.mask.part_count() == 0 {
                // Source complète : couvre toutes les parts.
                for c in counts.iter_mut() {
                    *c = c.saturating_add(1);
                }
                continue;
            }
            for p in 0..max_part as u16 {
                if src.mask.has_part(p) {
                    counts[p as usize] = counts[p as usize].saturating_add(1);
                }
            }
        }
        Some(counts)
    }

    /// Stores the per-part MD4 hashset for a file hash (learned from peer, link, or .part.met).
    pub fn store_part_hashes_by_hash(
        &mut self,
        hex_hash: &str,
        part_hashes: &[[u8; 16]],
    ) -> Result<(), DaemonError> {
        self.db
            .store_part_hashes(&hex_hash.trim().to_lowercase(), part_hashes)
            .map_err(|e| DaemonError::Business(format!("DB error storing part hashes: {e}")))
    }

    /// Loads the per-part MD4 hashset for a file hash, if known.
    pub fn get_part_hashes_by_hash(&self, hex_hash: &str) -> Vec<[u8; 16]> {
        self.db
            .get_part_hashes(&hex_hash.trim().to_lowercase())
            .unwrap_or_default()
    }

    /// Loads the per-part MD4 hashset for a download, if known.
    pub fn get_part_hashes_for_download(&self, id: &Uuid) -> Vec<[u8; 16]> {
        self.active
            .get(id)
            .map(|a| self.get_part_hashes_by_hash(&a.download.ed2k_hash.to_hex()))
            .unwrap_or_default()
    }

    /// Evicts corrupted parts from the received ranges so they get re-downloaded,
    /// and marks cleanly verified parts. Returns the indices actually evicted.
    pub fn evict_corrupted_parts(
        &mut self,
        id: &Uuid,
        corrupted: &[u32],
    ) -> Result<DownloadSnapshot, DaemonError> {
        operations::evict_corrupted_parts(&mut self.db, &mut self.active, id, corrupted)
    }

    // ── AICH recovery ───────────────────────────────────────────────────────

    /// Records an AICH master hash received from a peer (vote tracking,
    /// eMule `CAICHUntrustedHash` semantics). `masked_ip` = `ip & 0x00F0FFFF`.
    pub fn record_aich_root_vote(
        &mut self,
        id: &Uuid,
        root: [u8; 20],
        masked_ip: u32,
    ) -> operations::AichVoteOutcome {
        operations::record_aich_root_vote(&mut self.db, &mut self.active, id, root, masked_ip)
    }

    /// Returns the trusted AICH root hash for a download, if any.
    pub fn trusted_aich_root(&self, id: &Uuid) -> Option<[u8; 20]> {
        operations::trusted_aich_root(&self.active, id)
    }

    /// Trusted AICH root for a file hash, if any.
    pub fn trusted_aich_root_by_hash(&self, hex_hash: &str) -> Option<[u8; 20]> {
        let clean_hex = hex_hash.trim().to_lowercase();
        self.active
            .values()
            .find(|a| a.download.ed2k_hash.to_hex().to_lowercase() == clean_hex)
            .and_then(|a| a.download.aich_root.map(|r| r.0))
    }

    /// Next corrupted part awaiting an `OP_AICHREQUEST` for this download.
    /// Returns `(part_index, master_hash)`.
    pub fn next_aich_recovery(&mut self, id: &Uuid) -> Option<(u32, [u8; 20])> {
        operations::next_aich_recovery(&mut self.db, &mut self.active, id)
    }

    /// Same, resolved by ed2k file hash.
    pub fn next_aich_recovery_by_hash(&mut self, hex_hash: &str) -> Option<(u32, [u8; 20])> {
        let clean_hex = hex_hash.trim().to_lowercase();
        let id = self
            .active
            .values()
            .find(|a| a.download.ed2k_hash.to_hex().to_lowercase() == clean_hex)
            .map(|a| a.download.id)?;
        self.next_aich_recovery(&id)
    }

    /// Requeues or evicts a part whose AICH recovery request failed.
    pub fn fail_aich_recovery(&mut self, id: &Uuid, part: u32) {
        operations::fail_aich_recovery(&mut self.db, &mut self.active, id, part)
    }

    /// Fails the most recently sent AICH recovery request for this download
    /// (used when the peer answers with a bare file hash — the part index is
    /// not echoed in failed `OP_AICHANSWER` packets).
    pub fn fail_last_aich_request(&mut self, id: &Uuid) {
        let part = self.active.get(id).and_then(|a| {
            a.aich_requested
                .iter()
                .max_by_key(|(_, (ts, _))| *ts)
                .map(|(&p, _)| p)
        });
        if let Some(part) = part {
            self.fail_aich_recovery(id, part);
        }
    }

    /// Applies received AICH recovery data for a corrupted part.
    pub fn apply_aich_recovery(
        &mut self,
        id: &Uuid,
        part: u32,
        master_hash: [u8; 20],
        entries: Vec<mule_format::AichRecoveryEntry>,
    ) -> Result<operations::AichRecoveryReport, DaemonError> {
        operations::apply_aich_recovery(
            &mut self.db,
            &mut self.active,
            id,
            part,
            master_hash,
            entries,
        )
    }

    /// Caches the computed AICH root of a shared file.
    pub fn set_shared_aich_root(
        &mut self,
        file_hash: &str,
        aich_root: &[u8; 20],
    ) -> Result<bool, DaemonError> {
        self.db
            .set_shared_aich_root(file_hash, aich_root)
            .map_err(|e| DaemonError::Business(format!("DB error: {e}")))
    }

    pub fn get_completed_ranges_by_hash(&self, hex_hash: &str) -> Vec<(u64, u64)> {
        let clean_hex = hex_hash.trim().to_lowercase();
        for active in self.active.values() {
            if active.download.ed2k_hash.to_hex().to_lowercase() == clean_hex {
                return active
                    .ranges
                    .ranges()
                    .iter()
                    .map(|r| (r.start, r.end))
                    .collect();
            }
        }
        Vec::new()
    }

    pub fn list_shared_files(&self) -> Result<Vec<mule_core::model::SharedFile>, DaemonError> {
        peers_servers::list_shared_files(&self.db)
    }

    pub fn add_shared_file(
        &mut self,
        file: &mule_core::model::SharedFile,
    ) -> Result<(), DaemonError> {
        peers_servers::add_shared_file(&mut self.db, file)
    }

    /// Ajoute `delta` octets au total uploadé persisté d'un fichier partagé.
    pub fn add_share_uploaded_bytes(
        &mut self,
        file_hash: &str,
        delta: u64,
    ) -> Result<(), DaemonError> {
        peers_servers::add_share_uploaded_bytes(&mut self.db, file_hash, delta)
    }

    /// Totaux uploadés persistés par hash de fichier (clés en minuscules).
    pub fn list_share_uploaded_bytes(
        &self,
    ) -> Result<std::collections::HashMap<String, u64>, DaemonError> {
        peers_servers::list_share_uploaded_bytes(&self.db)
    }

    /// Retire un fichier des partages. Avec `delete_file`, le fichier sur
    /// disque est supprimé en plus : pour un download complété encore
    /// actif on délègue à `remove_download` (entrée DB + fichier final +
    /// share), sinon le fichier pointé par l'entrée est supprimé puis
    /// l'entrée `shared_files` — et l'éventuel download complété en DB —
    /// sont retirés.
    pub fn remove_shared_file(
        &mut self,
        file_hash: &str,
        delete_file: bool,
    ) -> Result<bool, DaemonError> {
        if !delete_file {
            return peers_servers::remove_shared_file(&mut self.db, file_hash);
        }
        let sf = self
            .get_shared_file(file_hash)?
            .ok_or_else(|| DaemonError::NotFound(format!("Shared file {file_hash} not found")))?;
        if sf.is_partial {
            // Partage transitoire d'un download en cours : le fichier à
            // supprimer est le `.part`, ce qui est le rôle de
            // `downloads.remove` — jamais de suppression ici.
            return Err(DaemonError::Business(
                "Cannot delete a partial share — remove the download instead".to_string(),
            ));
        }
        if let Some(id) = self
            .active
            .values()
            .find(|a| {
                a.download
                    .ed2k_hash
                    .to_hex()
                    .eq_ignore_ascii_case(file_hash)
            })
            .map(|a| a.download.id)
        {
            self.remove_download(&id, true)?;
            return Ok(true);
        }
        let path = PathBuf::from(&sf.file_path);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        let removed = peers_servers::remove_shared_file(&mut self.db, file_hash)?;
        // Download complété resté en DB (hors `active`) : sa target_path
        // vient d'être supprimée, l'entrée devient morte — on la retire.
        let stale_db_id = self
            .db
            .get_all_downloads()
            .unwrap_or_default()
            .into_iter()
            .find(|d| {
                d.status == DownloadStatus::Completed
                    && d.ed2k_hash.to_hex().eq_ignore_ascii_case(file_hash)
            })
            .map(|d| d.id);
        if let Some(id) = stale_db_id {
            let _ = self.db.delete_download(&id);
            return Ok(true);
        }
        Ok(removed)
    }

    /// Sets the upload priority of a shared file ("verylow".."veryhigh").
    pub fn set_shared_up_priority(
        &mut self,
        file_hash: &str,
        up_priority: &str,
    ) -> Result<bool, DaemonError> {
        self.db
            .set_shared_up_priority(file_hash, up_priority)
            .map_err(Into::into)
    }

    pub fn get_shared_file(
        &self,
        file_hash: &str,
    ) -> Result<Option<mule_core::model::SharedFile>, DaemonError> {
        if let Some(sf) = peers_servers::get_shared_file(&self.db, file_hash)? {
            return Ok(Some(sf));
        }
        for act in self.active.values() {
            if !act
                .download
                .ed2k_hash
                .to_hex()
                .eq_ignore_ascii_case(file_hash)
            {
                continue;
            }
            let mut sf = Self::shared_entry_for_active(act);
            sf.aich_root = act.download.aich_root.map(|a| a.0);
            return Ok(Some(sf));
        }
        // Completed downloads are persisted in the DB even when not loaded
        // into `active` (e.g. after a daemon restart) — they stay shared
        // like eMule keeps finished files in the shared list.
        if let Some(sf) = self.shared_entry_for_completed_db(file_hash) {
            return Ok(Some(sf));
        }
        Ok(None)
    }

    /// Resolves a completed download from the DB into a shareable entry —
    /// only when the target file actually exists on disk.
    fn shared_entry_for_completed_db(
        &self,
        file_hash: &str,
    ) -> Option<mule_core::model::SharedFile> {
        let dl = self.db.get_all_downloads().ok()?.into_iter().find(|d| {
            d.status == mule_core::DownloadStatus::Completed
                && d.ed2k_hash.to_hex().eq_ignore_ascii_case(file_hash)
        })?;
        if !dl.target_path.is_file() {
            return None;
        }
        let mut sf = mule_core::model::SharedFile::new(
            dl.ed2k_hash.to_hex(),
            &dl.display_name,
            dl.total_size,
            dl.target_path.to_string_lossy().as_ref(),
        );
        sf.aich_root = dl.aich_root.map(|a| a.0);
        sf.category_id = dl.category_id.clone();
        Some(sf)
    }

    /// Builds the shared-file view of an active download (eMule `CPartFile`
    /// semantics): a completed download resolves to its target path; an
    /// in-progress download resolves to its `.part` file as a partial share
    /// exposing only its fully-received eD2K parts. Part files are always
    /// shared files in eMule (`CDownloadQueue::AddPartFilesToShare`), even
    /// before any part completes — the empty `completed_parts` mask simply
    /// advertises nothing servable yet.
    fn shared_entry_for_active(act: &ActiveDownload) -> mule_core::model::SharedFile {
        if act.download.status == mule_core::DownloadStatus::Completed {
            let mut sf = mule_core::model::SharedFile::new(
                act.download.ed2k_hash.to_hex(),
                &act.download.display_name,
                act.download.total_size,
                act.download.target_path.to_string_lossy().as_ref(),
            );
            sf.category_id = act.download.category_id.clone();
            return sf;
        }
        const ED2K_PART_SIZE: u64 = 9_728_000;
        let part_count = act
            .download
            .total_size
            .div_ceil(ED2K_PART_SIZE)
            .min(u16::MAX as u64) as u16;
        let completed: Vec<u16> = (0..part_count)
            .filter(|&i| {
                let start = i as u64 * ED2K_PART_SIZE;
                let end = ((i as u64 + 1) * ED2K_PART_SIZE).min(act.download.total_size);
                end > start && act.ranges.contains(start..end)
            })
            .collect();
        let mut sf = mule_core::model::SharedFile::new(
            act.download.ed2k_hash.to_hex(),
            &act.download.display_name,
            act.download.total_size,
            act.download.temp_path.to_string_lossy().as_ref(),
        );
        sf.is_partial = true;
        sf.completed_parts = completed;
        sf.category_id = act.download.category_id.clone();
        sf
    }

    /// Every file currently servable to peers: persisted shared files plus
    /// active downloads (completed → full file, in-progress → completed
    /// parts only), like eMule's `CSharedFileList` including `CPartFile`s.
    pub fn list_all_shared_files(&self) -> Result<Vec<mule_core::model::SharedFile>, DaemonError> {
        let mut list = self.list_shared_files()?;
        let persisted: std::collections::HashSet<String> =
            list.iter().map(|f| f.file_hash.to_lowercase()).collect();
        for act in self.active.values() {
            if persisted.contains(&act.download.ed2k_hash.to_hex().to_lowercase()) {
                continue;
            }
            let mut sf = Self::shared_entry_for_active(act);
            sf.aich_root = act.download.aich_root.map(|a| a.0);
            list.push(sf);
        }
        // Completed downloads only live in the DB after a restart — merge
        // them too so the shared list survives reboots (eMule rehashes
        // finished files into `CSharedFileList` at startup).
        for d in self.db.get_all_downloads()? {
            if d.status != mule_core::DownloadStatus::Completed {
                continue;
            }
            if persisted.contains(&d.ed2k_hash.to_hex().to_lowercase()) {
                continue;
            }
            if let Some(sf) = self.shared_entry_for_completed_db(&d.ed2k_hash.to_hex()) {
                list.push(sf);
            }
        }
        Ok(list)
    }

    /// `shares.setCategory` — associe (ou retire) une catégorie à un
    /// fichier partagé identifié par son hash eD2K. Met à jour les deux
    /// sources possibles : la ligne `shared_files` persistée et/ou le
    /// download (actif ou complété en DB) dont le partage est synthétisé
    /// — `list_all_shared_files` reflète la catégorie depuis l'une ou
    /// l'autre.
    pub fn set_shared_file_category(
        &mut self,
        file_hash: &str,
        category_id: Option<String>,
    ) -> Result<(), DaemonError> {
        let cat = category_id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if let Some(ref cid) = cat {
            if self.get_category(cid)?.is_none() {
                return Err(DaemonError::Business(format!("Category '{cid}' not found")));
            }
        }
        let mut touched = false;
        // Download actif connu par ce hash.
        let active_id = self
            .active
            .values()
            .find(|a| {
                a.download
                    .ed2k_hash
                    .to_hex()
                    .eq_ignore_ascii_case(file_hash)
            })
            .map(|a| a.download.id);
        if let Some(id) = active_id {
            self.db
                .set_download_category(&id, cat.as_deref())
                .map_err(|e| DaemonError::Business(format!("DB error setting category: {e}")))?;
            if let Some(a) = self.active.get_mut(&id) {
                a.download.category_id = cat.clone();
            }
            touched = true;
        } else {
            // Download complété présent seulement en DB.
            let db_id = self
                .db
                .get_all_downloads()?
                .into_iter()
                .find(|d| d.ed2k_hash.to_hex().eq_ignore_ascii_case(file_hash))
                .map(|d| d.id);
            if let Some(id) = db_id {
                self.db
                    .set_download_category(&id, cat.as_deref())
                    .map_err(|e| {
                        DaemonError::Business(format!("DB error setting category: {e}"))
                    })?;
                touched = true;
            }
        }
        // Partage persisté autonome (fichier ajouté hors download).
        if self
            .db
            .set_shared_file_category(file_hash, cat.as_deref())
            .map_err(|e| DaemonError::Business(format!("DB error setting share category: {e}")))?
        {
            touched = true;
        }
        if !touched {
            return Err(DaemonError::Business(format!(
                "Shared file '{file_hash}' not found"
            )));
        }
        Ok(())
    }

    pub fn get_peer_credit(
        &self,
        pubkey_hash: &str,
    ) -> Result<Option<mule_core::model::PeerCredit>, DaemonError> {
        peers_servers::get_peer_credit(&self.db, pubkey_hash)
    }

    pub fn record_credit_upload(
        &mut self,
        pubkey_hash: &str,
        pubkey_hex: &str,
        bytes: u64,
    ) -> Result<(), DaemonError> {
        peers_servers::record_credit_upload(&mut self.db, pubkey_hash, pubkey_hex, bytes)
    }

    pub fn record_credit_download(
        &mut self,
        pubkey_hash: &str,
        pubkey_hex: &str,
        bytes: u64,
    ) -> Result<(), DaemonError> {
        peers_servers::record_credit_download(&mut self.db, pubkey_hash, pubkey_hex, bytes)
    }

    pub fn list_peer_credits(
        &self,
        limit: usize,
    ) -> Result<Vec<mule_core::model::PeerCredit>, DaemonError> {
        peers_servers::list_peer_credits(&self.db, limit)
    }

    pub fn add_friend(&mut self, user_hash: &str, name: &str) -> Result<(), DaemonError> {
        self.db.add_friend(user_hash, name).map_err(Into::into)
    }

    pub fn remove_friend(&mut self, user_hash: &str) -> Result<(), DaemonError> {
        self.db.remove_friend(user_hash).map_err(Into::into)
    }

    pub fn list_friends(&self) -> Result<Vec<mule_core::model::Friend>, DaemonError> {
        self.db.list_friends().map_err(Into::into)
    }

    pub fn db_ref(&self) -> &Database {
        &self.db
    }

    pub fn db_mut(&mut self) -> &mut Database {
        &mut self.db
    }
}
