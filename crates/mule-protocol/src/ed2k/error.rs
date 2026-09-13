//! Structured and contextual error types for eD2K parsing.

use std::fmt;
use thiserror::Error;

/// Specific category of an eD2K parsing failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ed2kErrorKind {
    /// Unexpected end of input buffer while reading a required field.
    UnexpectedEof {
        /// Number of bytes needed.
        needed: usize,
        /// Number of bytes actually available.
        available: usize,
    },
    /// Protocol byte is not recognized as valid eD2K or eMule.
    UnsupportedProtocol {
        /// Rejected protocol byte value.
        byte: u8,
    },
    /// Compressed protocol (0xD4) is explicitly rejected pending bounded decompression stage.
    CompressedProtocolUnsupported,
    /// Frame length field is smaller than the minimum 1 byte for opcode.
    FrameTooSmall {
        /// Parsed frame length.
        size: usize,
    },
    /// Frame size exceeds the configured safety limit.
    FrameTooLarge {
        /// Parsed frame length.
        size: usize,
        /// Safety ceiling.
        limit: usize,
    },
    /// A string field exceeds the configured maximum length.
    OversizedString {
        /// String byte length.
        size: usize,
        /// Safety limit.
        limit: usize,
    },
    /// Number of tags exceeds the maximum permissible limit.
    TooManyTags {
        /// Declared tag count.
        count: usize,
        /// Safety limit.
        limit: usize,
    },
    /// Tag type byte is unhandled or invalid.
    InvalidTagType {
        /// Tag type byte.
        tag_type: u8,
    },
    /// Malformed field or semantic constraint violation.
    MalformedField {
        /// Diagnostic message.
        message: String,
    },
}

impl fmt::Display for Ed2kErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { needed, available } => {
                write!(
                    f,
                    "unexpected EOF: needed {needed} bytes, but only {available} available"
                )
            }
            Self::UnsupportedProtocol { byte } => {
                write!(f, "unsupported protocol byte 0x{byte:02X}")
            }
            Self::CompressedProtocolUnsupported => {
                write!(
                    f,
                    "compressed protocol 0xD4 (PR_EMULE_COMPRESSED) is not supported prior to bounded decompression stage"
                )
            }
            Self::FrameTooSmall { size } => {
                write!(f, "frame size {size} is too small (minimum 1 for opcode)")
            }
            Self::FrameTooLarge { size, limit } => {
                write!(f, "frame size {size} exceeds limit of {limit} bytes")
            }
            Self::OversizedString { size, limit } => {
                write!(f, "string length {size} exceeds limit of {limit} bytes")
            }
            Self::TooManyTags { count, limit } => {
                write!(f, "tag count {count} exceeds limit of {limit}")
            }
            Self::InvalidTagType { tag_type } => {
                write!(f, "invalid tag type byte 0x{tag_type:02X}")
            }
            Self::MalformedField { message } => {
                write!(f, "malformed field: {message}")
            }
        }
    }
}

/// Rich, contextual error for eD2K wire decoding failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub struct Ed2kParseError {
    /// Wire protocol byte (e.g. 0xE3, 0xC5, 0xD4).
    pub protocol: u8,
    /// Operation code if known at the time of error.
    pub opcode: Option<u8>,
    /// Byte offset in the input buffer where the failure occurred.
    pub offset: usize,
    /// High-level context or field name where parsing failed.
    pub context: &'static str,
    /// Specific failure category.
    pub kind: Ed2kErrorKind,
}

impl Ed2kParseError {
    /// Creates a new `Ed2kParseError`.
    pub fn new(
        protocol: u8,
        opcode: Option<u8>,
        offset: usize,
        context: &'static str,
        kind: Ed2kErrorKind,
    ) -> Self {
        Self {
            protocol,
            opcode,
            offset,
            context,
            kind,
        }
    }
}

impl fmt::Display for Ed2kParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "eD2K parse error [protocol=0x{:02X}, opcode={:?}, offset={}, context={}]: {}",
            self.protocol,
            self.opcode.map(|op| format!("0x{op:02X}")),
            self.offset,
            self.context,
            self.kind
        )
    }
}
