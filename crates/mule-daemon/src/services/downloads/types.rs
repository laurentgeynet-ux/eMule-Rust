use mule_core::{Download, DownloadStatus, PartFile, RangeSet};
use mule_protocol::ed2k::PartAvailabilityMask;
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

        make_snapshot(
            &self.download,
            &self.ranges,
            self.source_count.max(self.sources.len() as u32),
            effective_rate,
        )
    }
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
        download_rate_bps,
        uploaded_bytes: 0,
        created_at_unix_ms: dl.created_at,
        updated_at_unix_ms: dl.updated_at,
        category_id: dl.category_id.clone(),
    }
}
