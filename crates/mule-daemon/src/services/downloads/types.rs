use mule_core::{Download, DownloadStatus, PartFile, RangeSet};
use mule_protocol::ed2k::{PartAvailabilityMask, ED2K_PART_SIZE};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use uuid::Uuid;

use serde::{Deserialize, Serialize};

pub const MAX_IMPORT_TOTAL_SIZE: u64 = 1024 * 1024 * 1024 * 1024; // 1 TB safety ceiling
pub const MAX_IMPORT_PART_COUNT: u32 = 65_536; // 65,536 parts ceiling

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSnapshot {
    pub id: Uuid,
    pub file_name: String,
    pub total_bytes: u64,
    pub received_bytes: u64,
    pub verified_bytes: u64,
    pub progress: f64,
    pub status: String,
    pub priority: String,
    pub source_count: u32,
    pub download_rate_bps: u64,
    pub uploaded_bytes: u64,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
    pub category_id: Option<String>,
    /// Seconds remaining at the current rate (None when stalled/done).
    #[serde(default)]
    pub eta_seconds: Option<u64>,
    /// eD2K file hash (hex) — needed by clients to query peer sources
    /// without an extra `downloads.get` round-trip.
    #[serde(default)]
    pub ed2k_hash: String,
    /// Per-part progress summary for the UI availability bar. In list
    /// snapshots `is_verified` is only exact for completed downloads (all
    /// parts verified); `downloads.get` reports the authoritative flags.
    #[serde(default)]
    pub parts: Vec<PartProgressSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PartProgressSummary {
    pub part_index: u32,
    pub start_offset: u64,
    pub end_offset: u64,
    pub size: u64,
    pub received_bytes: u64,
    pub is_verified: bool,
    pub state: String, // "verified", "completed", "downloading", "missing"
    /// Number of known sources whose availability mask covers this part
    /// (eMule's blue-shade/red coloring: 0 = no source has it → stuck part).
    pub available_source_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDetail {
    #[serde(flatten)]
    pub snapshot: DownloadSnapshot,
    pub ed2k_hash: String,
    pub aich_root: Option<String>,
    pub ed2k_link: String,
    pub temp_path: String,
    pub target_path: String,
    pub remaining_bytes: u64,
    pub average_rate_bps: u64,
    pub eta_seconds: Option<u64>,
    pub connection_attempts: u32,
    pub last_error: Option<String>,
    pub last_status_change_unix_ms: i64,
    pub parts: Vec<PartProgressSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PartMetInspection {
    pub file_name: String,
    pub total_size: u64,
    pub ed2k_hash: String,
    pub aich_hash: Option<String>,
    pub part_count: u32,
    pub part_met_path: String,
    pub part_path: Option<String>,
    pub part_file_exists: bool,
    pub part_file_size: Option<u64>,
    pub estimated_transferred_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportPartMetResult {
    pub download: DownloadSnapshot,
    pub total_parts: u32,
    pub verified_parts: u32,
    pub corrupted_parts: u32,
    pub transferred_bytes: u64,
    pub total_size: u64,
    #[serde(default)]
    pub report: Option<String>,
}

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
pub struct ActiveSource {
    pub source_id: String,
    pub _addr: SocketAddr,
    pub mask: PartAvailabilityMask,
    pub last_seen_unix_ms: i64,
}

pub struct ActiveDownload {
    pub download: Download,
    pub ranges: RangeSet,
    pub part_file: Option<PartFile>,
    pub sources: HashMap<SocketAddr, ActiveSource>,
    pub connection_attempts: u32,
    pub resume_count: u32,
    pub last_error: Option<String>,
    pub last_status_change_unix_ms: i64,
    pub session_start_ms: i64,
    pub session_bytes_start: u64,
    pub current_rate_bps: u64,
    pub last_rate_update: Option<Instant>,
    pub source_count: u32,
    pub verification_in_progress: bool,
    /// Parts that failed MD4 verification and await AICH recovery data
    /// (only populated when a trusted AICH root is known):
    /// `(part_index, enqueued_at)`.
    pub aich_pending: VecDeque<(u32, Instant)>,
    /// Parts for which an AICH recovery request was already sent:
    /// `part -> (sent_at, attempts)`.
    pub aich_requested: HashMap<u32, (Instant, u32)>,
    /// Untrusted AICH master-hash votes: root hash -> masked source IPs
    /// (eMule `CAICHUntrustedHash`; IP masked with `0x00F0FFFF`).
    pub aich_votes: HashMap<[u8; 20], HashSet<u32>>,
}

impl ActiveDownload {
    pub fn snapshot(&self) -> DownloadSnapshot {
        let effective_rate = if let Some(last_update) = self.last_rate_update {
            if last_update.elapsed() > Duration::from_secs(2) {
                0
            } else {
                self.current_rate_bps
            }
        } else {
            0
        };

        let mut snap = make_snapshot(
            &self.download,
            &self.ranges,
            self.source_count.max(self.sources.len() as u32),
            effective_rate,
        );
        // Verified part flags live in SQLite; the list snapshot only knows
        // "all verified" for completed downloads. `downloads.get` re-reads
        // the authoritative set for the detail view.
        let verified_parts: HashSet<u32> =
            if self.download.status == DownloadStatus::Completed {
                let pc = self.download.total_size.div_ceil(ED2K_PART_SIZE as u64).max(1) as u32;
                (0..pc).collect()
            } else {
                HashSet::new()
            };
        snap.parts = compute_part_summaries(
            self.download.total_size,
            &self.ranges,
            self.sources.values().map(|s| &s.mask),
            &verified_parts,
        );
        snap
    }
}

/// Computes the per-part progress summary shared by `downloads.list`
/// snapshots and `downloads.get` details (eMule `CPartFile` part states).
pub fn compute_part_summaries<'a>(
    total_size: u64,
    ranges: &RangeSet,
    source_masks: impl Iterator<Item = &'a PartAvailabilityMask>,
    verified_parts: &HashSet<u32>,
) -> Vec<PartProgressSummary> {
    let part_size_const = ED2K_PART_SIZE as u64;
    let part_count = if total_size == 0 {
        1
    } else {
        total_size.div_ceil(part_size_const) as u32
    };

    let all_ranges = ranges.ranges();
    let masks: Vec<&PartAvailabilityMask> = source_masks.collect();
    let mut parts = Vec::with_capacity(part_count as usize);

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

        let is_verified = verified_parts.contains(&i);
        let state = if is_verified {
            "verified"
        } else if received_in_part >= size && size > 0 {
            "completed"
        } else if received_in_part > 0 {
            "downloading"
        } else {
            "missing"
        };

        let available_source_count = masks
            .iter()
            .filter(|m| u16::try_from(i).is_ok_and(|p| m.has_part(p)))
            .count() as u32;

        parts.push(PartProgressSummary {
            part_index: i,
            start_offset,
            end_offset,
            size,
            received_bytes: received_in_part,
            is_verified,
            state: state.to_string(),
            available_source_count,
        });
    }

    parts
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn make_snapshot(
    dl: &Download,
    ranges: &RangeSet,
    source_count: u32,
    download_rate_bps: u64,
) -> DownloadSnapshot {
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

    let remaining = dl.total_size.saturating_sub(received_bytes);
    let eta_seconds = if download_rate_bps > 0 && remaining > 0 {
        Some(remaining / download_rate_bps)
    } else {
        None
    };

    DownloadSnapshot {
        id: dl.id,
        file_name: dl.display_name.clone(),
        total_bytes: dl.total_size,
        received_bytes,
        verified_bytes,
        progress,
        status: dl.status.as_str().to_string(),
        priority: dl.priority.clone(),
        source_count,
        download_rate_bps,
        uploaded_bytes: 0,
        created_at_unix_ms: dl.created_at,
        updated_at_unix_ms: dl.updated_at,
        category_id: dl.category_id.clone(),
        eta_seconds,
        ed2k_hash: dl.ed2k_hash.to_string(),
        parts: Vec::new(),
    }
}
