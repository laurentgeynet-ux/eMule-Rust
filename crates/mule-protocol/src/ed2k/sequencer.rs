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

pub mod sink;
pub mod types;

use crate::framing::RawPacket;
pub use sink::{Ed2kBlockSink, MAX_MEMORY_SINK_SIZE};
pub use types::{Ed2kSequencerError, InFlightBlock};

use super::encoder::Ed2kEncoder;
use super::packet::{
    Ed2kBlockRange, Ed2kBlockRange64, Ed2kFileStatus, Ed2kRequestParts, Ed2kRequestPartsI64,
    Ed2kSendingPart,
};
use super::session::Ed2kSessionError;

/// Standard size of an eD2K part in bytes (9,500 KiB = 9,728,000 bytes).
pub const ED2K_PART_SIZE: usize = crate::limits::ED2K_PART_SIZE;

/// Standard size of an eD2K transfer block in bytes (180 KiB = 184,320 bytes).
pub const ED2K_BLOCK_SIZE: usize = crate::limits::ED2K_BLOCK_SIZE;

/// Maximum number of block ranges allowed per standard eD2K request packet (3).
pub const MAX_REQUEST_RANGES_PER_PACKET: usize = 3;

/// eMule end-game threshold: when the remaining download fits in a single
/// request batch (≈ 3 blocks), a block already in-flight on *another* peer
/// may be requested in parallel so the file finishes at the speed of the
/// fastest source (`CPartFile::GetNextRequestedBlock` duplicates the last
/// chunks near completion).
const ENDGAME_REMAINING_BYTES: u64 =
    ED2K_BLOCK_SIZE as u64 * MAX_REQUEST_RANGES_PER_PACKET as u64;

/// Maximum concurrent in-flight assignments allowed for the same block range
/// during end-game (the first arriving copy wins; the duplicate is discarded).
const MAX_INFLIGHT_PER_BLOCK: usize = 2;

/// Deterministic download sequencer and block request planner for an active eD2K peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kDownloadSequencer {
    file_hash: [u8; 16],
    file_size: u64,
    total_parts: u16,
    status: Ed2kFileStatus,
    /// eMule preview mode: the first and last parts are requested before all
    /// others so the file can be previewed while downloading
    /// (`CPartFile` gives them `PR_VERYHIGH` when preview is enabled).
    preview_priority: bool,
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
            preview_priority: false,
        })
    }

    /// Enables the eMule preview part priority: the first and last parts are
    /// planned before all others (used for previewable media files).
    pub fn with_preview_priority(mut self, enabled: bool) -> Self {
        self.preview_priority = enabled;
        self
    }

    /// Creates a sequencer from the result of a file status request.
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
            let range =
                Ed2kBlockRange::from_wire_bounds(curr as u32, block_end as u32).map_err(|e| {
                    Ed2kSequencerError::InvalidBlockRange {
                        start: curr,
                        end: block_end,
                        reason: e.to_string(),
                    }
                })?;
            blocks.push(range);
            curr = block_end;
        }

        Ok(blocks)
    }

    /// 64-bit variant of `plan_part_blocks` for files larger than 4 GiB —
    /// offsets past `u32::MAX` are preserved instead of being truncated.
    pub fn plan_part_blocks64(
        &self,
        part_index: u16,
    ) -> Result<Vec<Ed2kBlockRange64>, Ed2kSequencerError> {
        self.select_part(part_index)?;
        let (part_start, part_end) = self.part_byte_bounds(part_index)?;

        let mut blocks = Vec::new();
        let mut curr = part_start;
        while curr < part_end {
            let block_end = std::cmp::min(curr + ED2K_BLOCK_SIZE as u64, part_end);
            blocks.push(Ed2kBlockRange64::new(curr, block_end));
            curr = block_end;
        }

        Ok(blocks)
    }

    /// Validates an arbitrary 64-bit block range `[start, end)` against all
    /// protocol and file invariants (same rules as `validate_block_range`).
    pub fn validate_block_range64(
        &self,
        start: u64,
        end: u64,
    ) -> Result<Ed2kBlockRange64, Ed2kSequencerError> {
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
                reason: format!(
                    "block length {len} exceeds maximum allowed {ED2K_BLOCK_SIZE} bytes"
                ),
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

        Ok(Ed2kBlockRange64::new(start, end))
    }

    /// Constructs a validated `Ed2kRequestPartsI64` (`OP_REQUESTPARTS_I64`)
    /// message from 1 to 3 64-bit block ranges.
    pub fn build_request_parts_i64(
        &self,
        ranges: &[Ed2kBlockRange64],
    ) -> Result<Ed2kRequestPartsI64, Ed2kSequencerError> {
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
            self.validate_block_range64(range.start, range.end)?;
        }
        Ok(Ed2kRequestPartsI64 {
            file_hash: self.file_hash,
            ranges: ranges.to_vec(),
        })
    }

    /// 64-bit variant of `plan_next_block_for_peer` — required for downloads
    /// whose blocks extend past the 4 GiB boundary.
    ///
    /// Part selection follows the eMule "rarest/progress-first" strategy:
    /// instead of always draining the lowest part index (which concentrates
    /// the whole swarm on the same chunks), the peer is assigned to the
    /// needed part it offers that currently has the *fewest* completed
    /// bytes. Ties resolve to the lowest index. This spreads sources across
    /// distinct parts like `CPartFile::GetNextRequestedBlock` does.
    pub fn plan_next_block_for_peer64(
        &self,
        peer_id: &str,
        completed_ranges: &[(u64, u64)],
        in_flight_blocks: &[InFlightBlock],
        now_unix_ms: i64,
    ) -> Result<Option<(u16, Ed2kBlockRange64)>, Ed2kSequencerError> {
        // eMule preview: the first and last parts are served first so the
        // file can be opened while downloading (`PR_VERYHIGH` on edges).
        if self.preview_priority {
            let last = self.total_parts - 1;
            for edge in [0u16, last] {
                if !self.is_part_available(edge) {
                    continue;
                }
                if let Some(block) =
                    self.next_free_block_in_part64(edge, completed_ranges, in_flight_blocks, now_unix_ms)?
                {
                    return Ok(Some((edge, block)));
                }
            }
        }

        let mut best: Option<(u16, Ed2kBlockRange64, u64)> = None;

        for part_idx in 0..self.total_parts {
            if !self.is_part_available(part_idx) {
                continue;
            }

            // Received bytes already covering this part (progress metric).
            let (part_start, part_end) = self.part_byte_bounds(part_idx)?;
            let received_in_part: u64 = completed_ranges
                .iter()
                .filter_map(|&(s, e)| {
                    let cs = s.max(part_start);
                    let ce = e.min(part_end);
                    (cs < ce).then(|| ce - cs)
                })
                .sum();
            if received_in_part >= part_end - part_start {
                continue;
            }

            if let Some(block) =
                self.next_free_block_in_part64(part_idx, completed_ranges, in_flight_blocks, now_unix_ms)?
            {
                let replace = match &best {
                    None => true,
                    Some((_, _, best_received)) => received_in_part < *best_received,
                };
                if replace {
                    best = Some((part_idx, block, received_in_part));
                }
            }
        }

        if let Some((part_idx, block, _)) = best {
            return Ok(Some((part_idx, block)));
        }

        self.endgame_block64(peer_id, completed_ranges, in_flight_blocks, now_unix_ms)
    }

    /// eMule end-game fallback: when fewer than `ENDGAME_REMAINING_BYTES`
    /// remain, allows requesting a block already in-flight on a *different*
    /// peer (max `MAX_INFLIGHT_PER_BLOCK` concurrent assignments per block).
    /// Returns `None` outside end-game, when every missing block is already
    /// doubly assigned, or when the only pending copies belong to this peer.
    fn endgame_block64(
        &self,
        peer_id: &str,
        completed_ranges: &[(u64, u64)],
        in_flight_blocks: &[InFlightBlock],
        now_unix_ms: i64,
    ) -> Result<Option<(u16, Ed2kBlockRange64)>, Ed2kSequencerError> {
        let covered: u64 = completed_ranges
            .iter()
            .map(|&(s, e)| e.saturating_sub(s))
            .sum();
        let remaining = self.file_size.saturating_sub(covered);
        if remaining == 0 || remaining > ENDGAME_REMAINING_BYTES {
            return Ok(None);
        }
        for part_idx in 0..self.total_parts {
            if !self.is_part_available(part_idx) {
                continue;
            }
            for block in self.plan_part_blocks64(part_idx)? {
                if Self::is_range_in_list(block.start, block.end, completed_ranges) {
                    continue;
                }
                let mut active = 0usize;
                let mut same_peer = false;
                for ifb in in_flight_blocks {
                    if ifb.is_expired(now_unix_ms) || !ifb.overlaps(block.start, block.end) {
                        continue;
                    }
                    if ifb.peer_id == peer_id {
                        same_peer = true;
                        break;
                    }
                    active += 1;
                }
                if same_peer || active == 0 || active >= MAX_INFLIGHT_PER_BLOCK {
                    continue;
                }
                return Ok(Some((part_idx, block)));
            }
        }
        Ok(None)
    }

    /// Returns the first missing, non-in-flight block inside `part_idx`,
    /// or `None` if the part is complete, fully in-flight, or not offered
    /// by this peer.
    fn next_free_block_in_part64(
        &self,
        part_idx: u16,
        completed_ranges: &[(u64, u64)],
        in_flight_blocks: &[InFlightBlock],
        now_unix_ms: i64,
    ) -> Result<Option<Ed2kBlockRange64>, Ed2kSequencerError> {
        let blocks = self.plan_part_blocks64(part_idx)?;
        Ok(blocks.into_iter().find(|block| {
            if Self::is_range_in_list(block.start, block.end, completed_ranges) {
                return false;
            }
            !in_flight_blocks.iter().any(|ifb| {
                !ifb.is_expired(now_unix_ms) && ifb.overlaps(block.start, block.end)
            })
        }))
    }

    /// Plans the sequence of block ranges for a part, filtering out any blocks
    /// that are already fully covered by the given completed ranges.
    pub fn plan_missing_blocks(
        &self,
        part_index: u16,
        completed_ranges: &[(u64, u64)],
    ) -> Result<Vec<Ed2kBlockRange>, Ed2kSequencerError> {
        let all_blocks = self.plan_part_blocks(part_index)?;
        let missing = all_blocks
            .into_iter()
            .filter(|block| {
                let b_start = block.start as u64;
                let b_end = block.end as u64;
                !Self::is_range_in_list(b_start, b_end, completed_ranges)
            })
            .collect();
        Ok(missing)
    }

    /// Helper to test if half-open byte range `[start, end)` is completely covered by a list of ranges.
    pub fn is_range_in_list(start: u64, end: u64, ranges: &[(u64, u64)]) -> bool {
        if start >= end {
            return true;
        }
        let mut curr = start;
        let mut matching_ranges: Vec<(u64, u64)> = ranges
            .iter()
            .copied()
            .filter(|&(s, e)| e > start && s < end)
            .collect();
        matching_ranges.sort_unstable_by_key(|r| r.0);

        for (s, e) in matching_ranges {
            if s > curr {
                return false;
            }
            if e > curr {
                curr = e;
            }
            if curr >= end {
                return true;
            }
        }
        curr >= end
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
                reason: format!(
                    "block length {len} exceeds maximum allowed {ED2K_BLOCK_SIZE} bytes"
                ),
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

    /// Validates an incoming `Ed2kSendingPart` block against file hash, data length,
    /// block length limit, and part boundaries.
    pub fn validate_sending_part(
        &self,
        part: &Ed2kSendingPart,
    ) -> Result<Ed2kBlockRange, Ed2kSequencerError> {
        if part.file_hash != self.file_hash {
            return Err(Ed2kSequencerError::HashMismatch {
                expected: hex::encode(self.file_hash),
                actual: hex::encode(part.file_hash),
            });
        }

        let expected_len = part.end_offset.saturating_sub(part.start_offset) as usize;
        if part.data.len() != expected_len {
            return Err(Ed2kSequencerError::BlockDataMismatch {
                expected: expected_len,
                actual: part.data.len(),
            });
        }

        self.validate_block_range(part.start_offset as u64, part.end_offset as u64)
    }

    /// Plans the next unassigned and uncompleted block range for a specific peer.
    ///
    /// Same rarest/progress-first part selection as
    /// `plan_next_block_for_peer64`: the peer is assigned to the needed part
    /// it offers with the fewest completed bytes (ties → lowest index).
    pub fn plan_next_block_for_peer(
        &self,
        peer_id: &str,
        completed_ranges: &[(u64, u64)],
        in_flight_blocks: &[InFlightBlock],
        now_unix_ms: i64,
    ) -> Result<Option<(u16, Ed2kBlockRange)>, Ed2kSequencerError> {
        if self.preview_priority {
            let last = self.total_parts - 1;
            for edge in [0u16, last] {
                if !self.is_part_available(edge) {
                    continue;
                }
                if let Some(block) =
                    self.next_free_block_in_part(edge, completed_ranges, in_flight_blocks, now_unix_ms)?
                {
                    return Ok(Some((edge, block)));
                }
            }
        }

        let mut best: Option<(u16, Ed2kBlockRange, u64)> = None;

        for part_idx in 0..self.total_parts {
            if !self.is_part_available(part_idx) {
                continue;
            }

            let (part_start, part_end) = self.part_byte_bounds(part_idx)?;
            let received_in_part: u64 = completed_ranges
                .iter()
                .filter_map(|&(s, e)| {
                    let cs = s.max(part_start);
                    let ce = e.min(part_end);
                    (cs < ce).then(|| ce - cs)
                })
                .sum();
            if received_in_part >= part_end - part_start {
                continue;
            }

            if let Some(block) =
                self.next_free_block_in_part(part_idx, completed_ranges, in_flight_blocks, now_unix_ms)?
            {
                let replace = match &best {
                    None => true,
                    Some((_, _, best_received)) => received_in_part < *best_received,
                };
                if replace {
                    best = Some((part_idx, block, received_in_part));
                }
            }
        }

        if let Some((part_idx, block, _)) = best {
            return Ok(Some((part_idx, block)));
        }

        self.endgame_block(peer_id, completed_ranges, in_flight_blocks, now_unix_ms)
    }

    /// 32-bit variant of `endgame_block64`.
    fn endgame_block(
        &self,
        peer_id: &str,
        completed_ranges: &[(u64, u64)],
        in_flight_blocks: &[InFlightBlock],
        now_unix_ms: i64,
    ) -> Result<Option<(u16, Ed2kBlockRange)>, Ed2kSequencerError> {
        let covered: u64 = completed_ranges
            .iter()
            .map(|&(s, e)| e.saturating_sub(s))
            .sum();
        let remaining = self.file_size.saturating_sub(covered);
        if remaining == 0 || remaining > ENDGAME_REMAINING_BYTES {
            return Ok(None);
        }
        for part_idx in 0..self.total_parts {
            if !self.is_part_available(part_idx) {
                continue;
            }
            for block in self.plan_part_blocks(part_idx)? {
                let b_start = block.start as u64;
                let b_end = block.end as u64;
                if Self::is_range_in_list(b_start, b_end, completed_ranges) {
                    continue;
                }
                let mut active = 0usize;
                let mut same_peer = false;
                for ifb in in_flight_blocks {
                    if ifb.is_expired(now_unix_ms) || !ifb.overlaps(b_start, b_end) {
                        continue;
                    }
                    if ifb.peer_id == peer_id {
                        same_peer = true;
                        break;
                    }
                    active += 1;
                }
                if same_peer || active == 0 || active >= MAX_INFLIGHT_PER_BLOCK {
                    continue;
                }
                return Ok(Some((part_idx, block)));
            }
        }
        Ok(None)
    }

    /// 32-bit variant of `next_free_block_in_part64`.
    fn next_free_block_in_part(
        &self,
        part_idx: u16,
        completed_ranges: &[(u64, u64)],
        in_flight_blocks: &[InFlightBlock],
        now_unix_ms: i64,
    ) -> Result<Option<Ed2kBlockRange>, Ed2kSequencerError> {
        let blocks = self.plan_part_blocks(part_idx)?;
        Ok(blocks.into_iter().find(|block| {
            let b_start = block.start as u64;
            let b_end = block.end as u64;
            if Self::is_range_in_list(b_start, b_end, completed_ranges) {
                return false;
            }
            !in_flight_blocks
                .iter()
                .any(|ifb| !ifb.is_expired(now_unix_ms) && ifb.overlaps(b_start, b_end))
        }))
    }
}
