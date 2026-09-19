use mule_core::{
    AllocationPolicy, Database, DownloadCategory, DownloadStatus, PartFile,
};
use mule_protocol::ed2k::PartAvailabilityMask;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::error::DaemonError;


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
                && std::fs::metadata(&dl.target_path).map(|m| m.len()).unwrap_or(0) == dl.total_size;

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
                .map(|s| s.len() as u32)
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
        })
    }

    pub fn add_ed2k_link(
        &mut self,
        link_str: &str,
        target_dir: &Path,
        policy: AllocationPolicy,
        category_id: Option<String>,
    ) -> Result<DownloadSnapshot, DaemonError> {
        operations::add_ed2k_link(
            &mut self.db,
            &self.temp_dir,
            &mut self.active,
            link_str,
            target_dir,
            policy,
            category_id,
        )
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
        let (result, active_dl) = part_met::import_part_met(
            &mut self.db,
            &self.temp_dir,
            &self.active,
            met_path,
            part_path,
            target_dir,
            category_id,
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

    pub fn list_downloads(&self) -> Vec<DownloadSnapshot> {
        self.active
            .values()
            .map(|a| a.snapshot())
            .collect()
    }

    pub fn get_download(&self, id: &Uuid) -> Option<DownloadSnapshot> {
        self.active
            .get(id)
            .map(|a| a.snapshot())
    }

    pub fn get_download_detail(&self, id: &Uuid) -> Option<DownloadDetail> {
        operations::get_download_detail(&self.db, &self.active, id)
    }

    pub fn pause_download(&mut self, id: &Uuid) -> Result<DownloadSnapshot, DaemonError> {
        operations::pause_download(&mut self.db, &mut self.active, id)
    }

    pub fn resume_download(&mut self, id: &Uuid) -> Result<DownloadSnapshot, DaemonError> {
        operations::resume_download(&mut self.db, &mut self.active, id)
    }

    pub fn remove_download(&mut self, id: &Uuid, delete_files: bool) -> Result<(), DaemonError> {
        operations::remove_download(&mut self.db, &mut self.active, id, delete_files)
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
        operations::update_status(&mut self.db, &mut self.active, id, status)
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
            return Err(DaemonError::Business(format!("Download {download_id} not found")));
        }

        let cat_opt = category_id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

        if let Some(ref cid) = cat_opt {
            let cat_exists = self.get_category(cid)?.is_some();
            if !cat_exists {
                return Err(DaemonError::Business(format!("Category '{cid}' not found")));
            }
        }

        self.db
            .set_download_category(download_id, cat_opt.as_deref())
            .map_err(|e| DaemonError::Business(format!("DB error setting download category: {e}")))?;

        let now = now_ms();
        let active = self.active.get_mut(download_id).unwrap();
        active.download.category_id = cat_opt;
        active.download.updated_at = now;

        self.get_download_detail(download_id)
            .ok_or_else(|| DaemonError::Business(format!("Download {download_id} detail not found")))
    }

    pub fn upsert_servers(&mut self, servers: &[mule_core::model::Ed2kServer]) -> Result<usize, DaemonError> {
        peers_servers::upsert_servers(&mut self.db, servers)
    }

    pub fn list_servers(&self) -> Result<Vec<mule_core::model::Ed2kServer>, DaemonError> {
        peers_servers::list_servers(&self.db)
    }

    pub fn set_server_enabled(&mut self, address: &str, enabled: bool) -> Result<bool, DaemonError> {
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
            active.last_rate_update = if rate_bps > 0 { Some(Instant::now()) } else { None };
            Some(active.snapshot())
        } else {
            None
        }
    }

    pub fn set_download_rate_by_hash(&mut self, hex_hash: &str, rate_bps: u64) -> Option<DownloadSnapshot> {
        let clean_hex = hex_hash.trim().to_lowercase();
        for active in self.active.values_mut() {
            if active.download.ed2k_hash.to_hex().to_lowercase() == clean_hex {
                active.current_rate_bps = rate_bps;
                active.last_rate_update = if rate_bps > 0 { Some(Instant::now()) } else { None };
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
        peers_servers::update_peer_source_status(
            &mut self.db,
            file_hash,
            address,
            status,
            error,
            cooldown_until_unix_ms,
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
        peers_servers::delete_peer_source(&mut self.db, file_hash, address)
    }

    pub fn clear_peer_sources(&mut self, file_hash: &str) -> Result<usize, DaemonError> {
        peers_servers::clear_peer_sources(&mut self.db, file_hash)
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

    pub fn get_completed_ranges_by_hash(&self, hex_hash: &str) -> Vec<(u64, u64)> {
        let clean_hex = hex_hash.trim().to_lowercase();
        for active in self.active.values() {
            if active.download.ed2k_hash.to_hex().to_lowercase() == clean_hex {
                return active.ranges.ranges().iter().map(|r| (r.start, r.end)).collect();
            }
        }
        Vec::new()
    }

    pub fn list_shared_files(&self) -> Result<Vec<mule_core::model::SharedFile>, DaemonError> {
        peers_servers::list_shared_files(&self.db)
    }

    pub fn add_shared_file(&mut self, file: &mule_core::model::SharedFile) -> Result<(), DaemonError> {
        peers_servers::add_shared_file(&mut self.db, file)
    }

    pub fn remove_shared_file(&mut self, file_hash: &str) -> Result<bool, DaemonError> {
        peers_servers::remove_shared_file(&mut self.db, file_hash)
    }

    pub fn get_shared_file(&self, file_hash: &str) -> Result<Option<mule_core::model::SharedFile>, DaemonError> {
        if let Some(sf) = peers_servers::get_shared_file(&self.db, file_hash)? {
            return Ok(Some(sf));
        }
        for act in self.active.values() {
            if act.download.ed2k_hash.to_hex().eq_ignore_ascii_case(file_hash)
                && act.download.status == mule_core::DownloadStatus::Completed
            {
                return Ok(Some(mule_core::model::SharedFile::new(
                    &act.download.ed2k_hash.to_hex(),
                    &act.download.display_name,
                    act.download.total_size,
                    act.download.target_path.to_string_lossy().as_ref(),
                )));
            }
        }
        Ok(None)
    }

    pub fn get_peer_credit(&self, pubkey_hash: &str) -> Result<Option<mule_core::model::PeerCredit>, DaemonError> {
        peers_servers::get_peer_credit(&self.db, pubkey_hash)
    }

    pub fn record_credit_upload(&mut self, pubkey_hash: &str, pubkey_hex: &str, bytes: u64) -> Result<(), DaemonError> {
        peers_servers::record_credit_upload(&mut self.db, pubkey_hash, pubkey_hex, bytes)
    }

    pub fn record_credit_download(&mut self, pubkey_hash: &str, pubkey_hex: &str, bytes: u64) -> Result<(), DaemonError> {
        peers_servers::record_credit_download(&mut self.db, pubkey_hash, pubkey_hex, bytes)
    }

    pub fn list_peer_credits(&self, limit: usize) -> Result<Vec<mule_core::model::PeerCredit>, DaemonError> {
        peers_servers::list_peer_credits(&self.db, limit)
    }

    pub fn db_ref(&self) -> &Database {
        &self.db
    }

    pub fn db_mut(&mut self) -> &mut Database {
        &mut self.db
    }
}
