//! In-memory block sink for assembling downloaded eD2K blocks.

use crate::ed2k::packet::{Ed2kBlockRange, Ed2kSendingPart};
use super::types::Ed2kSequencerError;
use super::Ed2kDownloadSequencer;

/// Maximum allowable in-memory sink allocation (512 MiB) to prevent OOM denial-of-service.
pub const MAX_MEMORY_SINK_SIZE: u64 = 512 * 1024 * 1024;

/// In-memory sink for assembled eD2K block data.
///
/// Invariants:
/// - Exact pre-allocated buffer matching the total file size.
/// - Validates every incoming block against the active `Ed2kDownloadSequencer`.
/// - Rejects duplicate receptions of the same block range (`Ed2kSequencerError::DuplicateBlock`).
/// - Rejects block ranges crossing part boundaries (`Ed2kSequencerError::CrossPartBoundary`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kBlockSink {
    file_size: u64,
    buffer: Vec<u8>,
    received_ranges: Vec<(u64, u64)>,
}

impl Ed2kBlockSink {
    /// Creates a new zero-initialized `Ed2kBlockSink` for a given file size.
    /// Panics if `file_size > MAX_MEMORY_SINK_SIZE`.
    pub fn new(file_size: u64) -> Self {
        assert!(
            file_size <= MAX_MEMORY_SINK_SIZE,
            "file_size {} exceeds MAX_MEMORY_SINK_SIZE ({})",
            file_size,
            MAX_MEMORY_SINK_SIZE
        );
        Self {
            file_size,
            buffer: vec![0u8; file_size as usize],
            received_ranges: Vec::new(),
        }
    }

    /// Safely attempts to create an `Ed2kBlockSink`, returning an error if `file_size` exceeds limit.
    pub fn try_new(file_size: u64) -> Result<Self, String> {
        if file_size > MAX_MEMORY_SINK_SIZE {
            return Err(format!(
                "Requested file size {file_size} exceeds in-memory sink maximum of {MAX_MEMORY_SINK_SIZE} bytes"
            ));
        }
        Ok(Self {
            file_size,
            buffer: vec![0u8; file_size as usize],
            received_ranges: Vec::new(),
        })
    }

    /// Returns the total file size in bytes.
    #[inline]
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Returns the underlying byte buffer.
    #[inline]
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Returns the list of successfully written byte ranges `[start, end)`.
    #[inline]
    pub fn received_ranges(&self) -> &[(u64, u64)] {
        &self.received_ranges
    }

    /// Returns true if the half-open byte range `[start, end)` is completely covered by received slices.
    pub fn is_range_covered(&self, start: u64, end: u64) -> bool {
        Ed2kDownloadSequencer::is_range_in_list(start, end, &self.received_ranges)
    }

    /// Returns true if a block range `[start, end)` has already been received or is fully covered.
    pub fn has_range(&self, start: u64, end: u64) -> bool {
        if self.received_ranges.iter().any(|&(s, e)| s == start && e == end) {
            return true;
        }
        self.is_range_covered(start, end)
    }

    /// Returns true if any byte in `[start, end)` overlaps already received ranges.
    pub fn has_overlap(&self, start: u64, end: u64) -> bool {
        self.received_ranges.iter().any(|&(s, e)| s < end && start < e)
    }

    /// Returns the total number of bytes received and written so far.
    pub fn received_bytes_count(&self) -> usize {
        self.received_ranges.iter().map(|&(s, e)| (e - s) as usize).sum()
    }

    /// Validates and writes an incoming `Ed2kSendingPart` into the sink.
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

        let start_idx = start as usize;
        let end_idx = end as usize;
        self.buffer[start_idx..end_idx].copy_from_slice(&part.data);
        self.received_ranges.push((start, end));

        Ok(range)
    }
}
