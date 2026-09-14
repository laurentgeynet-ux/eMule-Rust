//! Persistent storage, sparse .part files, and crash-resilient resume state for eD2K transfers.
//!
//! Tier 3.5 formalizes physical file persistence and resume invariants:
//! - Exact-sized `.part` file pre-allocation / sparse representation on disk.
//! - Companion `.resume.json` state journal recording completed byte intervals and total bytes received.
//! - Atomic state persistence via temp file (`.tmp`) and rename to guarantee crash-consistency.
//! - Resumption from partial disk state: reloads `.resume.json`, verifies file hash and size consistency.
//! - Incremental chunk writes into `.part` file with strict duplicate / overlap protection.
//! - In-disk part integrity verification: streams completed part bytes directly into local MD4 hasher
//!   to verify part checksum against fixture/oracle without needing to hold the full part in RAM.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use md4::{Digest, Md4};
use serde::{Deserialize, Serialize};

use super::packet::{Ed2kBlockRange, Ed2kSendingPart};
use super::sequencer::{Ed2kBlockSink, Ed2kDownloadSequencer, Ed2kSequencerError};

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

/// Persistent physical storage sink for assembling downloaded eD2K chunks into a `.part` file.
///
/// Ensures crash consistency with an atomic companion `.resume.json` state journal.
#[derive(Debug)]
pub struct Ed2kPartFileSink {
    part_file_path: PathBuf,
    resume_file_path: PathBuf,
    file: File,
    file_hash: [u8; 16],
    file_size: u64,
    resume_state: Ed2kPartResumeState,
}

impl Ed2kPartFileSink {
    /// Creates a new `.part` file and associated `.resume.json` journal.
    ///
    /// Truncates / pre-allocates the file to the target `file_size`.
    pub fn create_new(
        part_file_path: PathBuf,
        file_hash: [u8; 16],
        file_size: u64,
    ) -> Result<Self, Ed2kSequencerError> {
        let resume_file_path = PathBuf::from(format!("{}.resume.json", part_file_path.display()));

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&part_file_path)
            .map_err(|e| {
                Ed2kSequencerError::Storage(format!(
                    "Failed to create part file '{}': {e}",
                    part_file_path.display()
                ))
            })?;

        file.set_len(file_size).map_err(|e| {
            Ed2kSequencerError::Storage(format!(
                "Failed to set part file length to {file_size}: {e}"
            ))
        })?;

        let resume_state = Ed2kPartResumeState::new(file_hash, file_size);
        resume_state.save_to_file(&resume_file_path)?;

        Ok(Self {
            part_file_path,
            resume_file_path,
            file,
            file_hash,
            file_size,
            resume_state,
        })
    }

    /// Opens an existing `.part` file or resumes an existing download session.
    ///
    /// If both `.part` and `.resume.json` exist, reloads and validates the resume state.
    /// If either does not exist, creates a clean session via `create_new`.
    pub fn open_or_resume(
        part_file_path: PathBuf,
        file_hash: [u8; 16],
        file_size: u64,
    ) -> Result<Self, Ed2kSequencerError> {
        let resume_file_path = PathBuf::from(format!("{}.resume.json", part_file_path.display()));

        if part_file_path.exists() && resume_file_path.exists() {
            let resume_state = Ed2kPartResumeState::load_from_file(&resume_file_path)?;

            let expected_hex = hex::encode(file_hash);
            if resume_state.file_hash != expected_hex {
                return Err(Ed2kSequencerError::HashMismatch {
                    expected: expected_hex,
                    actual: resume_state.file_hash,
                });
            }

            if resume_state.file_size != file_size {
                return Err(Ed2kSequencerError::Storage(format!(
                    "File size mismatch: expected {file_size}, found in resume {}",
                    resume_state.file_size
                )));
            }

            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&part_file_path)
                .map_err(|e| {
                    Ed2kSequencerError::Storage(format!(
                        "Failed to open part file '{}': {e}",
                        part_file_path.display()
                    ))
                })?;

            let disk_len = file
                .metadata()
                .map_err(|e| Ed2kSequencerError::Storage(e.to_string()))?
                .len();
            if disk_len < file_size {
                file.set_len(file_size).map_err(|e| {
                    Ed2kSequencerError::Storage(format!("Failed to expand part file to {file_size}: {e}"))
                })?;
            }

            Ok(Self {
                part_file_path,
                resume_file_path,
                file,
                file_hash,
                file_size,
                resume_state,
            })
        } else {
            Self::create_new(part_file_path, file_hash, file_size)
        }
    }

    /// Path to the on-disk `.part` file.
    pub fn part_file_path(&self) -> &Path {
        &self.part_file_path
    }

    /// 16-byte MD4 file hash.
    pub fn file_hash(&self) -> [u8; 16] {
        self.file_hash
    }

    /// Total expected file size in bytes.
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Path to the companion `.resume.json` state journal.
    pub fn resume_file_path(&self) -> &Path {
        &self.resume_file_path
    }

    /// Reference to the active in-memory resume journal state.
    pub fn resume_state(&self) -> &Ed2kPartResumeState {
        &self.resume_state
    }

    /// Returns the list of completed half-open byte intervals `[start, end)`.
    pub fn completed_ranges(&self) -> &[(u64, u64)] {
        &self.resume_state.completed_ranges
    }

    /// Returns the total cumulative bytes received and written to disk.
    pub fn total_bytes_received(&self) -> u64 {
        self.resume_state.total_bytes_received
    }

    /// Checks if any byte in `[start, end)` overlaps with already completed intervals.
    pub fn has_overlap(&self, start: u64, end: u64) -> bool {
        self.resume_state
            .completed_ranges
            .iter()
            .any(|&(s, e)| s < end && start < e)
    }

    /// Flushes file buffers and syncs both the data file and resume state to disk.
    pub fn flush_and_sync(&mut self) -> Result<(), Ed2kSequencerError> {
        self.file
            .flush()
            .map_err(|e| Ed2kSequencerError::Storage(format!("Flush error: {e}")))?;
        self.file
            .sync_all()
            .map_err(|e| Ed2kSequencerError::Storage(format!("Sync error: {e}")))?;
        self.resume_state.save_to_file(&self.resume_file_path)?;
        Ok(())
    }

    /// Returns true if the specified part index is completely received on disk.
    pub fn is_part_complete(
        &self,
        part_index: u16,
        sequencer: &Ed2kDownloadSequencer,
    ) -> Result<bool, Ed2kSequencerError> {
        let (p_start, p_end) = sequencer.part_byte_bounds(part_index)?;
        Ok(self.has_range(p_start, p_end))
    }

    /// Computes the 16-byte MD4 checksum of the specified part by streaming bytes directly
    /// from disk in 64 KiB chunks into the hasher.
    ///
    /// Requires that the part be completely downloaded and present in the sink.
    pub fn compute_part_md4(
        &mut self,
        part_index: u16,
        sequencer: &Ed2kDownloadSequencer,
    ) -> Result<[u8; 16], Ed2kSequencerError> {
        let (p_start, p_end) = sequencer.part_byte_bounds(part_index)?;
        if !self.has_range(p_start, p_end) {
            return Err(Ed2kSequencerError::Storage(format!(
                "Part {part_index} [{p_start}..{p_end}) is not fully downloaded on disk"
            )));
        }

        self.file
            .flush()
            .map_err(|e| Ed2kSequencerError::Storage(e.to_string()))?;
        self.file
            .seek(SeekFrom::Start(p_start))
            .map_err(|e| Ed2kSequencerError::Storage(format!("Seek error: {e}")))?;

        let mut hasher = Md4::new();
        let mut remaining = p_end - p_start;
        let mut buffer = [0u8; 65536];

        while remaining > 0 {
            let to_read = std::cmp::min(remaining, buffer.len() as u64) as usize;
            self.file
                .read_exact(&mut buffer[..to_read])
                .map_err(|e| {
                    Ed2kSequencerError::Storage(format!(
                        "Failed reading part {part_index} from disk: {e}"
                    ))
                })?;
            hasher.update(&buffer[..to_read]);
            remaining -= to_read as u64;
        }

        let digest: [u8; 16] = hasher.finalize().into();
        Ok(digest)
    }

    /// Validates and writes an incoming chunk into the `.part` file, updating and syncing the `.resume.json` journal.
    pub fn write_block(
        &mut self,
        sequencer: &Ed2kDownloadSequencer,
        part: &Ed2kSendingPart,
    ) -> Result<Ed2kBlockRange, Ed2kSequencerError> {
        let range = sequencer.validate_sending_part(part)?;
        let start = range.start as u64;
        let end = range.end as u64;

        if self.has_overlap(start, end) {
            return Err(Ed2kSequencerError::DuplicateBlock { start, end });
        }

        self.file
            .seek(SeekFrom::Start(start))
            .map_err(|e| Ed2kSequencerError::Storage(format!("Seek error: {e}")))?;
        self.file
            .write_all(&part.data)
            .map_err(|e| Ed2kSequencerError::Storage(format!("Write error: {e}")))?;
        self.file
            .flush()
            .map_err(|e| Ed2kSequencerError::Storage(format!("Flush error: {e}")))?;

        self.resume_state.record_range(start, end);
        self.resume_state.save_to_file(&self.resume_file_path)?;

        Ok(range)
    }

    /// Checks if a byte range `[start, end)` has already been completely received and written to disk.
    pub fn has_range(&self, start: u64, end: u64) -> bool {
        Ed2kDownloadSequencer::is_range_in_list(start, end, &self.resume_state.completed_ranges)
    }
}

impl Ed2kBlockWriter for Ed2kPartFileSink {
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
