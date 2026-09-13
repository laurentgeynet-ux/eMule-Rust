//! Safety thresholds and resource limits dedicated to eD2K wire parsing.

/// Default maximum frame size for general eD2K packets (256 KiB).
pub const ED2K_MAX_FRAME_DEFAULT: usize = 256 * 1024;

/// Maximum frame size permitted specifically for `OP_HELLO` and `OP_HELLO_ANSWER` (64 KiB).
pub const ED2K_MAX_HELLO_FRAME: usize = 64 * 1024;

/// Maximum frame size permitted for status / hashset packets (64 KiB).
pub const ED2K_MAX_STATUS_FRAME: usize = 64 * 1024;

/// Maximum number of tags allowed in any single message (256).
pub const ED2K_MAX_TAGS: usize = 256;

/// Maximum length in bytes permitted for any string tag value or tag name (1 KiB).
pub const ED2K_MAX_STRING_BYTES: usize = 1024;

/// Maximum number of file parts in a status bitfield (65,530 parts).
pub const ED2K_MAX_PARTS: usize = 65_530;

/// Maximum blob length permitted in a tag (64 KiB).
pub const ED2K_MAX_BLOB_BYTES: usize = 64 * 1024;

/// Dedicated resource boundaries for eD2K decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kLimits {
    /// Maximum allowable frame size (opcode + payload) for standard packets.
    pub max_frame_bytes: usize,
    /// Maximum allowable frame size for hello / handshake packets.
    pub max_hello_frame_bytes: usize,
    /// Maximum number of tags allowed in a single packet.
    pub max_tags: usize,
    /// Maximum allowable byte length for strings.
    pub max_string_bytes: usize,
    /// Maximum number of file parts permitted in status messages.
    pub max_parts: usize,
    /// Maximum allowable byte length for tag blobs.
    pub max_blob_bytes: usize,
}

impl Default for Ed2kLimits {
    fn default() -> Self {
        Self {
            max_frame_bytes: ED2K_MAX_FRAME_DEFAULT,
            max_hello_frame_bytes: ED2K_MAX_HELLO_FRAME,
            max_tags: ED2K_MAX_TAGS,
            max_string_bytes: ED2K_MAX_STRING_BYTES,
            max_parts: ED2K_MAX_PARTS,
            max_blob_bytes: ED2K_MAX_BLOB_BYTES,
        }
    }
}

impl Ed2kLimits {
    /// Creates a new `Ed2kLimits` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the maximum permitted frame size for a specific opcode.
    pub fn max_frame_for_opcode(&self, opcode: u8) -> usize {
        match opcode {
            super::opcodes::OP_HELLO | super::opcodes::OP_HELLO_ANSWER => {
                self.max_hello_frame_bytes
            }
            super::opcodes::OP_FILESTATUS | super::opcodes::OP_HASHSETANSWER => {
                ED2K_MAX_STATUS_FRAME
            }
            _ => self.max_frame_bytes,
        }
    }
}
