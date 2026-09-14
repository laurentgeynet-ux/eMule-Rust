//! Deterministic eD2K download sequencing and block request planning.
//!
//! Tier 3.3 isolates and formalizes the pre-transfer planning rules:
//! - Deterministic part availability checking (respecting both partial bitmasks and
//!   eMule complete file semantics where `part_count == 0` denotes a complete source).
//! - Deterministic part selection.
//! - Decomposition of file parts into standard eD2K block ranges ($\le 184\,320$ bytes).
//! - Strict part boundary alignment ($9\,728\,000$ bytes boundary) preventing cross-part blocks.
//! - Bounded validation against overflows, invalid offsets, and file boundaries.
//! - Local wire frame generation for `OP_REQUESTPARTS_EMULE` (`0x47`) without transmission.
//! - Strict rejection of block planning when `OP_FILEREQANSNOFILE` (`0x48`) is received.

use crate::framing::RawPacket;
use super::encoder::Ed2kEncoder;
use super::packet::{Ed2kBlockRange, Ed2kFileStatus, Ed2kRequestParts};
use super::session::Ed2kSessionError;

/// Standard size of an eD2K part in bytes (9,500 KiB = 9,728,000 bytes).
pub const ED2K_PART_SIZE: usize = crate::limits::ED2K_PART_SIZE;

/// Standard size of an eD2K transfer block in bytes (180 KiB = 184,320 bytes).
pub const ED2K_BLOCK_SIZE: usize = crate::limits::ED2K_BLOCK_SIZE;

/// Maximum number of block ranges allowed per standard eD2K request packet (3).
pub const MAX_REQUEST_RANGES_PER_PACKET: usize = 3;

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
}

/// Deterministic download sequencer and block request planner for an active eD2K peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kDownloadSequencer {
    file_hash: [u8; 16],
    file_size: u64,
    total_parts: u16,
    status: Ed2kFileStatus,
}

impl Ed2kDownloadSequencer {
    /// Creates a new `Ed2kDownloadSequencer` from a known file hash, file size, and validated `Ed2kFileStatus`.
    pub fn new(
        file_hash: [u8; 16],
        file_size: u64,
        status: Ed2kFileStatus,
    ) -> Result<Self, Ed2kSequencerError> {
        if file_size == 0 {
            return Err(Ed2kSequencerError::EmptyFile);
        }
        if status.file_hash != file_hash {
            return Err(Ed2kSequencerError::HashMismatch {
                expected: hex::encode(file_hash),
                actual: hex::encode(status.file_hash),
            });
        }

        let total_parts = (file_size.div_ceil(ED2K_PART_SIZE as u64)).max(1) as u16;

        Ok(Self {
            file_hash,
            file_size,
            total_parts,
            status,
        })
    }

    /// Creates a sequencer from the result of a file status request.
    ///
    /// If the result was `Ed2kSessionError::FileNotFound`, immediately returns `Ed2kSequencerError::FileNotFound`.
    pub fn from_file_status_result(
        file_hash: [u8; 16],
        file_size: u64,
        result: Result<Ed2kFileStatus, Ed2kSessionError>,
    ) -> Result<Self, Ed2kSequencerError> {
        match result {
            Ok(status) => Self::new(file_hash, file_size, status),
            Err(Ed2kSessionError::FileNotFound(h)) => Err(Ed2kSequencerError::FileNotFound(h)),
            Err(e) => Err(Ed2kSequencerError::FileNotFound(e.to_string())),
        }
    }

    /// Returns the 16-byte MD4 file hash.
    #[inline]
    pub fn file_hash(&self) -> [u8; 16] {
        self.file_hash
    }

    /// Returns the total file size in bytes.
    #[inline]
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Returns the total number of parts in this file.
    #[inline]
    pub fn total_parts(&self) -> u16 {
        self.total_parts
    }

    /// Reference to the underlying `Ed2kFileStatus`.
    #[inline]
    pub fn status(&self) -> &Ed2kFileStatus {
        &self.status
    }

    /// Returns true if the remote peer has the specified part index available.
    ///
    /// Respects the eMule protocol specification:
    /// - If `status.is_complete()` (`part_count == 0` or complete bitmask), all valid file parts
    ///   are available.
    /// - If a partial mask is present, checks the specific part bit.
    /// - Returns false for out-of-bounds parts (`part_index >= total_parts`).
    pub fn is_part_available(&self, part_index: u16) -> bool {
        if part_index >= self.total_parts {
            return false;
        }
        if self.status.is_complete() {
            true
        } else {
            self.status.has_part(part_index)
        }
    }

    /// Returns a list of all part indices available from this remote source.
    pub fn available_parts(&self) -> Vec<u16> {
        (0..self.total_parts)
            .filter(|&p| self.is_part_available(p))
            .collect()
    }

    /// Returns the first available part index in ascending order, if any.
    pub fn select_first_available_part(&self) -> Option<u16> {
        (0..self.total_parts).find(|&p| self.is_part_available(p))
    }

    /// Selects and validates an explicit part index for download.
    pub fn select_part(&self, part_index: u16) -> Result<u16, Ed2kSequencerError> {
        if part_index >= self.total_parts {
            return Err(Ed2kSequencerError::PartIndexOutOfBounds {
                part_index,
                total_parts: self.total_parts,
            });
        }
        if !self.is_part_available(part_index) {
            return Err(Ed2kSequencerError::PartNotAvailable {
                part_index,
                total_parts: self.total_parts,
            });
        }
        Ok(part_index)
    }

    /// Returns the half-open byte bounds `[start, end)` for a given part index.
    pub fn part_byte_bounds(&self, part_index: u16) -> Result<(u64, u64), Ed2kSequencerError> {
        if part_index >= self.total_parts {
            return Err(Ed2kSequencerError::PartIndexOutOfBounds {
                part_index,
                total_parts: self.total_parts,
            });
        }
        let start = part_index as u64 * ED2K_PART_SIZE as u64;
        let end = std::cmp::min(start + ED2K_PART_SIZE as u64, self.file_size);
        Ok((start, end))
    }

    /// Plans the sequence of block ranges ($\le 184\,320$ bytes each) for a selected part.
    ///
    /// Guarantees:
    /// - Every block range is within the part's boundaries.
    /// - No block range crosses the part boundary ($9\,728\,000$ bytes alignment).
    /// - The final block in the part ends exactly at the part's end offset.
    pub fn plan_part_blocks(
        &self,
        part_index: u16,
    ) -> Result<Vec<Ed2kBlockRange>, Ed2kSequencerError> {
        self.select_part(part_index)?;
        let (part_start, part_end) = self.part_byte_bounds(part_index)?;

        let mut blocks = Vec::new();
        let mut curr = part_start;
        while curr < part_end {
            let block_end = std::cmp::min(curr + ED2K_BLOCK_SIZE as u64, part_end);
            let range = Ed2kBlockRange::from_wire_bounds(curr as u32, block_end as u32)
                .map_err(|e| Ed2kSequencerError::InvalidBlockRange {
                    start: curr,
                    end: block_end,
                    reason: e.to_string(),
                })?;
            blocks.push(range);
            curr = block_end;
        }

        Ok(blocks)
    }

    /// Validates an arbitrary block range `[start, end)` against all protocol and file invariants.
    pub fn validate_block_range(
        &self,
        start: u64,
        end: u64,
    ) -> Result<Ed2kBlockRange, Ed2kSequencerError> {
        if start >= end {
            return Err(Ed2kSequencerError::InvalidBlockRange {
                start,
                end,
                reason: format!("start offset {start} must be strictly less than end offset {end}"),
            });
        }
        if end > self.file_size {
            return Err(Ed2kSequencerError::InvalidBlockRange {
                start,
                end,
                reason: format!("end offset {end} exceeds file size {}", self.file_size),
            });
        }
        let len = end - start;
        if len > ED2K_BLOCK_SIZE as u64 {
            return Err(Ed2kSequencerError::InvalidBlockRange {
                start,
                end,
                reason: format!("block length {len} exceeds maximum allowed {ED2K_BLOCK_SIZE} bytes"),
            });
        }

        let p_start = (start / ED2K_PART_SIZE as u64) as u16;
        let p_end = ((end - 1) / ED2K_PART_SIZE as u64) as u16;
        if p_start != p_end {
            let part_boundary = (p_start as u64 + 1) * ED2K_PART_SIZE as u64;
            return Err(Ed2kSequencerError::CrossPartBoundary {
                start,
                end,
                part_boundary,
            });
        }

        if !self.is_part_available(p_start) {
            return Err(Ed2kSequencerError::PartNotAvailable {
                part_index: p_start,
                total_parts: self.total_parts,
            });
        }

        Ed2kBlockRange::from_wire_bounds(start as u32, end as u32).map_err(|e| {
            Ed2kSequencerError::InvalidBlockRange {
                start,
                end,
                reason: e.to_string(),
            }
        })
    }

    /// Constructs a validated `Ed2kRequestParts` message from 1 to 3 block ranges.
    pub fn build_request_parts(
        &self,
        ranges: &[Ed2kBlockRange],
    ) -> Result<Ed2kRequestParts, Ed2kSequencerError> {
        if ranges.is_empty() {
            return Err(Ed2kSequencerError::EmptyRanges);
        }
        if ranges.len() > MAX_REQUEST_RANGES_PER_PACKET {
            return Err(Ed2kSequencerError::TooManyRanges {
                count: ranges.len(),
                max: MAX_REQUEST_RANGES_PER_PACKET,
            });
        }

        for range in ranges {
            self.validate_block_range(range.start as u64, range.end as u64)?;
        }

        Ok(Ed2kRequestParts::multi(self.file_hash, ranges.to_vec()))
    }

    /// Constructs and encodes an `OP_REQUESTPARTS_EMULE` (`0x47`) wire packet without transmitting it.
    pub fn build_request_packet(
        &self,
        ranges: &[Ed2kBlockRange],
        encoder: &Ed2kEncoder,
    ) -> Result<RawPacket, Ed2kSequencerError> {
        let req = self.build_request_parts(ranges)?;
        encoder
            .encode_request_parts(&req)
            .map_err(|e| Ed2kSequencerError::Encode(e.to_string()))
    }
}
