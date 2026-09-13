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
        }
    }
}
