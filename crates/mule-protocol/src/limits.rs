// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Protocol limits and constants for safe deterministic network handling.

/// Header size of an eD2K packet (1 byte protocol + 4 bytes little-endian length).
pub const HEADER_SIZE: usize = 5;

/// Maximum allowable frame size (length field) in bytes (1 MiB).
///
/// Standard eD2K blocks are 180 KiB (184,320 bytes), so 1 MiB provides plenty of headroom
/// while completely preventing memory exhaustion from malicious length fields.
pub const MAX_FRAME_BYTES: usize = 1_048_576;

/// Maximum buffer capacity allowed per connection before peer is forcefully disconnected (2 MiB).
pub const MAX_BUFFERED_BYTES: usize = 2_097_152;

/// Maximum string length permitted in any tag payload (16 KiB).
pub const MAX_STRING_BYTES: usize = 16_384;

/// Maximum number of tags permitted in a single message.
pub const MAX_TAGS_PER_MESSAGE: usize = 256;

/// Maximum number of peers returned in a single source response.
pub const MAX_PEERS_PER_RESPONSE: usize = 500;

/// Standard size of an eD2K block request/transfer (180 KiB = 184,320 bytes).
pub const ED2K_BLOCK_SIZE: usize = 184_320;

/// Standard size of an eD2K part (9,500 KiB = 9,728,000 bytes).
pub const ED2K_PART_SIZE: usize = 9_728_000;

/// Maximum number of active in-flight block requests per peer session.
pub const MAX_IN_FLIGHT_REQUESTS: usize = 8;

/// Maximum block transfer size (180 KiB = 184,320 bytes).
pub const MAX_BLOCK_BYTES: usize = ED2K_BLOCK_SIZE;

/// Handshake negotiation timeout (5 seconds).
pub const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Inactive connection idle timeout (30 seconds).
pub const IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Maximum protocol violations tolerated before closing session (immediate failure).
pub const MAX_PROTOCOL_VIOLATIONS: u8 = 1;

/// Trait defining contextual, opcode-aware resource boundaries for protocol messages.
pub trait PacketLimits: Send + Sync {
    /// Maximum wire frame size permitted for a given protocol byte and opcode.
    fn max_wire_frame_bytes(&self, protocol: u8, opcode: u8) -> usize;

    /// Maximum decoded payload size permitted post-framing.
    fn max_decoded_payload_bytes(&self, protocol: u8, opcode: u8) -> usize;

    /// Maximum string length permitted within tags for the specified opcode.
    fn max_string_bytes(&self, opcode: u8) -> usize;
}

/// Default global packet limits implementation.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultPacketLimits;

impl PacketLimits for DefaultPacketLimits {
    fn max_wire_frame_bytes(&self, _protocol: u8, _opcode: u8) -> usize {
        MAX_FRAME_BYTES
    }

    fn max_decoded_payload_bytes(&self, _protocol: u8, _opcode: u8) -> usize {
        MAX_FRAME_BYTES
    }

    fn max_string_bytes(&self, _opcode: u8) -> usize {
        MAX_STRING_BYTES
    }
}
