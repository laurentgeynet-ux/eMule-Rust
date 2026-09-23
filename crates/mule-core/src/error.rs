// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("invalid range [{start}, {end}): {reason}")]
    InvalidRange {
        start: u64,
        end: u64,
        reason: &'static str,
    },

    #[error("storage I/O error: {0}")]
    Storage(String),

    #[error("corrupted part #{part_index}: {reason}")]
    CorruptedPart { part_index: u32, reason: String },

    #[error("database error: {0}")]
    Database(String),

    #[error("target file already exists: {}", .0.display())]
    TargetFileExists(PathBuf),

    #[error("incomplete download: received {received}/{total} bytes")]
    IncompleteDownload { received: u64, total: u64 },

    #[error("download not found: {0}")]
    DownloadNotFound(String),
}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        CoreError::Storage(err.to_string())
    }
}

impl From<rusqlite::Error> for CoreError {
    fn from(err: rusqlite::Error) -> Self {
        CoreError::Database(err.to_string())
    }
}
