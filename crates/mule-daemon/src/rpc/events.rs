use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadUpdatedEvent {
    pub sequence: u64,
    pub download: DownloadSnapshot,
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

