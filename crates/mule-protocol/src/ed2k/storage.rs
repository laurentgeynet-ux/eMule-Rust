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

/// Persistent physical storage sink for assembling downloaded eD2K chunks into a `.part` file.
///
/// Ensures crash consistency with an atomic companion `.resume.json` state journal.
#[derive(Debug)]
pub struct Ed2kPartFileSink {
    part_file_path: PathBuf,
    resume_file_path: PathBuf,
    file: Option<File>,
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
            file: Some(file),
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
                file: Some(file),
                file_hash,
                file_size,
                resume_state,
            })
        } else {
            Self::create_new(part_file_path, file_hash, file_size)
        }
    }

    /// Internal helper to borrow the underlying mutable file handle.
    fn get_file_mut(&mut self) -> Result<&mut File, Ed2kSequencerError> {
        self.file.as_mut().ok_or_else(|| {
            Ed2kSequencerError::Storage(
                "Part file handle has already been closed or promoted".to_string(),
            )
        })
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

    /// Current download and promotion status.
    pub fn status(&self) -> Ed2kPartDownloadStatus {
        self.resume_state.status
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
        if let Some(file) = self.file.as_mut() {
            file.flush()
                .map_err(|e| Ed2kSequencerError::Storage(format!("Flush error: {e}")))?;
            file.sync_all()
                .map_err(|e| Ed2kSequencerError::Storage(format!("Sync error: {e}")))?;
        }
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

    /// Returns true if the entire file `[0, sequencer.file_size())` is completely received on disk.
    pub fn is_file_complete(&self, sequencer: &Ed2kDownloadSequencer) -> bool {
        self.has_range(0, sequencer.file_size())
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

        let file = self.get_file_mut()?;
        file.flush()
            .map_err(|e| Ed2kSequencerError::Storage(e.to_string()))?;
        file.seek(SeekFrom::Start(p_start))
            .map_err(|e| Ed2kSequencerError::Storage(format!("Seek error: {e}")))?;

        let mut hasher = Md4::new();
        let mut remaining = p_end - p_start;
        let mut buffer = [0u8; 65536];

        while remaining > 0 {
            let to_read = std::cmp::min(remaining, buffer.len() as u64) as usize;
            file.read_exact(&mut buffer[..to_read])
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

    /// Computes the complete 16-byte eD2K root hash of the file by streaming parts from disk.
    ///
    /// - For single-part files (total_parts <= 1), this is the MD4 of part 0.
    /// - For multi-part files (total_parts > 1), each part is hashed with MD4, and the concatenated
    ///   part digests (16 * total_parts bytes) are hashed with MD4 to produce the root file hash.
    ///
    /// Requires that all parts be completely downloaded and present in the sink.
    pub fn compute_file_ed2k_hash(
        &mut self,
        sequencer: &Ed2kDownloadSequencer,
    ) -> Result<[u8; 16], Ed2kSequencerError> {
        if !self.is_file_complete(sequencer) {
            return Err(Ed2kSequencerError::Storage(format!(
                "Cannot compute file root hash: file is incomplete ({}/{} bytes received)",
                self.total_bytes_received(),
                self.file_size
            )));
        }

        let total_parts = sequencer.total_parts();
        if total_parts <= 1 {
            self.compute_part_md4(0, sequencer)
        } else {
            let mut root_hasher = Md4::new();
            for part_index in 0..total_parts {
                let part_md4 = self.compute_part_md4(part_index, sequencer)?;
                root_hasher.update(part_md4);
            }
            let root_digest: [u8; 16] = root_hasher.finalize().into();
            Ok(root_digest)
        }
    }

    /// Verifies the complete file integrity against expected `self.file_hash`,
    /// transitions the resume state to `VerifiedPendingPromotion`, and persists it to disk.
    pub fn mark_verified_pending_promotion(
        &mut self,
        sequencer: &Ed2kDownloadSequencer,
    ) -> Result<[u8; 16], Ed2kSequencerError> {
        let computed_hash = self.compute_file_ed2k_hash(sequencer)?;
        if computed_hash != self.file_hash {
            return Err(Ed2kSequencerError::HashMismatch {
                expected: hex::encode(self.file_hash),
                actual: hex::encode(computed_hash),
            });
        }

        self.resume_state.status = Ed2kPartDownloadStatus::VerifiedPendingPromotion;
        self.flush_and_sync()?;
        Ok(computed_hash)
    }

    /// Two-phase crash-resilient promotion: moves the validated `.part` file to `destination_dir/final_file_name`.
    ///
    /// Pre-conditions:
    /// - `self.resume_state.status` must be `VerifiedPendingPromotion`.
    /// - `destination_dir/final_file_name` must not already exist (collision avoidance).
    ///
    /// Post-conditions:
    /// - `.part` file handle is closed (preventing Windows file lock issues).
    /// - Staged to temporary file in `destination_dir`, then atomically renamed to `final_file_name`.
    /// - Companion `.resume.json` state journal is deleted.
    /// - Returns final `PathBuf`.
    pub fn promote_to_destination(
        &mut self,
        destination_dir: &Path,
        final_file_name: &str,
    ) -> Result<PathBuf, Ed2kSequencerError> {
        if self.resume_state.status != Ed2kPartDownloadStatus::VerifiedPendingPromotion {
            return Err(Ed2kSequencerError::Storage(format!(
                "Cannot promote file: current status is {:?}, expected VerifiedPendingPromotion",
                self.resume_state.status
            )));
        }

        let target_file_path = destination_dir.join(final_file_name);
        if target_file_path.exists() {
            return Err(Ed2kSequencerError::Storage(format!(
                "Destination file already exists: '{}'",
                target_file_path.display()
            )));
        }

        if let Err(e) = fs::create_dir_all(destination_dir) {
            return Err(Ed2kSequencerError::Storage(format!(
                "Failed to create destination directory '{}': {e}",
                destination_dir.display()
            )));
        }

        // Flush and sync before releasing the file handle
        let _ = self.flush_and_sync();

        // Release/close the file handle so Windows does not lock the file from rename
        let _ = self.file.take();

        // Two-phase rename via temporary file in destination directory
        let tmp_target = destination_dir.join(format!("{}.part_promoted.tmp", final_file_name));
        if tmp_target.exists() {
            let _ = fs::remove_file(&tmp_target);
        }

        // First move source .part to tmp_target
        if fs::rename(&self.part_file_path, &tmp_target).is_err() {
            // Fallback for cross-volume moves: copy + sync + remove source
            if let Err(e) = fs::copy(&self.part_file_path, &tmp_target) {
                return Err(Ed2kSequencerError::Storage(format!(
                    "Failed to stage '{}' to '{}': {e}",
                    self.part_file_path.display(),
                    tmp_target.display()
                )));
            }
            let _ = fs::remove_file(&self.part_file_path);
        }

        // Second phase: atomic rename from tmp_target to final_file_path
        if let Err(e) = fs::rename(&tmp_target, &target_file_path) {
            return Err(Ed2kSequencerError::Storage(format!(
                "Failed to rename temporary target '{}' to final target '{}': {e}",
                tmp_target.display(),
                target_file_path.display()
            )));
        }

        // Update status and clean up companion resume journal
        self.resume_state.status = Ed2kPartDownloadStatus::Promoted;
        if self.resume_file_path.exists() {
            let _ = fs::remove_file(&self.resume_file_path);
        }
        let tmp_resume = format!("{}.tmp", self.resume_file_path.display());
        if Path::new(&tmp_resume).exists() {
            let _ = fs::remove_file(&tmp_resume);
        }

        Ok(target_file_path)
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

        let file = self.get_file_mut()?;
        file.seek(SeekFrom::Start(start))
            .map_err(|e| Ed2kSequencerError::Storage(format!("Seek error: {e}")))?;
        file.write_all(&part.data)
            .map_err(|e| Ed2kSequencerError::Storage(format!("Write error: {e}")))?;
        file.flush()
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
