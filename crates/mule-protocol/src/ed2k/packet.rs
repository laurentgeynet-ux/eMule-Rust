//! Passive data observation structures representing decoded eD2K wire messages.

use super::tags::{Ed2kTag, RawTag};
use crate::framing::RawPacket;

/// Passive observation structure for `OP_HELLO` (`0x01`).
#[derive(Debug, Clone, PartialEq)]
pub struct Ed2kHello {
    /// 16-byte MD4 User Hash.
    pub user_hash: [u8; 16],
    /// eD2K Client ID (IPv4 or LowID).
    pub client_id: u32,
    /// Listening TCP port.
    pub tcp_port: u16,
    /// Decoded recognized metadata tags.
    pub tags: Vec<Ed2kTag>,
    /// Unrecognized or raw tags preserved without data loss.
    pub unknown_tags: Vec<RawTag>,
    /// Connected server IPv4 address (or 0 if not connected).
    pub server_ip: u32,
    /// Connected server port (or 0 if not connected).
    pub server_port: u16,
}

impl Ed2kHello {
    /// Finds a recognized tag by name display string.
    pub fn find_tag(&self, name: &str) -> Option<&Ed2kTag> {
        self.tags.iter().find(|t| t.name.display_name() == name)
    }
}

/// Passive observation structure for `OP_HELLO_ANSWER` (`0x4C`).
#[derive(Debug, Clone, PartialEq)]
pub struct Ed2kHelloAnswer {
    /// 16-byte MD4 User Hash of the remote peer.
    pub user_hash: [u8; 16],
    /// eD2K Client ID of the remote peer.
    pub client_id: u32,
    /// Listening TCP port of the remote peer.
    pub tcp_port: u16,
    /// Decoded recognized metadata tags.
    pub tags: Vec<Ed2kTag>,
    /// Unrecognized or raw tags preserved without data loss.
    pub unknown_tags: Vec<RawTag>,
    /// Remote peer's connected server IPv4 address.
    pub server_ip: u32,
    /// Remote peer's connected server port.
    pub server_port: u16,
}

impl Ed2kHelloAnswer {
    /// Finds a recognized tag by name display string.
    pub fn find_tag(&self, name: &str) -> Option<&Ed2kTag> {
        self.tags.iter().find(|t| t.name.display_name() == name)
    }
}

/// Observation structure for eMule protocol extensions announcement `OP_EMULEINFO` (`0xC5 0x01`).
#[derive(Debug, Clone, PartialEq)]
pub struct Ed2kEmuleInfo {
    /// Remote eMule client version code (e.g. 0x3C = 60).
    pub client_version: u8,
    /// eMule protocol version byte (typically 0x01).
    pub protocol_version: u8,
    /// Decoded recognized metadata tags.
    pub tags: Vec<Ed2kTag>,
    /// Unrecognized or raw tags preserved without data loss.
    pub unknown_tags: Vec<RawTag>,
}

impl Ed2kEmuleInfo {
    /// Finds a recognized tag by name display string.
    pub fn find_tag(&self, name: &str) -> Option<&Ed2kTag> {
        self.tags.iter().find(|t| t.name.display_name() == name)
    }

    /// Returns true if compression capability tag was announced by remote peer.
    pub fn has_compression_capability(&self) -> bool {
        self.tags.iter().any(|t| match &t.name {
            super::tags::TagName::Id(0x20) | super::tags::TagName::Id(0x01) => true,
            super::tags::TagName::Text(s) => s.eq_ignore_ascii_case("compression"),
            _ => false,
        })
    }

    /// Returns true if source exchange capability tag was announced by remote peer.
    pub fn has_source_exchange_capability(&self) -> bool {
        self.tags.iter().any(|t| match &t.name {
            super::tags::TagName::Id(super::tags::TAG_SPECIAL_EMULE_EXT_SOURCEEXCHANGE)
            | super::tags::TagName::Id(0x04) => true,
            super::tags::TagName::Text(s) => {
                s.eq_ignore_ascii_case("sourceexchange") || s.eq_ignore_ascii_case("source_exchange")
            }
            _ => false,
        })
    }
}

/// Helper function to compute expected byte count for a given part count.
#[inline]
pub fn expected_mask_bytes(part_count: u16) -> usize {
    usize::from(part_count).div_ceil(8)
}

/// Encapsulated, safely bounded part availability mask.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PartAvailabilityMask {
    raw: bytes::Bytes,
    part_count: u16,
}

impl PartAvailabilityMask {
    /// Creates a mask from raw bytes, verifying exact length and zero trailing unused bits.
    pub fn from_bytes(raw: bytes::Bytes, part_count: u16) -> Result<Self, super::error::Ed2kErrorKind> {
        let expected = expected_mask_bytes(part_count);
        if raw.len() != expected {
            return Err(super::error::Ed2kErrorKind::MalformedField {
                message: format!(
                    "mask length {} does not match required {} for part_count {}",
                    raw.len(),
                    expected,
                    part_count
                ),
            });
        }

        // Verify trailing bits if part_count % 8 != 0
        let remainder = (part_count % 8) as usize;
        if remainder != 0 && !raw.is_empty() {
            let last_byte = raw[raw.len() - 1];
            // Valid bits mask: bits 0..(remainder - 1)
            let valid_mask = (1u8 << remainder) - 1;
            let trailing_bits = last_byte & !valid_mask;
            if trailing_bits != 0 {
                return Err(super::error::Ed2kErrorKind::InvalidBitfieldTrailingBits {
                    remainder,
                    trailing_bits,
                });
            }
        }

        Ok(Self { raw, part_count })
    }

    /// Creates a new mask from a slice of available part indices.
    pub fn from_parts(available_parts: &[u16], part_count: u16) -> Self {
        let expected = expected_mask_bytes(part_count);
        let mut buf = vec![0u8; expected];
        for &part in available_parts {
            if part < part_count {
                let byte_idx = (part / 8) as usize;
                let bit_idx = (part % 8) as usize;
                buf[byte_idx] |= 1 << bit_idx;
            }
        }
        Self {
            raw: bytes::Bytes::from(buf),
            part_count,
        }
    }

    /// Creates an empty mask (0 parts).
    pub fn empty() -> Self {
        Self {
            raw: bytes::Bytes::new(),
            part_count: 0,
        }
    }

    /// Total number of parts covered by this mask.
    pub fn part_count(&self) -> u16 {
        self.part_count
    }

    /// Returns true if the remote peer has the specified part available.
    pub fn has_part(&self, part_index: u16) -> bool {
        if part_index >= self.part_count {
            return false;
        }
        let byte_idx = (part_index / 8) as usize;
        let bit_idx = (part_index % 8) as usize;
        if byte_idx < self.raw.len() {
            (self.raw[byte_idx] & (1 << bit_idx)) != 0
        } else {
            false
        }
    }

    /// Returns an iterator over all available part indices.
    pub fn available_parts(&self) -> impl Iterator<Item = u16> + '_ {
        (0..self.part_count).filter(move |&idx| self.has_part(idx))
    }

    /// Returns the raw underlying bytes of the mask.
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw
    }

    /// Returns the underlying `Bytes` buffer.
    pub fn into_bytes(self) -> bytes::Bytes {
        self.raw
    }
}

/// Request for remote file parts availability bitmask (`0x49`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kFileStatusReq {
    /// 16-byte MD4 File Hash.
    pub file_hash: [u8; 16],
}

/// Passive observation structure for `OP_FILESTATUS` (`0x50`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kFileStatus {
    /// 16-byte MD4 File Hash.
    pub file_hash: [u8; 16],
    /// Total number of parts in the file.
    pub part_count: u16,
    /// Availability bitmask: ceil(part_count / 8) bytes.
    pub availability_mask: PartAvailabilityMask,
}

impl Ed2kFileStatus {
    /// Returns true if the remote peer has the specified part available.
    #[inline]
    pub fn has_part(&self, part_index: u16) -> bool {
        self.availability_mask.has_part(part_index)
    }

    /// Compatibility accessor for raw byte slice.
    #[inline]
    pub fn raw_bytes(&self) -> &[u8] {
        self.availability_mask.raw_bytes()
    }
}

/// A contiguous byte range within a file for block requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ed2kBlockRange {
    /// Start byte offset.
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
}

impl Ed2kBlockRange {
    /// Creates a new `Ed2kBlockRange`.
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Length of the range in bytes.
    #[inline]
    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Returns true if start >= end.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Creates a block range directly from canonical eMule wire bounds `(start, end)`.
    ///
    /// In eMule/aMule reference implementations (`DownloadClient.cpp`, `ListenSocket.cpp`),
    /// the end offset on the wire is transmitted as **exclusive**: `endpos = block->EndOffset + 1`.
    /// The byte count of the block is strictly `end - start`.
    pub fn from_wire_bounds(start: u32, end: u32) -> Result<Self, super::error::Ed2kErrorKind> {
        if start >= end {
            return Err(super::error::Ed2kErrorKind::InvalidBlockOffsets {
                start: start as u64,
                end: end as u64,
            });
        }
        Ok(Self { start, end })
    }

    /// Returns the canonical wire bounds `(start, end)` directly matching eMule exclusive convention.
    #[inline]
    pub fn to_wire_bounds(&self) -> (u32, u32) {
        (self.start, self.end)
    }

    /// Converts a legacy donkey wire representation with an inclusive end offset `[start, end_inclusive]`
    /// into a half-open `[start, end)` block range used by the core engine.
    pub fn from_legacy_inclusive_wire_bounds(
        start: u32,
        end_inclusive: u32,
    ) -> Result<Self, super::error::Ed2kErrorKind> {
        let end = end_inclusive.checked_add(1).ok_or_else(|| {
            super::error::Ed2kErrorKind::MalformedField {
                message: "integer overflow in legacy inclusive end offset conversion".to_string(),
            }
        })?;
        if start >= end {
            return Err(super::error::Ed2kErrorKind::InvalidBlockOffsets {
                start: start as u64,
                end: end as u64,
            });
        }
        Ok(Self { start, end })
    }

    /// Alias for `from_legacy_inclusive_wire_bounds`.
    #[inline]
    pub fn from_inclusive_wire_bounds(
        start: u32,
        end_inclusive: u32,
    ) -> Result<Self, super::error::Ed2kErrorKind> {
        Self::from_legacy_inclusive_wire_bounds(start, end_inclusive)
    }

    /// Converts a half-open `[start, end)` range to a legacy inclusive wire representation `(start, end_inclusive)`.
    pub fn to_legacy_inclusive_wire_bounds(&self) -> Result<(u32, u32), super::error::Ed2kErrorKind> {
        if self.is_empty() {
            return Err(super::error::Ed2kErrorKind::InvalidBlockOffsets {
                start: self.start as u64,
                end: self.end as u64,
            });
        }
        Ok((self.start, self.end - 1))
    }

    /// Alias for `to_legacy_inclusive_wire_bounds`.
    #[inline]
    pub fn to_inclusive_wire_bounds(&self) -> Result<(u32, u32), super::error::Ed2kErrorKind> {
        self.to_legacy_inclusive_wire_bounds()
    }
}

/// Request for file part blocks (`0x4B`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kRequestParts {
    /// 16-byte MD4 File Hash.
    pub file_hash: [u8; 16],
    /// Requested ranges (up to 3 in standard eMule wire format).
    pub ranges: Vec<Ed2kBlockRange>,
}

impl Ed2kRequestParts {
    /// Creates a single-range block request.
    pub fn single(file_hash: [u8; 16], start: u32, end: u32) -> Self {
        Self {
            file_hash,
            ranges: vec![Ed2kBlockRange::new(start, end)],
        }
    }

    /// Creates a multi-range block request.
    pub fn multi(file_hash: [u8; 16], ranges: Vec<Ed2kBlockRange>) -> Self {
        Self { file_hash, ranges }
    }
}

/// Data payload delivery of a requested block (`0x46`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kSendingPart {
    /// 16-byte MD4 File Hash.
    pub file_hash: [u8; 16],
    /// Block start byte offset.
    pub start_offset: u32,
    /// Block end byte offset.
    pub end_offset: u32,
    /// Raw block data bytes.
    pub data: bytes::Bytes,
}

impl Ed2kSendingPart {
    /// Creates a new `Ed2kSendingPart`.
    pub fn new(file_hash: [u8; 16], start_offset: u32, end_offset: u32, data: bytes::Bytes) -> Self {
        Self {
            file_hash,
            start_offset,
            end_offset,
            data,
        }
    }

    /// Returns the byte length of data.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the block payload is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// High-level passive eD2K observation message enum.
#[derive(Debug, Clone, PartialEq)]
pub enum Ed2kMessage {
    /// Handshake initialization.
    Hello(Ed2kHello),
    /// Handshake response.
    HelloAnswer(Ed2kHelloAnswer),
    /// Extended eMule handshake capabilities exchange.
    EmuleInfo(Ed2kEmuleInfo),
    /// File parts availability request.
    FileStatusReq(Ed2kFileStatusReq),
    /// File part availability status response.
    FileStatus(Ed2kFileStatus),
    /// Request for block data.
    RequestParts(Ed2kRequestParts),
    /// Delivery of block data.
    SendingPart(Ed2kSendingPart),
    /// Unparsed raw packet (used when opcode does not have a dedicated decoder yet).
    Raw(RawPacket),
}
