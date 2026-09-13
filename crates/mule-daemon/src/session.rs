use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

use crate::error::DaemonError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DaemonSession {
    pub protocol_version: u32,
    pub instance_id: Uuid,
    pub pid: u32,
    pub endpoint: String,
    pub started_at_unix_ms: i64,
}

impl DaemonSession {
    pub fn new(protocol_version: u32, instance_id: Uuid, endpoint: impl Into<String>) -> Self {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        Self {
            protocol_version,
            instance_id,
            pid: std::process::id(),
            endpoint: endpoint.into(),
            started_at_unix_ms: now_ms,
        }
    }

    pub fn write_atomic(&self, data_dir: &Path) -> Result<(), DaemonError> {
        std::fs::create_dir_all(data_dir)?;
        let session_path = data_dir.join("session.json");
        let tmp_path = data_dir.join("session.json.tmp");

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| DaemonError::Session(format!("Failed to serialize session: {e}")))?;

        std::fs::write(&tmp_path, json)?;

        // On Windows, rename fails if destination exists, so safely remove it first
        if session_path.exists() {
            let _ = std::fs::remove_file(&session_path);
        }
        std::fs::rename(&tmp_path, &session_path)?;

        Ok(())
    }

    pub fn read(data_dir: &Path) -> Result<Self, DaemonError> {
        let session_path = data_dir.join("session.json");
        let data = std::fs::read_to_string(&session_path)?;
        let session: Self = serde_json::from_str(&data)
            .map_err(|e| DaemonError::Session(format!("Invalid session file format: {e}")))?;
        Ok(session)
    }

    pub fn remove(data_dir: &Path) {
        let session_path = data_dir.join("session.json");
        let _ = std::fs::remove_file(session_path);
        let tmp_path = data_dir.join("session.json.tmp");
        let _ = std::fs::remove_file(tmp_path);
    }

    pub fn write_token(data_dir: &Path, token: &str) -> Result<(), DaemonError> {
        std::fs::create_dir_all(data_dir)?;
        let token_path = data_dir.join("auth.token");
        std::fs::write(token_path, token.trim())?;
        Ok(())
    }

    pub fn read_token(data_dir: &Path) -> Option<String> {
        let token_path = data_dir.join("auth.token");
        std::fs::read_to_string(token_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}
