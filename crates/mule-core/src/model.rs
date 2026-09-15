use mule_format::{AichHash, Ed2kHash};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    Queued,
    WaitingForSources,
    Downloading,
    Verifying,
    Paused,
    Completing,
    Completed,
    Failed,
    Removed,
}

impl DownloadStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::WaitingForSources => "waiting_for_sources",
            Self::Downloading => "downloading",
            Self::Verifying => "verifying",
            Self::Paused => "paused",
            Self::Completing => "completing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Removed => "removed",
        }
    }
}

impl std::str::FromStr for DownloadStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "waiting_for_sources" => Self::WaitingForSources,
            "downloading" => Self::Downloading,
            "verifying" => Self::Verifying,
            "paused" => Self::Paused,
            "completing" => Self::Completing,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "removed" => Self::Removed,
            _ => Self::Queued,
        })
    }
}

/// In-memory representation of an active download task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub id: Uuid,
    pub display_name: String,
    pub total_size: u64,
    pub ed2k_hash: Ed2kHash,
    pub aich_root: Option<AichHash>,
    pub target_path: PathBuf,
    pub temp_path: PathBuf,
    pub status: DownloadStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub category_id: Option<String>,
}

impl Download {
    pub fn new(
        display_name: impl Into<String>,
        total_size: u64,
        ed2k_hash: Ed2kHash,
        aich_root: Option<AichHash>,
        target_path: PathBuf,
        temp_path: PathBuf,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            id: Uuid::new_v4(),
            display_name: display_name.into(),
            total_size,
            ed2k_hash,
            aich_root,
            target_path,
            temp_path,
            status: DownloadStatus::Queued,
            created_at: now,
            updated_at: now,
            category_id: None,
        }
    }
}

/// Category metadata for organizing local downloads.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadCategory {
    pub id: String,
    pub name: String,
    pub color: String, // Hex "#RRGGBB"
    pub sort_order: i32,
    pub default_destination: Option<String>,
    pub default_priority: Option<String>, // "low", "normal", "high"
    pub created_at: i64,
    pub updated_at: i64,
}

impl DownloadCategory {
    pub fn new(
        name: impl Into<String>,
        color: impl Into<String>,
        sort_order: i32,
        default_destination: Option<String>,
        default_priority: Option<String>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            color: color.into(),
            sort_order,
            default_destination,
            default_priority,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Single historical event entry for a download transition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub download_id: String,
    pub file_name: String,
    pub total_size: u64,
    pub transferred_bytes: u64,
    pub ed2k_hash: String,
    pub event_type: String, // "completed", "failed", "removed", "verified_pending_promotion", "resumed"
    pub status: String,
    pub duration_secs: u64,
    pub connection_attempts: u32,
    pub resume_count: u32,
    pub verification_result: String, // "verified_ok", "hash_mismatch", "unverified", "skipped"
    pub error_message: Option<String>,
    pub timestamp_unix_ms: i64,
}

/// Query filter for history list.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFilter {
    pub event_type: Option<String>,
    pub status: Option<String>,
    pub from_timestamp_ms: Option<i64>,
    pub to_timestamp_ms: Option<i64>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Aggregated history and transfer statistics.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStats {
    pub total_completed_count: u64,
    pub total_completed_bytes: u64,
    pub total_failed_count: u64,
    pub total_resumes_count: u64,
    pub total_integrity_failures: u64,
    // Time windows
    pub session_completed_bytes: u64,
    pub session_completed_count: u64,
    pub last_24h_completed_bytes: u64,
    pub last_24h_completed_count: u64,
    pub last_7d_completed_bytes: u64,
    pub last_7d_completed_count: u64,
}

/// Filter status for an eD2K server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerFilterStatus {
    Allowed,
    BlockedByFilter,
    Disabled,
}

impl ServerFilterStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::BlockedByFilter => "blocked_by_filter",
            Self::Disabled => "disabled",
        }
    }
}

impl std::str::FromStr for ServerFilterStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "blocked_by_filter" => Self::BlockedByFilter,
            "disabled" => Self::Disabled,
            _ => Self::Allowed,
        })
    }
}

/// Persistent record of an eD2K server in local storage.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Ed2kServer {
    pub address: String, // "ip:port" as primary key
    pub ip: String,
    pub port: u16,
    pub name: String,
    pub description: String,
    pub ping: u32,
    pub preference: u32,
    pub failed_count: u32,
    pub dyn_ip: Option<String>,
    pub version: Option<String>,
    pub filter_status: ServerFilterStatus,
    pub last_seen_unix_ms: Option<i64>,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
}

