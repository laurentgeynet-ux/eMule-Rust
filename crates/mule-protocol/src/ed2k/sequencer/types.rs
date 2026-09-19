//! Error and tracking types for eD2K sequencing and block planning.

/// Errors arising during download sequencing and block request planning.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Ed2kSequencerError {
    /// Remote peer reported file not shared or unknown via `OP_FILEREQANSNOFILE` (`0x48`).
    #[error("File is not shared or known by remote peer: {0}")]
    FileNotFound(String),

    /// File size is zero.
    #[error("File size is 0 bytes")]
    EmptyFile,

    /// Requested file hash does not match status hash.
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch {
        /// Expected file hash hex.
        expected: String,
        /// Actual status file hash hex.
        actual: String,
    },

    /// Part index is out of bounds for the file.
    #[error("Part index {part_index} is out of bounds (total parts: {total_parts})")]
    PartIndexOutOfBounds {
        /// Requested part index.
        part_index: u16,
        /// Total number of parts in file.
        total_parts: u16,
    },

    /// Part index is not available from the remote peer.
    #[error("Part index {part_index} is not available from remote source (total parts: {total_parts})")]
    PartNotAvailable {
        /// Requested part index.
        part_index: u16,
        /// Total number of parts in file.
        total_parts: u16,
    },

    /// Invalid block range parameters.
    #[error("Invalid block range [{start}, {end}): {reason}")]
    InvalidBlockRange {
        /// Start byte offset.
        start: u64,
        /// End byte offset (exclusive).
        end: u64,
        /// Failure explanation.
        reason: String,
    },

    /// Block range improperly crosses an eD2K part boundary.
    #[error("Block range [{start}, {end}) crosses part boundary at {part_boundary}")]
    CrossPartBoundary {
        /// Start byte offset.
        start: u64,
        /// End byte offset (exclusive).
        end: u64,
        /// Byte offset of the crossed part boundary.
        part_boundary: u64,
    },

    /// Number of requested ranges exceeds protocol maximum (3).
    #[error("Requested {count} ranges exceeds maximum allowed {max} ranges per packet")]
    TooManyRanges {
        /// Number of ranges specified.
        count: usize,
        /// Maximum allowed ranges.
        max: usize,
    },

    /// No block ranges specified.
    #[error("Zero ranges specified for block request")]
    EmptyRanges,

    /// Wire encoding failure.
    #[error("Encoding error: {0}")]
    Encode(String),

    /// Block payload data length does not match specified range bounds.
    #[error("Block data length mismatch: expected {expected} bytes, got {actual} bytes")]
    BlockDataMismatch {
        /// Expected byte count (`end - start`).
        expected: usize,
        /// Actual payload byte count.
        actual: usize,
    },

    /// Block range was already received and written.
    #[error("Duplicate block received for range [{start}, {end})")]
    DuplicateBlock {
        /// Start byte offset.
        start: u64,
        /// End byte offset (exclusive).
        end: u64,
    },

    /// Compressed frame received when uncompressed block transfer was expected.
    #[error("Compressed block transfer is not negotiated or supported in passive profile")]
    CompressedNotSupported,

    /// Storage, filesystem, or serialization error.
    #[error("Storage error: {0}")]
    Storage(String),
}

/// Represents an active in-flight block range assigned to a specific peer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InFlightBlock {
    /// Identifier or address of the owning peer.
    pub peer_id: String,
    /// Target part index.
    pub part_index: u16,
    /// Half-open start byte offset.
    pub start: u64,
    /// Half-open end byte offset (exclusive).
    pub end: u64,
    /// Timestamp when block was requested (unix ms).
    pub issued_at_unix_ms: i64,
    /// Expiration deadline after which block may be reallocated (unix ms).
    pub deadline_unix_ms: i64,
}

impl InFlightBlock {
    /// Creates a new in-flight block tracking entry.
    pub fn new(
        peer_id: String,
        part_index: u16,
        start: u64,
        end: u64,
        issued_at_unix_ms: i64,
        timeout_ms: i64,
    ) -> Self {
        Self {
            peer_id,
            part_index,
            start,
            end,
            issued_at_unix_ms,
            deadline_unix_ms: issued_at_unix_ms + timeout_ms,
        }
    }

    /// Returns true if the block range has expired at `now_unix_ms`.
    pub fn is_expired(&self, now_unix_ms: i64) -> bool {
        now_unix_ms >= self.deadline_unix_ms
    }

    /// Checks if this in-flight block overlaps with range `[s, e)`.
    pub fn overlaps(&self, s: u64, e: u64) -> bool {
        self.start < e && s < self.end
    }
}
