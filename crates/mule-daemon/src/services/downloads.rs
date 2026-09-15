use mule_core::{
    AllocationPolicy, Database, Download, DownloadCategory, DownloadStatus, PartFile, RangeSet,
};
use mule_format::Ed2kFileLink;
use mule_protocol::ed2k::PartAvailabilityMask;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::JsonRpcError;
use crate::rpc::events::{
    DownloadDetail, DownloadSnapshot, ImportPartMetResult, PartMetInspection, PartProgressSummary,
};

/// Neutral availability summary exposed to UI and external clients.
/// Never leaks raw SocketAddr or network topology.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceAvailabilitySummary {
    /// Opaque source identifier.
    pub source_id: String,
    /// Number of available parts.
    pub available_part_count: u16,
    /// Total number of parts in the file.
    pub total_part_count: u16,
    /// Timestamp of last observation.
    pub last_seen_unix_ms: i64,
}

#[derive(Debug, Clone)]
struct ActiveSource {
    source_id: String,
    _addr: SocketAddr,
    mask: PartAvailabilityMask,
    last_seen_unix_ms: i64,
}

struct ActiveDownload {
    download: Download,
    ranges: RangeSet,
    part_file: Option<PartFile>,
    sources: HashMap<SocketAddr, ActiveSource>,
    connection_attempts: u32,
    resume_count: u32,
    last_error: Option<String>,
    last_status_change_unix_ms: i64,
    session_start_ms: i64,
    session_bytes_start: u64,
}

pub struct DownloadService {
    db: Database,
    temp_dir: PathBuf,
    active: HashMap<Uuid, ActiveDownload>,
    session_start_ms: i64,
}

impl DownloadService {
    pub fn new(db_path: &Path, temp_dir: PathBuf) -> Result<Self, JsonRpcError> {
        let mut db = Database::open(db_path)
            .map_err(|e| JsonRpcError::business(format!("Failed to open DB: {e}")))?;
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| JsonRpcError::business(format!("Failed to create temp dir: {e}")))?;

        let session_start_ms = now_ms();
        let existing = db.get_all_downloads().unwrap_or_default();
        let mut active = HashMap::new();
        for dl in existing {
            let ranges = db.load_received_ranges(&dl.id).unwrap_or_default();
            let part_file =
                PartFile::open_or_create(&dl.temp_path, dl.total_size, AllocationPolicy::None).ok();
            let covered = ranges.covered_len();
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

    pub fn in_memory(temp_dir: PathBuf) -> Result<Self, JsonRpcError> {
        let db = Database::open_in_memory()
            .map_err(|e| JsonRpcError::business(format!("Failed to open in-memory DB: {e}")))?;
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| JsonRpcError::business(format!("Failed to create temp dir: {e}")))?;

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
    ) -> Result<DownloadSnapshot, JsonRpcError> {
        let link = Ed2kFileLink::parse(link_str)
            .map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

        let id = Uuid::new_v4();
        let temp_filename = format!("{id}.part");
        let temp_path = self.temp_dir.join(temp_filename);
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

        // Open/create part file on disk
        let part_file = PartFile::open_or_create(&temp_path, link.size, policy)
            .map_err(|e| JsonRpcError::business(format!("Storage error: {e}")))?;

        // Persist to SQLite
        self.db
            .insert_download(&download)
            .map_err(|e| JsonRpcError::business(format!("DB error: {e}")))?;

        let ranges = RangeSet::new();
        let snapshot = make_snapshot(&download, &ranges, 0);
        let now = now_ms();

        self.active.insert(
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
            },
        );

        Ok(snapshot)
    }

    pub fn inspect_part_met(
        &self,
        met_path: &Path,
        part_path: Option<&Path>,
    ) -> Result<PartMetInspection, JsonRpcError> {
        if !met_path.exists() {
            return Err(JsonRpcError::business(format!(
                ".part.met file not found: {}",
                met_path.display()
            )));
        }

        let met = mule_format::PartMetFile::open(met_path)
            .map_err(|e| JsonRpcError::business(format!("Invalid .part.met file: {e}")))?;

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
            if met_name.ends_with(".part.met") {
                let part_name = format!("{}.part", &met_name[..met_name.len() - ".part.met".len()]);
                met_path.with_file_name(part_name)
            } else if met_name.ends_with(".met") {
                let part_name = format!("{}.part", &met_name[..met_name.len() - ".met".len()]);
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

pub const MAX_IMPORT_TOTAL_SIZE: u64 = 1024 * 1024 * 1024 * 1024; // 1 TB safety ceiling
pub const MAX_IMPORT_PART_COUNT: u32 = 65_536; // 65,536 parts ceiling

    pub fn import_part_met(
        &mut self,
        met_path: &Path,
        part_path: Option<&Path>,
        target_dir: &Path,
        category_id: Option<String>,
    ) -> Result<ImportPartMetResult, JsonRpcError> {
        let inspection = self.inspect_part_met(met_path, part_path)?;

        let met = mule_format::PartMetFile::open(met_path)
            .map_err(|e| JsonRpcError::business(format!("Invalid .part.met file: {e}")))?;

        let total_size = met
            .file_size
            .ok_or_else(|| JsonRpcError::business("File size missing in .part.met file"))?;

        if total_size > Self::MAX_IMPORT_TOTAL_SIZE {
            let max = Self::MAX_IMPORT_TOTAL_SIZE;
            return Err(JsonRpcError::business(format!(
                "File size ({total_size} bytes) exceeds maximum import safety limit of {max} bytes"
            )));
        }

        let total_parts = met.part_count() as u32;
        if total_parts > Self::MAX_IMPORT_PART_COUNT {
            let max = Self::MAX_IMPORT_PART_COUNT;
            return Err(JsonRpcError::business(format!(
                "Part count ({total_parts}) exceeds maximum import safety limit of {max} parts"
            )));
        }

        let actual_part_path = PathBuf::from(inspection.part_path.as_ref().unwrap());
        if !actual_part_path.exists() {
            return Err(JsonRpcError::business(format!(
                "Associated .part file not found: {}",
                actual_part_path.display()
            )));
        }

        let metadata = std::fs::metadata(&actual_part_path)
            .map_err(|e| JsonRpcError::business(format!("Failed to read .part metadata: {e}")))?;
        if metadata.len() > total_size {
            return Err(JsonRpcError::business(format!(
                "Physical .part file size ({} bytes) exceeds declared file size ({} bytes)",
                metadata.len(),
                total_size
            )));
        }

        let completed_ranges = met
            .compute_completed_ranges()
            .map_err(|e| JsonRpcError::business(format!("Failed to compute completed ranges: {e}")))?;

        let mut range_set = RangeSet::new();
        for (s, e) in completed_ranges {
            let _ = range_set.insert(s..e);
        }

        let id = Uuid::new_v4();
        let internal_temp_filename = format!("{id}.part");
        let internal_temp_path = self.temp_dir.join(internal_temp_filename);
        let target_path = target_dir.join(&inspection.file_name);

        std::fs::copy(&actual_part_path, &internal_temp_path).map_err(|e| {
            JsonRpcError::business(format!("Failed to stage .part file into sandbox: {e}"))
        })?;

        let mut part_file =
            PartFile::open_or_create(&internal_temp_path, total_size, AllocationPolicy::None)
                .map_err(|e| JsonRpcError::business(format!("Failed to open staged .part file: {e}")))?;

        let part_size_const = mule_format::ed2k::hash::ED2K_PART_SIZE as u64;
        let total_parts = met.part_count() as u32;
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
        };

        self.db
            .insert_download(&download)
            .map_err(|e| JsonRpcError::business(format!("DB error: {e}")))?;

        self.db
            .sync_received_ranges(&id, &range_set)
            .map_err(|e| JsonRpcError::business(format!("Failed to sync ranges: {e}")))?;

        for idx in verified_indices {
            let _ = self.db.mark_part_verified(&id, idx);
        }

        let verif_status = if corrupted_parts > 0 {
            "partial_corruption"
        } else if verified_parts == total_parts && total_parts > 0 {
            "verified_ok"
        } else {
            "unverified"
        };
        self.record_history_event(
            &download,
            range_set.covered_len(),
            0,
            0,
            "imported",
            verif_status,
            None,
        );

        let snapshot = make_snapshot(&download, &range_set, 0);

        self.active.insert(
            id,
            ActiveDownload {
                download,
                ranges: range_set,
                part_file: Some(part_file),
                sources: HashMap::new(),
                connection_attempts: 0,
                resume_count: 0,
                last_error: None,
                last_status_change_unix_ms: now,
                session_start_ms: now,
                session_bytes_start: snapshot.received_bytes,
            },
        );

        let met_version_str = match met.version {
            mule_format::PARTFILE_VERSION => "0xE0 (Standard eMule)",
            mule_format::PARTFILE_SPLITTEDVERSION => "0xE1 (Splitted eMule)",
            mule_format::PARTFILE_VERSION_LARGEFILE => "0xE2 (Large Files 64-bit)",
            _ => "Unknown/Custom",
        };

        let report = format!(
            "=== Emule-Sion .part.met Import Audit Report ===\n\
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

        Ok(ImportPartMetResult {
            download: snapshot.clone(),
            total_parts,
            verified_parts,
            corrupted_parts,
            transferred_bytes: snapshot.received_bytes,
            total_size,
            report: Some(report),
        })
    }


    pub fn list_downloads(&self) -> Vec<DownloadSnapshot> {
        self.active
            .values()
            .map(|a| make_snapshot(&a.download, &a.ranges, a.sources.len() as u32))
            .collect()
    }

    pub fn get_download(&self, id: &Uuid) -> Option<DownloadSnapshot> {
        self.active
            .get(id)
            .map(|a| make_snapshot(&a.download, &a.ranges, a.sources.len() as u32))
    }

    pub fn get_download_detail(&self, id: &Uuid) -> Option<DownloadDetail> {
        let active = self.active.get(id)?;
        let snapshot = make_snapshot(&active.download, &active.ranges, active.sources.len() as u32);

        let total_size = active.download.total_size;
        let part_size_const = mule_format::ed2k::hash::ED2K_PART_SIZE as u64;
        let part_count = if total_size == 0 {
            1
        } else {
            total_size.div_ceil(part_size_const) as u32
        };

        let verified_parts_set: std::collections::HashSet<u32> = self
            .db
            .get_verified_parts(id)
            .unwrap_or_default()
            .into_iter()
            .collect();

        let all_ranges = active.ranges.ranges();

        let mut parts = Vec::with_capacity(part_count as usize);
        let mut total_verified_bytes = 0u64;

        for i in 0..part_count {
            let start_offset = i as u64 * part_size_const;
            let end_offset = ((i as u64 + 1) * part_size_const).min(total_size);
            let size = if end_offset > start_offset {
                end_offset - start_offset
            } else {
                0
            };

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

            let is_verified = active.download.status == DownloadStatus::Completed
                || verified_parts_set.contains(&i);

            if is_verified {
                total_verified_bytes += size;
            }

            let state = if is_verified {
                "verified".to_string()
            } else if received_in_part >= size && size > 0 {
                "completed".to_string()
            } else if received_in_part > 0 {
                "downloading".to_string()
            } else {
                "missing".to_string()
            };

            parts.push(PartProgressSummary {
                part_index: i,
                start_offset,
                end_offset,
                size,
                received_bytes: received_in_part,
                is_verified,
                state,
            });
        }

        let link_builder = Ed2kFileLink {
            file_name: active.download.display_name.clone(),
            size: total_size,
            ed2k_hash: active.download.ed2k_hash,
            aich_root: active.download.aich_root.as_ref().map(|a| a.0),
            part_hashes: Vec::new(),
            web_sources: Vec::new(),
            peer_sources: Vec::new(),
            unknown_fields: Vec::new(),
        };
        let ed2k_link = link_builder.to_link_string();

        let elapsed_sec = ((now_ms() - active.session_start_ms) / 1000).max(1) as u64;
        let transferred = snapshot.received_bytes.saturating_sub(active.session_bytes_start);
        let average_rate_bps = transferred / elapsed_sec;
        let remaining_bytes = total_size.saturating_sub(snapshot.received_bytes);

        let effective_rate = if snapshot.download_rate_bps > 0 {
            snapshot.download_rate_bps
        } else {
            average_rate_bps
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
            ed2k_hash: hex::encode(active.download.ed2k_hash.0),
            aich_root: active.download.aich_root.as_ref().map(|a| a.to_base32()),
            ed2k_link,
            temp_path: active.download.temp_path.to_string_lossy().to_string(),
            target_path: active.download.target_path.to_string_lossy().to_string(),
            remaining_bytes,
            average_rate_bps,
            eta_seconds,
            connection_attempts: active.connection_attempts,
            last_error: active.last_error.clone(),
            last_status_change_unix_ms: active.last_status_change_unix_ms,
            parts,
        })
    }

    pub fn pause_download(&mut self, id: &Uuid) -> Result<DownloadSnapshot, JsonRpcError> {
        let active = self
            .active
            .get_mut(id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

        let now = now_ms();
        active.download.status = DownloadStatus::Paused;
        active.download.updated_at = now;
        active.last_status_change_unix_ms = now;
        self.db
            .update_status(id, DownloadStatus::Paused)
            .map_err(|e| JsonRpcError::business(format!("DB error: {e}")))?;

        Ok(make_snapshot(&active.download, &active.ranges, active.sources.len() as u32))
    }

    pub fn resume_download(&mut self, id: &Uuid) -> Result<DownloadSnapshot, JsonRpcError> {
        let (dl, ranges, conn_att, res_cnt) = {
            let active = self
                .active
                .get_mut(id)
                .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

            let now = now_ms();
            active.download.status = DownloadStatus::Downloading;
            active.download.updated_at = now;
            active.last_status_change_unix_ms = now;
            active.resume_count = active.resume_count.saturating_add(1);
            (
                active.download.clone(),
                active.ranges.clone(),
                active.connection_attempts,
                active.resume_count,
            )
        };

        self.db
            .update_status(id, DownloadStatus::Downloading)
            .map_err(|e| JsonRpcError::business(format!("DB error: {e}")))?;

        self.record_history_event(
            &dl,
            ranges.covered_len(),
            conn_att,
            res_cnt,
            "resumed",
            "unverified",
            None,
        );

        let active = self.active.get(id).unwrap();
        Ok(make_snapshot(&active.download, &active.ranges, active.sources.len() as u32))
    }

    pub fn remove_download(&mut self, id: &Uuid, delete_files: bool) -> Result<(), JsonRpcError> {
        let mut temp_path_to_delete = None;
        let (was_active, history_data) = if let Some(mut active) = self.active.remove(id) {
            let _ = active.part_file.take(); // Explicitly drop/close file handle first (vital on Windows)
            if delete_files {
                temp_path_to_delete = Some(active.download.temp_path.clone());
            }
            let verif = if active.download.status == DownloadStatus::Completed {
                "verified_ok"
            } else {
                "unverified"
            };
            (
                true,
                Some((
                    active.download,
                    active.ranges.covered_len(),
                    active.connection_attempts,
                    active.resume_count,
                    verif,
                )),
            )
        } else {
            (false, None)
        };

        if let Some((dl, transferred, attempts, resumes, verif)) = history_data {
            self.record_history_event(
                &dl,
                transferred,
                attempts,
                resumes,
                "removed",
                verif,
                None,
            );
        }

        let db_deleted = self
            .db
            .delete_download(id)
            .map_err(|e| JsonRpcError::business(format!("DB error deleting download: {e}")))?;

        if let Some(temp_path) = temp_path_to_delete {
            if temp_path.exists() {
                let _ = std::fs::remove_file(temp_path);
            }
        }

        if was_active || db_deleted {
            Ok(())
        } else {
            Err(JsonRpcError::business(format!("Download {id} not found")))
        }
    }

    /// Injects a data segment directly (used for tests and simulation)
    pub fn inject_range(
        &mut self,
        id: &Uuid,
        offset: u64,
        data: &[u8],
    ) -> Result<DownloadSnapshot, JsonRpcError> {
        let active = self
            .active
            .get_mut(id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

        let end = offset + data.len() as u64;

        if let Some(pf) = &mut active.part_file {
            pf.write_segment(offset, data)
                .map_err(|e| JsonRpcError::business(format!("Write error: {e}")))?;
            pf.flush()
                .map_err(|e| JsonRpcError::business(format!("Flush error: {e}")))?;
        }

        active
            .ranges
            .insert(offset..end)
            .map_err(|e| JsonRpcError::business(format!("Range error: {e}")))?;

        // Synchronize to DB
        self.db
            .sync_received_ranges(id, &active.ranges)
            .map_err(|e| JsonRpcError::business(format!("DB sync error: {e}")))?;

        active.download.status = if active.ranges.covered_len() == active.download.total_size {
            DownloadStatus::Verifying
        } else {
            DownloadStatus::Downloading
        };
        active.download.updated_at = now_ms();
        let _ = self.db.update_status(id, active.download.status);

        Ok(make_snapshot(&active.download, &active.ranges, active.sources.len() as u32))
    }

    pub fn update_status(
        &mut self,
        id: &Uuid,
        status: DownloadStatus,
    ) -> Result<DownloadSnapshot, JsonRpcError> {
        let (old_status, dl, transferred, attempts, resumes, last_error) = {
            let active = self
                .active
                .get_mut(id)
                .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

            let old = active.download.status;
            active.download.status = status;
            active.download.updated_at = now_ms();
            (
                old,
                active.download.clone(),
                active.ranges.covered_len(),
                active.connection_attempts,
                active.resume_count,
                active.last_error.clone(),
            )
        };

        self.db
            .update_status(id, status)
            .map_err(|e| JsonRpcError::business(format!("DB error: {e}")))?;

        if status == DownloadStatus::Failed {
            let verif = if old_status == DownloadStatus::Verifying {
                "hash_mismatch"
            } else {
                "unverified"
            };
            self.record_history_event(
                &dl,
                transferred,
                attempts,
                resumes,
                "failed",
                verif,
                last_error,
            );
        } else if status == DownloadStatus::Verifying {
            self.record_history_event(
                &dl,
                transferred,
                attempts,
                resumes,
                "verified_pending_promotion",
                "unverified",
                None,
            );
        }

        let active = self.active.get(id).unwrap();
        Ok(make_snapshot(&active.download, &active.ranges, active.sources.len() as u32))
    }

    pub fn get_missing_ranges(&self, id: &Uuid) -> Option<Vec<std::ops::Range<u64>>> {
        self.active
            .get(id)
            .map(|a| a.ranges.missing_within(0..a.download.total_size))
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

    /// Records peer part availability for a download without writing any block data.
    pub fn record_source_availability(
        &mut self,
        id: &Uuid,
        source: SocketAddr,
        mask: PartAvailabilityMask,
    ) -> Result<DownloadSnapshot, JsonRpcError> {
        let active = self
            .active
            .get_mut(id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

        let now = now_ms();
        let source_id = format!("src_{:08x}", rand::random::<u32>());
        let entry = active.sources.entry(source).or_insert_with(|| ActiveSource {
            source_id,
            _addr: source,
            mask: mask.clone(),
            last_seen_unix_ms: now,
        });
        entry.mask = mask;
        entry.last_seen_unix_ms = now;
        active.download.updated_at = now;

        Ok(make_snapshot(&active.download, &active.ranges, active.sources.len() as u32))
    }

    /// Returns the sanitized source availability summaries for external clients (neutral DTO, zero raw IP/port).
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

    /// Returns the active source addresses and masks internally.
    pub fn get_sources(&self, id: &Uuid) -> Option<HashMap<SocketAddr, PartAvailabilityMask>> {
        self.active.get(id).map(|a| {
            a.sources
                .iter()
                .map(|(addr, s)| (*addr, s.mask.clone()))
                .collect()
        })
    }

    pub fn finalize_download(&mut self, id: &Uuid) -> Result<DownloadSnapshot, JsonRpcError> {
        let (temp_path, target_path, dl, transferred, attempts, resumes) = {
            let active = self
                .active
                .get_mut(id)
                .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;
            let _ = active.part_file.take();
            (
                active.download.temp_path.clone(),
                active.download.target_path.clone(),
                active.download.clone(),
                active.ranges.covered_len(),
                active.connection_attempts,
                active.resume_count,
            )
        };

        mule_core::finalize_file(
            &temp_path,
            &target_path,
            mule_core::CollisionStrategy::Overwrite,
        )
        .map_err(|e| JsonRpcError::business(format!("Finalize error: {e}")))?;

        let now = now_ms();
        {
            let active = self.active.get_mut(id).unwrap();
            active.download.status = DownloadStatus::Completed;
            active.download.updated_at = now;
            active.last_status_change_unix_ms = now;
        }
        self.db
            .update_status(id, DownloadStatus::Completed)
            .map_err(|e| JsonRpcError::business(format!("DB error: {e}")))?;

        let mut finalized_dl = dl;
        finalized_dl.status = DownloadStatus::Completed;
        self.record_history_event(
            &finalized_dl,
            transferred,
            attempts,
            resumes,
            "completed",
            "verified_ok",
            None,
        );

        let active = self.active.get(id).unwrap();
        Ok(make_snapshot(&active.download, &active.ranges, active.sources.len() as u32))
    }

    /// Updates the finalization target directory for a download.
    ///
    /// Invariant: The temporary `.part` file in `data/temp/` is never moved.
    /// Only the destination of the final file when completed is updated.
    pub fn set_destination(
        &mut self,
        id: &Uuid,
        target_dir: &Path,
    ) -> Result<DownloadDetail, JsonRpcError> {
        let active = self
            .active
            .get_mut(id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

        // 1. Validate target directory: must be creatable/writable
        std::fs::create_dir_all(target_dir)
            .map_err(|e| JsonRpcError::business(format!("Cannot create target directory: {e}")))?;

        let probe_file = target_dir.join(format!(".mule_write_probe_{}", Uuid::new_v4()));
        match std::fs::write(&probe_file, b"probe") {
            Ok(_) => {
                let _ = std::fs::remove_file(&probe_file);
            }
            Err(e) => {
                return Err(JsonRpcError::business(format!(
                    "Target directory is not writable: {e}"
                )));
            }
        }

        // 2. Set new target path
        let new_target_path = target_dir.join(&active.download.display_name);
        active.download.target_path = new_target_path.clone();
        active.download.updated_at = now_ms();

        // 3. Persist to SQLite
        self.db
            .update_target_path(id, &new_target_path)
            .map_err(|e| JsonRpcError::business(format!("DB error updating target path: {e}")))?;

        self.get_download_detail(id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {id} detail not found")))
    }

    /// Retries connection to loopback peers for this download.
    pub fn retry_connection(&mut self, id: &Uuid) -> Result<DownloadDetail, JsonRpcError> {
        let active = self
            .active
            .get_mut(id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

        let now = now_ms();
        active.connection_attempts = active.connection_attempts.saturating_add(1);
        active.last_error = None;
        active.last_status_change_unix_ms = now;
        active.download.updated_at = now;

        if active.download.status == DownloadStatus::Failed
            || active.download.status == DownloadStatus::Paused
        {
            active.download.status = DownloadStatus::Downloading;
            let _ = self.db.update_status(id, DownloadStatus::Downloading);
        }

        self.get_download_detail(id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {id} detail not found")))
    }

    /// Reads a persistent setting from SQLite.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, JsonRpcError> {
        self.db
            .get_setting(key)
            .map_err(|e| JsonRpcError::business(format!("DB error reading setting: {e}")))
    }

    /// Writes a persistent setting to SQLite.
    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<(), JsonRpcError> {
        self.db
            .set_setting(key, value)
            .map_err(|e| JsonRpcError::business(format!("DB error saving setting: {e}")))
    }

    /// Internal helper to record history transitions.
    fn record_history_event(
        &mut self,
        download: &Download,
        transferred_bytes: u64,
        connection_attempts: u32,
        resume_count: u32,
        event_type: &str,
        verification_result: &str,
        error_message: Option<String>,
    ) {
        let now = now_ms();
        let duration_secs = ((now - download.created_at) / 1000).max(0) as u64;
        let entry = mule_core::HistoryEntry {
            id: 0,
            download_id: download.id.to_string(),
            file_name: download.display_name.clone(),
            total_size: download.total_size,
            transferred_bytes,
            ed2k_hash: hex::encode(download.ed2k_hash.0),
            event_type: event_type.to_string(),
            status: download.status.as_str().to_string(),
            duration_secs,
            connection_attempts,
            resume_count,
            verification_result: verification_result.to_string(),
            error_message,
            timestamp_unix_ms: now,
        };
        if let Err(e) = self.db.insert_history_entry(&entry) {
            tracing::warn!("Failed to insert history entry: {e}");
        }
    }

    /// Queries historical download transitions with filtering.
    pub fn query_history(
        &self,
        filter: &mule_core::HistoryFilter,
    ) -> Result<Vec<mule_core::HistoryEntry>, JsonRpcError> {
        self.db
            .query_history(filter)
            .map_err(|e| JsonRpcError::business(format!("DB error querying history: {e}")))
    }

    /// Computes aggregated statistics across session, 24h, 7d, and lifetime.
    pub fn get_history_stats(&self) -> Result<mule_core::HistoryStats, JsonRpcError> {
        self.db
            .get_history_stats(self.session_start_ms)
            .map_err(|e| JsonRpcError::business(format!("DB error calculating history stats: {e}")))
    }

    /// Clears all historical entries.
    pub fn clear_history(&mut self) -> Result<usize, JsonRpcError> {
        self.db
            .clear_history()
            .map_err(|e| JsonRpcError::business(format!("DB error clearing history: {e}")))
    }

    /// Prunes historical entries older than `retention_days`. (0 = keep indefinitely)
    pub fn apply_retention_policy(&mut self, retention_days: u32) -> Result<usize, JsonRpcError> {
        self.db
            .apply_retention_policy(retention_days)
            .map_err(|e| JsonRpcError::business(format!("DB error applying retention policy: {e}")))
    }

    /// Exports history in either CSV or JSON format with all paths and sensitive information sanitized.
    pub fn export_history(
        &self,
        format: &str,
        filter: &mule_core::HistoryFilter,
    ) -> Result<String, JsonRpcError> {
        let entries = self.query_history(filter)?;
        match format.to_ascii_lowercase().as_str() {
            "csv" => {
                let mut out = String::from(
                    "id,download_id,file_name,total_size,transferred_bytes,ed2k_hash,event_type,status,duration_secs,connection_attempts,resume_count,verification_result,error_message,timestamp_unix_ms\n",
                );
                for e in entries {
                    let escaped_name = e.file_name.replace('"', "\"\"");
                    let escaped_err = e.error_message.as_deref().unwrap_or("").replace('"', "\"\"");
                    out.push_str(&format!(
                        "{},{},\"{}\",{},{},{},{},{},{},{},{},{},\"{}\",{}\n",
                        e.id,
                        e.download_id,
                        escaped_name,
                        e.total_size,
                        e.transferred_bytes,
                        e.ed2k_hash,
                        e.event_type,
                        e.status,
                        e.duration_secs,
                        e.connection_attempts,
                        e.resume_count,
                        e.verification_result,
                        escaped_err,
                        e.timestamp_unix_ms
                    ));
                }
                Ok(out)
            }
            "json" => serde_json::to_string_pretty(&entries)
                .map_err(|e| JsonRpcError::business(format!("JSON serialization error: {e}"))),
            other => Err(JsonRpcError::invalid_params(&format!(
                "Unsupported export format: {other}"
            ))),
        }
    }

    /// Removes a persistent setting from SQLite.
    pub fn remove_setting(&mut self, key: &str) -> Result<bool, JsonRpcError> {
        self.db
            .remove_setting(key)
            .map_err(|e| JsonRpcError::business(format!("DB error removing setting: {e}")))
    }

    // --- Category Management ---

    pub fn list_categories(&self) -> Result<Vec<DownloadCategory>, JsonRpcError> {
        self.db
            .get_categories()
            .map_err(|e| JsonRpcError::business(format!("DB error listing categories: {e}")))
    }

    pub fn get_category(&self, id: &str) -> Result<Option<DownloadCategory>, JsonRpcError> {
        self.db
            .get_category(id)
            .map_err(|e| JsonRpcError::business(format!("DB error getting category: {e}")))
    }

    pub fn create_category(
        &mut self,
        name: &str,
        color: &str,
        default_destination: Option<String>,
        default_priority: Option<String>,
    ) -> Result<DownloadCategory, JsonRpcError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(JsonRpcError::invalid_params("Category name cannot be empty"));
        }

        let existing = self.list_categories()?;
        let sort_order = existing.len() as i32;

        let category = DownloadCategory::new(
            name,
            color,
            sort_order,
            default_destination.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            default_priority.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()),
        );

        self.db
            .insert_category(&category)
            .map_err(|e| JsonRpcError::business(format!("DB error creating category: {e}")))?;

        Ok(category)
    }

    pub fn update_category(
        &mut self,
        id: &str,
        name: Option<String>,
        color: Option<String>,
        default_destination: Option<Option<String>>,
        default_priority: Option<Option<String>>,
        sort_order: Option<i32>,
    ) -> Result<DownloadCategory, JsonRpcError> {
        let mut cat = self
            .get_category(id)?
            .ok_or_else(|| JsonRpcError::business(format!("Category '{id}' not found")))?;

        if let Some(n) = name {
            let trimmed = n.trim();
            if trimmed.is_empty() {
                return Err(JsonRpcError::invalid_params("Category name cannot be empty"));
            }
            cat.name = trimmed.to_string();
        }

        if let Some(c) = color {
            cat.color = c;
        }

        if let Some(dest_opt) = default_destination {
            cat.default_destination = dest_opt.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        }

        if let Some(prio_opt) = default_priority {
            cat.default_priority = prio_opt.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
        }

        if let Some(order) = sort_order {
            cat.sort_order = order;
        }

        cat.updated_at = now_ms();

        self.db
            .update_category(&cat)
            .map_err(|e| JsonRpcError::business(format!("DB error updating category: {e}")))?;

        Ok(cat)
    }

    pub fn delete_category(&mut self, id: &str) -> Result<usize, JsonRpcError> {
        // Atomic reassignment in DB
        let reassigned = self
            .db
            .delete_category(id)
            .map_err(|e| JsonRpcError::business(format!("DB error deleting category: {e}")))?;

        // Update in-memory active downloads
        let now = now_ms();
        for active in self.active.values_mut() {
            if active.download.category_id.as_deref() == Some(id) {
                active.download.category_id = None;
                active.download.updated_at = now;
            }
        }

        Ok(reassigned)
    }

    pub fn reorder_categories(&mut self, category_ids: &[String]) -> Result<(), JsonRpcError> {
        self.db
            .reorder_categories(category_ids)
            .map_err(|e| JsonRpcError::business(format!("DB error reordering categories: {e}")))
    }

    pub fn set_download_category(
        &mut self,
        download_id: &Uuid,
        category_id: Option<String>,
    ) -> Result<DownloadDetail, JsonRpcError> {
        if !self.active.contains_key(download_id) {
            return Err(JsonRpcError::business(format!("Download {download_id} not found")));
        }

        let cat_opt = category_id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

        if let Some(ref cid) = cat_opt {
            let cat_exists = self.get_category(cid)?.is_some();
            if !cat_exists {
                return Err(JsonRpcError::business(format!("Category '{cid}' not found")));
            }
        }

        // Persist to DB
        self.db
            .set_download_category(download_id, cat_opt.as_deref())
            .map_err(|e| JsonRpcError::business(format!("DB error setting download category: {e}")))?;

        let now = now_ms();
        let active = self.active.get_mut(download_id).unwrap();
        active.download.category_id = cat_opt;
        active.download.updated_at = now;

        self.get_download_detail(download_id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {download_id} detail not found")))
    }

    pub fn upsert_servers(&mut self, servers: &[mule_core::model::Ed2kServer]) -> Result<usize, JsonRpcError> {
        self.db
            .upsert_servers(servers)
            .map_err(|e| JsonRpcError::business(format!("DB error upserting servers: {e}")))
    }

    pub fn list_servers(&self) -> Result<Vec<mule_core::model::Ed2kServer>, JsonRpcError> {
        self.db
            .list_servers()
            .map_err(|e| JsonRpcError::business(format!("DB error listing servers: {e}")))
    }

    pub fn set_server_enabled(&mut self, address: &str, enabled: bool) -> Result<bool, JsonRpcError> {
        self.db
            .set_server_enabled(address, enabled)
            .map_err(|e| JsonRpcError::business(format!("DB error updating server status: {e}")))
    }

    pub fn delete_server(&mut self, address: &str) -> Result<bool, JsonRpcError> {
        self.db
            .delete_server(address)
            .map_err(|e| JsonRpcError::business(format!("DB error deleting server: {e}")))
    }

    pub fn clear_servers(&mut self) -> Result<usize, JsonRpcError> {
        self.db
            .clear_servers()
            .map_err(|e| JsonRpcError::business(format!("DB error clearing servers: {e}")))
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn make_snapshot(dl: &Download, ranges: &RangeSet, source_count: u32) -> DownloadSnapshot {
    let received_bytes = ranges.covered_len();
    let progress = if dl.total_size > 0 {
        (received_bytes as f64 / dl.total_size as f64).min(1.0)
    } else {
        0.0
    };

    let verified_bytes = if dl.status == DownloadStatus::Completed {
        dl.total_size
    } else {
        0
    };

    DownloadSnapshot {
        id: dl.id,
        file_name: dl.display_name.clone(),
        total_bytes: dl.total_size,
        received_bytes,
        verified_bytes,
        progress,
        status: dl.status.as_str().to_string(),
        priority: "normal".to_string(),
        source_count,
        download_rate_bps: 0,
        uploaded_bytes: 0,
        created_at_unix_ms: dl.created_at,
        updated_at_unix_ms: dl.updated_at,
        category_id: dl.category_id.clone(),
    }
}
