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
