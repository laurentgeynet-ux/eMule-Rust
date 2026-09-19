use serde::{Deserialize, Serialize};

pub use crate::services::downloads::types::{
    DownloadDetail, DownloadSnapshot, ImportPartMetResult, PartMetInspection, PartProgressSummary,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadUpdatedEvent {
    pub sequence: u64,
    pub download: DownloadSnapshot,
}
