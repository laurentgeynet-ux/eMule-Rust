// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Storage types, status enums, and resume state tracking for eD2K.

use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ed2k::packet::{Ed2kBlockRange, Ed2kSendingPart};
use crate::ed2k::sequencer::{Ed2kBlockSink, Ed2kDownloadSequencer, Ed2kSequencerError};

/// Common trait for byte sinks capable of validating and recording downloaded eD2K chunks.
pub trait Ed2kBlockWriter {
    /// Validates and writes an incoming `Ed2kSendingPart` chunk into the sink.
    fn write_block(
        &mut self,
        sequencer: &Ed2kDownloadSequencer,
        part: &Ed2kSendingPart,
    ) -> Result<Ed2kBlockRange, Ed2kSequencerError>;

    /// Returns true if the half-open byte range `[start, end)` is completely covered.
    fn has_range(&self, start: u64, end: u64) -> bool;
}

impl Ed2kBlockWriter for Ed2kBlockSink {
    fn write_block(
        &mut self,
        sequencer: &Ed2kDownloadSequencer,
        part: &Ed2kSendingPart,
    ) -> Result<Ed2kBlockRange, Ed2kSequencerError> {
        self.write_block(sequencer, part)
    }

    fn has_range(&self, start: u64, end: u64) -> bool {
        self.has_range(start, end)
    }
}

/// Merges overlapping or adjacent half-open byte ranges `[start, end)`.
pub fn merge_intervals(mut ranges: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    if ranges.is_empty() {
        return ranges;
    }
    ranges.sort_unstable_by_key(|r| r.0);
    let mut merged: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
    let mut current = ranges[0];

    for r in ranges.into_iter().skip(1) {
        if r.0 <= current.1 {
            current.1 = std::cmp::max(current.1, r.1);
        } else {
            merged.push(current);
            current = r;
        }
    }
    merged.push(current);
    merged
}

/// Lifecycle status of an eD2K download in local storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Ed2kPartDownloadStatus {
    /// In progress: downloading blocks into `.part` file.
    #[default]
    Downloading,
    /// Download complete: all parts verified with eD2K root MD4, ready for local promotion.
    VerifiedPendingPromotion,
    /// Successfully promoted to target download directory; `.part` renamed and journal removed.
    Promoted,
}

/// Persistent journal state saved alongside a `.part` download file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ed2kPartResumeState {
    /// 16-byte MD4 eD2K file hash in lowercase hex format.
    pub file_hash: String,
    /// Total expected file size in bytes.
    pub file_size: u64,
    /// Canonical, merged list of half-open byte intervals `[start, end)` received and written to disk.
    pub completed_ranges: Vec<(u64, u64)>,
    /// Total cumulative payload bytes received and verified.
    pub total_bytes_received: u64,
    /// Current download and promotion status.
    #[serde(default)]
    pub status: Ed2kPartDownloadStatus,
    /// UNIX timestamp of last journal update.
    pub updated_at: u64,
}

impl Ed2kPartResumeState {
    /// Creates a fresh resume state for a given file.
    pub fn new(file_hash: [u8; 16], file_size: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            file_hash: hex::encode(file_hash),
            file_size,
            completed_ranges: Vec::new(),
            total_bytes_received: 0,
            status: Ed2kPartDownloadStatus::Downloading,
            updated_at: now,
        }
    }

    /// Loads and parses a resume state from the specified JSON file.
    pub fn load_from_file(path: &Path) -> Result<Self, Ed2kSequencerError> {
        let data = fs::read(path).map_err(|e| {
            Ed2kSequencerError::Storage(format!(
                "Failed to read resume file '{}': {e}",
                path.display()
            ))
        })?;
        let state: Self = serde_json::from_slice(&data).map_err(|e| {
            Ed2kSequencerError::Storage(format!(
                "Failed to parse resume JSON '{}': {e}",
                path.display()
            ))
        })?;
        Ok(state)
    }

    /// Atomically persists the resume state to `path` via a temporary file.
    pub fn save_to_file(&self, path: &Path) -> Result<(), Ed2kSequencerError> {
        let json_bytes = serde_json::to_vec_pretty(self).map_err(|e| {
            Ed2kSequencerError::Storage(format!("Failed to serialize resume state to JSON: {e}"))
        })?;

        let tmp_path = format!("{}.tmp", path.display());
        let tmp_path = PathBuf::from(tmp_path);

        {
            let mut file = File::create(&tmp_path).map_err(|e| {
                Ed2kSequencerError::Storage(format!(
                    "Failed to create temporary resume file '{}': {e}",
                    tmp_path.display()
                ))
            })?;
            file.write_all(&json_bytes).map_err(|e| {
                Ed2kSequencerError::Storage(format!(
                    "Failed to write to temporary resume file '{}': {e}",
                    tmp_path.display()
                ))
            })?;
            file.sync_all().map_err(|e| {
                Ed2kSequencerError::Storage(format!(
                    "Failed to sync temporary resume file '{}': {e}",
                    tmp_path.display()
                ))
            })?;
        }

        if let Err(e) = fs::rename(&tmp_path, path) {
            if path.exists() {
                let _ = fs::remove_file(path);
                fs::rename(&tmp_path, path).map_err(|e2| {
                    Ed2kSequencerError::Storage(format!(
                        "Failed to atomically rename '{}' to '{}': {e2} (initial error: {e})",
                        tmp_path.display(),
                        path.display()
                    ))
                })?;
            } else {
                return Err(Ed2kSequencerError::Storage(format!(
                    "Failed to rename '{}' to '{}': {e}",
                    tmp_path.display(),
                    path.display()
                )));
            }
        }

        Ok(())
    }

    /// Records a newly written byte interval `[start, end)` and merges contiguous ranges.
    pub fn record_range(&mut self, start: u64, end: u64) {
        if start >= end {
            return;
        }
        self.completed_ranges.push((start, end));
        self.completed_ranges = merge_intervals(std::mem::take(&mut self.completed_ranges));
        self.total_bytes_received = self.completed_ranges.iter().map(|&(s, e)| e - s).sum();
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }
}
