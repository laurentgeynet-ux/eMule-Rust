// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use serde::{Deserialize, Serialize};
use std::io::Write;
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

pub const AUTH_TOKEN_BYTES: usize = 32;
pub const AUTH_TOKEN_HEX_LEN: usize = AUTH_TOKEN_BYTES * 2;

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

    pub fn generate_token() -> String {
        use rand::rngs::OsRng;
        use rand::TryRngCore;
        let mut bytes = [0u8; AUTH_TOKEN_BYTES];
        OsRng
            .try_fill_bytes(&mut bytes)
            .expect("OS CSPRNG must be available");
        hex::encode(bytes)
    }

    pub fn write_atomic(&self, data_dir: &Path) -> Result<(), DaemonError> {
        std::fs::create_dir_all(data_dir)?;
        let session_path = data_dir.join("session.json");
        let tmp_path = data_dir.join("session.json.tmp");

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| DaemonError::Session(format!("Failed to serialize session: {e}")))?;

        // 1. Write completely to tmp file
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(json.as_bytes())?;
        // 2. Ensure data is flushed and synced to disk
        file.sync_all()?;
        // 3. Close the file handle before renaming
        drop(file);

        // 4. Atomically replace target file with retries for Windows sharing violations
        atomic_replace(&tmp_path, &session_path)?;

        Ok(())
    }

    pub fn read(data_dir: &Path) -> Result<Self, DaemonError> {
        let session_path = data_dir.join("session.json");
        let data = std::fs::read_to_string(&session_path)?;
        if data.trim().is_empty() {
            return Err(DaemonError::Session("Session file is empty".to_string()));
        }
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
        let trimmed = token.trim();
        if trimmed.len() != AUTH_TOKEN_HEX_LEN {
            return Err(DaemonError::Session(format!(
                "Security error: auth token must be exactly {AUTH_TOKEN_HEX_LEN} hex characters (32 random bytes)"
            )));
        }
        if !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DaemonError::Session(
                "Security error: auth token must contain only hexadecimal characters".to_string(),
            ));
        }

        std::fs::create_dir_all(data_dir)?;
        let token_path = data_dir.join("auth.token");
        let tmp_path = data_dir.join("auth.token.tmp");

        // Reject if target path is a symlink or reparse point
        if let Ok(meta) = std::fs::symlink_metadata(&token_path) {
            if meta.file_type().is_symlink() {
                return Err(DaemonError::Session(
                    "Security error: auth.token cannot be a symlink or reparse point".to_string(),
                ));
            }
        }

        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(trimmed.as_bytes())?;
        file.sync_all()?;
        drop(file);

        atomic_replace(&tmp_path, &token_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600));
        }

        Ok(())
    }

    pub fn read_token(data_dir: &Path) -> Option<String> {
        let token_path = data_dir.join("auth.token");
        if let Ok(meta) = std::fs::symlink_metadata(&token_path) {
            if meta.file_type().is_symlink() {
                return None;
            }
        }
        std::fs::read_to_string(token_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| s.len() == AUTH_TOKEN_HEX_LEN && s.chars().all(|c| c.is_ascii_hexdigit()))
    }
}

/// Robust file replacement with retries to handle temporary sharing violations
/// on Windows (antivirus, file indexer, or concurrent reader).
fn atomic_replace(tmp_path: &Path, dest_path: &Path) -> Result<(), DaemonError> {
    let mut last_err = None;
    for _ in 0..10 {
        if dest_path.exists() {
            let _ = std::fs::remove_file(dest_path);
        }
        match std::fs::rename(tmp_path, dest_path) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }
    if let Some(e) = last_err {
        Err(e.into())
    } else {
        Ok(())
    }
}
