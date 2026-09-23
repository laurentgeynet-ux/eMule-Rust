// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Protocol errors and framing failures.

use thiserror::Error;

/// Errors that occur during packet framing, boundary slicing, or stream decoding.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum FramingError {
    /// Packet length field exceeds the permitted maximum.
    #[error("Frame size {size} bytes exceeds maximum limit of {limit} bytes")]
    FrameTooLarge {
        /// Actual parsed size in bytes.
        size: usize,
        /// Maximum allowed size in bytes.
        limit: usize,
    },

    /// Packet length field is too small (must be at least 1 for the opcode).
    #[error("Frame size {size} bytes is too small (must be at least 1 byte for opcode)")]
    FrameTooSmall {
        /// Actual parsed size in bytes.
        size: usize,
    },

    /// Accumulator buffer exceeded safety threshold before a complete frame arrived.
    #[error("Connection buffer overflow: {buffered} bytes accumulated without complete frame (limit {limit})")]
    BufferOverflow {
        /// Bytes currently buffered.
        buffered: usize,
        /// Maximum allowable buffer capacity.
        limit: usize,
    },

    /// Unknown or unsupported protocol prefix byte.
    #[error("Unsupported protocol byte 0x{protocol:02X}")]
    UnsupportedProtocol {
        /// Rejected protocol byte value.
        protocol: u8,
    },

    /// Low-level I/O failure during framing.
    #[error("Framing I/O error: {0}")]
    Io(String),
}

/// High-level wire protocol and domain errors.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ProtocolError {
    /// Framing-level parsing or encoding failure.
    #[error(transparent)]
    Framing(#[from] FramingError),

    /// Received opcode did not match the expected state machine transition.
    #[error("Unexpected opcode 0x{got:02X} (expected 0x{expected:02X})")]
    UnexpectedOpcode {
        /// Expected opcode byte.
        expected: u8,
        /// Actual opcode byte received.
        got: u8,
    },

    /// Malformed packet payload data.
    #[error("Malformed packet payload: {0}")]
    MalformedPayload(String),

    /// String length exceeded maximum allowed limit.
    #[error("String length {len} exceeds limit of {limit} bytes")]
    OversizedString {
        /// Length in bytes.
        len: usize,
        /// Maximum allowed limit in bytes.
        limit: usize,
    },

    /// Number of tags exceeded maximum allowed count.
    #[error("Tag count {count} exceeds limit of {limit}")]
    TooManyTags {
        /// Tag count received.
        count: usize,
        /// Maximum allowed tag count.
        limit: usize,
    },

    /// Offset or range specification is invalid or out of bounds.
    #[error("Invalid range [{start}..{end}] for file size {file_size}")]
    InvalidRange {
        /// Range start offset.
        start: u64,
        /// Range end offset.
        end: u64,
        /// Total file size.
        file_size: u64,
    },

    /// Peer closed connection or timed out.
    #[error("Peer disconnected: {0}")]
    PeerDisconnected(String),

    /// Storage download sink error.
    #[error("Download sink error: {0}")]
    Sink(String),

    /// Download planner error.
    #[error("Download planner error: {0}")]
    Planner(String),

    /// Compressed frame (0xD4) rejected pending bounded decompression stage.
    #[error("Compressed frame 0xD4 (PR_EMULE_COMPRESSED) is not supported prior to bounded decompression stage")]
    CompressedProtocolUnsupported,
}
