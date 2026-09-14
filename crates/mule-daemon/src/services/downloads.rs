use mule_core::{AllocationPolicy, Database, Download, DownloadStatus, PartFile, RangeSet};
use mule_format::Ed2kFileLink;
use mule_protocol::ed2k::PartAvailabilityMask;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::JsonRpcError;
use crate::rpc::events::DownloadSnapshot;

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
}

pub struct DownloadService {
    db: Database,
    temp_dir: PathBuf,
    active: HashMap<Uuid, ActiveDownload>,
}

impl DownloadService {
    pub fn new(db_path: &Path, temp_dir: PathBuf) -> Result<Self, JsonRpcError> {
        let db = Database::open(db_path)
            .map_err(|e| JsonRpcError::business(format!("Failed to open DB: {e}")))?;
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| JsonRpcError::business(format!("Failed to create temp dir: {e}")))?;

        let existing = db.get_all_downloads().unwrap_or_default();
        let mut active = HashMap::new();
        for dl in existing {
            let ranges = db.load_received_ranges(&dl.id).unwrap_or_default();
            let part_file =
                PartFile::open_or_create(&dl.temp_path, dl.total_size, AllocationPolicy::None).ok();
            active.insert(
                dl.id,
                ActiveDownload {
                    download: dl,
                    ranges,
                    part_file,
                    sources: HashMap::new(),
                },
            );
        }

        Ok(Self {
            db,
            temp_dir,
            active,
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
        })
    }

    pub fn add_ed2k_link(
        &mut self,
        link_str: &str,
        target_dir: &Path,
        policy: AllocationPolicy,
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

        self.active.insert(
            id,
            ActiveDownload {
                download,
                ranges,
                part_file: Some(part_file),
                sources: HashMap::new(),
            },
        );

        Ok(snapshot)
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

    pub fn pause_download(&mut self, id: &Uuid) -> Result<DownloadSnapshot, JsonRpcError> {
        let active = self
            .active
            .get_mut(id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

        active.download.status = DownloadStatus::Paused;
        active.download.updated_at = now_ms();
        self.db
            .update_status(id, DownloadStatus::Paused)
            .map_err(|e| JsonRpcError::business(format!("DB error: {e}")))?;

        Ok(make_snapshot(&active.download, &active.ranges, active.sources.len() as u32))
    }

    pub fn resume_download(&mut self, id: &Uuid) -> Result<DownloadSnapshot, JsonRpcError> {
        let active = self
            .active
            .get_mut(id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

        active.download.status = DownloadStatus::Downloading;
        active.download.updated_at = now_ms();
        self.db
            .update_status(id, DownloadStatus::Downloading)
            .map_err(|e| JsonRpcError::business(format!("DB error: {e}")))?;

        Ok(make_snapshot(&active.download, &active.ranges, active.sources.len() as u32))
    }

    pub fn remove_download(&mut self, id: &Uuid, delete_files: bool) -> Result<(), JsonRpcError> {
        let mut temp_path_to_delete = None;
        let was_active = if let Some(mut active) = self.active.remove(id) {
            let _ = active.part_file.take(); // Explicitly drop/close file handle first (vital on Windows)
            if delete_files {
                temp_path_to_delete = Some(active.download.temp_path.clone());
            }
            true
        } else {
            false
        };

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
        let active = self
            .active
            .get_mut(id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

        active.download.status = status;
        active.download.updated_at = now_ms();
        self.db
            .update_status(id, status)
            .map_err(|e| JsonRpcError::business(format!("DB error: {e}")))?;

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
        let active = self
            .active
            .get_mut(id)
            .ok_or_else(|| JsonRpcError::business(format!("Download {id} not found")))?;

        // Close part file handle first so file can be moved cleanly
        let _ = active.part_file.take();

        mule_core::finalize_file(
            &active.download.temp_path,
            &active.download.target_path,
            mule_core::CollisionStrategy::Overwrite,
        )
        .map_err(|e| JsonRpcError::business(format!("Finalize error: {e}")))?;

        active.download.status = DownloadStatus::Completed;
        active.download.updated_at = now_ms();
        self.db
            .update_status(id, DownloadStatus::Completed)
            .map_err(|e| JsonRpcError::business(format!("DB error: {e}")))?;

        Ok(make_snapshot(&active.download, &active.ranges, active.sources.len() as u32))
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

    DownloadSnapshot {
        id: dl.id,
        file_name: dl.display_name.clone(),
        total_bytes: dl.total_size,
        received_bytes,
        verified_bytes: 0,
        progress,
        status: dl.status.as_str().to_string(),
        priority: "normal".to_string(),
        source_count,
        download_rate_bps: 0,
        uploaded_bytes: 0,
        created_at_unix_ms: dl.created_at,
        updated_at_unix_ms: dl.updated_at,
    }
}
