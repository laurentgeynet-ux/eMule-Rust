// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! Types and error definitions for eD2K sessions.

use std::time::Duration;

use super::super::error::{Ed2kEncodeError, Ed2kParseError};
use super::super::peer_info::Ed2kPeerInfo;

/// Category of an eD2K session protocol violation or failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Ed2kSessionError {
    /// An unexpected message arrived for the current session state.
    #[error("Unexpected opcode 0x{opcode:02X} in state {state}")]
    UnexpectedMessageInState {
        /// Current state name.
        state: &'static str,
        /// Operation code received.
        opcode: u8,
    },

    /// Hello was received more than once.
    #[error("Double Hello rejected: session already initialized")]
    DoubleHello,

    /// HelloAnswer was received when a Hello was expected first.
    #[error("HelloAnswer received before Hello")]
    HelloAnswerBeforeHello,

    /// A required tag was missing or invalid.
    #[error("Missing mandatory tag: {0}")]
    MissingRequiredTag(&'static str),

    /// Incompatible peer version or capabilities.
    #[error("Incompatible peer version")]
    IncompatiblePeer,

    /// Remote peer provided an invalid user hash (all zeros).
    #[error("Invalid user hash: cannot be all zeros")]
    ZeroUserHash,

    /// Attempted to send a request before the session entered the Ready state.
    #[error("Cannot send request before session is Ready (current state: {0})")]
    RequestBeforeReady(&'static str),

    /// Received a response before the session entered the Ready state.
    #[error("Received response before session is Ready (current state: {0})")]
    ResponseBeforeReady(&'static str),

    /// Received a file status without an in-flight request or with mismatched hash.
    #[error("Unsolicited file status received for hash {0}")]
    UnsolicitedFileStatus(String),

    /// Remote peer reported it does not have the requested file (OP_FILEREQANSNOFILE).
    #[error("Remote peer does not have requested file: {0}")]
    FileNotFound(String),

    /// A duplicate file status request was initiated for an already in-flight file hash.
    #[error("Duplicate file status request for hash {0}")]
    DuplicateFileStatusRequest(String),

    /// In-flight requests exceeded the maximum allowed capacity.
    #[error("Too many pending file status requests (capacity: {0})")]
    TooManyPendingRequests(usize),

    /// Received a block without an in-flight request or with mismatched hash/range.
    #[error("Unsolicited block received: {0}")]
    UnsolicitedBlock(String),

    /// A duplicate or overlapping block request was initiated.
    #[error("Duplicate or overlapping block request: {0}")]
    DuplicateBlockRequest(String),

    /// A duplicate block chunk was received.
    #[error("Duplicate block received: {0}")]
    DuplicateBlock(String),

    /// In-flight block requests exceeded the maximum allowed capacity.
    #[error("Too many pending block requests (capacity: {0})")]
    TooManyPendingBlockRequests(usize),

    /// Block request timed out.
    #[error("Block request timed out: {0}")]
    BlockTimeout(String),

    /// Invalid block request parameters.
    #[error("Invalid block request: {0}")]
    InvalidBlockRequest(String),

    /// Malformed packet structure.
    #[error("Malformed packet: {0}")]
    Malformed(String),

    /// Parse failure on incoming wire buffer.
    #[error("Parse error: {0}")]
    Parse(String),

    /// Unexpected or unsupported wire protocol byte.
    #[error("Unexpected protocol byte 0x{protocol:02X} (expected 0xE3 PR_ED2K or 0xC5 PR_EMULE)")]
    UnexpectedProtocol {
        /// Rejected protocol byte.
        protocol: u8,
    },

    /// Compressed frame 0xD4 (PR_EMULE_COMPRESSED) is not supported.
    #[error("Compressed protocol 0xD4 (PR_EMULE_COMPRESSED) is not supported")]
    CompressedProtocolUnsupported,

    /// Handshake negotiation timeout elapsed.
    #[error("Handshake timeout after {0:?}")]
    Timeout(Duration),

    /// File status request timed out.
    #[error("File status request timed out for hash {0}")]
    FileStatusTimeout(String),

    /// Connection reset or I/O failure.
    #[error("I/O error: {0}")]
    Io(String),
}

impl From<Ed2kParseError> for Ed2kSessionError {
    fn from(err: Ed2kParseError) -> Self {
        Self::Parse(err.to_string())
    }
}

impl From<Ed2kEncodeError> for Ed2kSessionError {
    fn from(err: Ed2kEncodeError) -> Self {
        Self::Malformed(err.to_string())
    }
}

/// Lifecycle states of an eD2K peer session.
#[derive(Debug, Clone, PartialEq)]
pub enum Ed2kSessionState {
    /// Initial disconnected state.
    Disconnected,
    /// Actively establishing transport connection.
    Connecting,
    /// Listener waiting for the initiator's `OP_HELLO`.
    AwaitingHello,
    /// Initiator waiting for the receiver's `OP_HELLO_ANSWER`.
    AwaitingHelloAnswer {
        /// Local user hash identifying this client instance in the handshake.
        local_handshake_id: [u8; 16],
    },
    /// Handshake successfully completed, ready for peer operations.
    Ready {
        /// Negotiated remote peer identity and metadata.
        peer: Ed2kPeerInfo,
    },
    /// Gracefully closing the session.
    Closing,
    /// Cleanly terminated session.
    Closed,
    /// Irreversible terminal error state.
    Failed {
        /// Reason for failure.
        reason: Ed2kSessionError,
    },
}

impl Ed2kSessionState {
    /// Returns a human-readable display string for this state.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::AwaitingHello => "AwaitingHello",
            Self::AwaitingHelloAnswer { .. } => "AwaitingHelloAnswer",
            Self::Ready { .. } => "Ready",
            Self::Closing => "Closing",
            Self::Closed => "Closed",
            Self::Failed { .. } => "Failed",
        }
    }
}

/// Pure deterministic snapshot capturing the protocol and tracker state of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    /// Lifecycle state name.
    pub state_name: &'static str,
    /// Whether session is fully established and ready for data transfer.
    pub is_ready: bool,
    /// Active in-flight file status request count.
    pub pending_status_count: usize,
    /// Active in-flight block request count.
    pub pending_block_count: usize,
    /// Count of unhandled / raw packets received.
    pub unhandled_packets_count: usize,
}
