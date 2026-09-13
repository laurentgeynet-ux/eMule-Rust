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

/// Passive observation structure for `OP_FILESTATUS` (`0x50`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kFileStatus {
    /// 16-byte MD4 File Hash.
    pub file_hash: [u8; 16],
    /// Total number of parts in the file.
    pub part_count: u16,
    /// Availability bitmask: ceil(part_count / 8) bytes.
    pub status_bitfield: Vec<u8>,
}

impl Ed2kFileStatus {
    /// Returns true if the remote peer has the specified part available.
    pub fn has_part(&self, part_index: usize) -> bool {
        if part_index >= self.part_count as usize {
            return false;
        }
        let byte_idx = part_index / 8;
        let bit_idx = part_index % 8;
        if byte_idx < self.status_bitfield.len() {
            (self.status_bitfield[byte_idx] & (1 << bit_idx)) != 0
        } else {
            false
        }
    }
}

/// High-level passive eD2K observation message enum.
#[derive(Debug, Clone, PartialEq)]
pub enum Ed2kMessage {
    /// Handshake initialization.
    Hello(Ed2kHello),
    /// Handshake response.
    HelloAnswer(Ed2kHelloAnswer),
    /// File part availability status.
    FileStatus(Ed2kFileStatus),
    /// Unparsed raw packet (used when opcode does not have a dedicated decoder yet).
    Raw(RawPacket),
}
