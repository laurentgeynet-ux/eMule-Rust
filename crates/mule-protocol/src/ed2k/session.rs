//! Pure deterministic state machine for eD2K protocol sessions.
//! Enforces orderly handshake transitions, capability negotiation, and failure tracking.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::decoder::Ed2kDecoder;
use super::encoder::Ed2kEncoder;
use super::error::{Ed2kEncodeError, Ed2kParseError};
use super::opcodes::{OP_HELLO, OP_HELLO_ANSWER};
use super::packet::{
    Ed2kEmuleInfo, Ed2kFileStatus, Ed2kFileStatusReq, Ed2kHello, Ed2kHelloAnswer, Ed2kMessage,
    Ed2kRequestParts, Ed2kSendingPart,
};
use super::peer_info::Ed2kPeerInfo;
use super::profile::Ed2kCompatibilityProfile;
use super::protocol::{is_valid_ed2k_protocol, PR_EMULE, PR_EMULE_COMPRESSED};
use super::tags::Ed2kTag;
use crate::framing::RawPacket;

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

/// Metadata for an in-flight file status request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStatusRequest {
    /// 16-byte MD4 file hash requested.
    pub file_hash: [u8; 16],
    /// Instant when the request was dispatched.
    pub sent_at: Instant,
}

/// Bounded tracking structure for pending `OP_FILESTATUSREQ` exchanges.
#[derive(Debug, Clone)]
pub struct FileStatusTracker {
    pending: HashMap<[u8; 16], FileStatusRequest>,
    max_pending: usize,
}

impl Default for FileStatusTracker {
    fn default() -> Self {
        Self::new(16)
    }
}

impl FileStatusTracker {
    /// Creates a new tracker with a maximum in-flight capacity.
    pub fn new(max_pending: usize) -> Self {
        Self {
            pending: HashMap::with_capacity(max_pending.min(32)),
            max_pending,
        }
    }

    /// Registers a newly sent file status request.
    pub fn register_request(
        &mut self,
        file_hash: [u8; 16],
        now: Instant,
    ) -> Result<(), Ed2kSessionError> {
        if self.pending.contains_key(&file_hash) {
            return Err(Ed2kSessionError::DuplicateFileStatusRequest(hex::encode(file_hash)));
        }
        if self.pending.len() >= self.max_pending {
            return Err(Ed2kSessionError::TooManyPendingRequests(self.max_pending));
        }
        self.pending.insert(
            file_hash,
            FileStatusRequest {
                file_hash,
                sent_at: now,
            },
        );
        Ok(())
    }

    /// Validates an incoming response and removes the pending entry.
    pub fn validate_and_remove(
        &mut self,
        file_hash: &[u8; 16],
    ) -> Result<FileStatusRequest, Ed2kSessionError> {
        self.pending
            .remove(file_hash)
            .ok_or_else(|| Ed2kSessionError::UnsolicitedFileStatus(hex::encode(file_hash)))
    }

    /// Checks if a request is currently pending for the specified hash.
    pub fn is_pending(&self, file_hash: &[u8; 16]) -> bool {
        self.pending.contains_key(file_hash)
    }

    /// Returns the number of currently pending requests.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Removes and returns hashes of all requests that exceeded the timeout threshold.
    pub fn expire_timed_out(&mut self, timeout: Duration, now: Instant) -> Vec<[u8; 16]> {
        let mut expired = Vec::new();
        self.pending.retain(|hash, req| {
            if now.saturating_duration_since(req.sent_at) >= timeout {
                expired.push(*hash);
                false
            } else {
                true
            }
        });
        expired
    }

    /// Clears all pending requests (e.g. on disconnection).
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

/// Default maximum concurrent in-flight block requests per peer (8).
pub const MAX_IN_FLIGHT_BLOCK_REQUESTS: usize = 8;

/// Composite key uniquely identifying an in-flight eD2K block request.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct BlockRequestKey {
    /// 16-byte MD4 eD2K file hash.
    pub file_hash: [u8; 16],
    /// Absolute start byte offset.
    pub start: u64,
    /// Absolute end byte offset (exclusive).
    pub end: u64,
}

impl BlockRequestKey {
    /// Creates a new `BlockRequestKey`.
    pub fn new(file_hash: [u8; 16], start: u64, end: u64) -> Self {
        Self {
            file_hash,
            start,
            end,
        }
    }

    /// Length in bytes of the requested block range.
    #[inline]
    pub fn len(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Returns true if start >= end.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// Information tracking an active in-flight block request.
#[derive(Clone, Debug)]
pub struct PendingBlockRequest {
    /// Unique composite key.
    pub key: BlockRequestKey,
    /// Timestamp when this request was issued.
    pub requested_at: Instant,
    /// Retry attempt counter.
    pub attempt: u16,
    /// Byte ranges received so far for this request.
    pub received_ranges: Vec<(u64, u64)>,
}

/// Bounded tracking structure for pending `OP_REQUESTPARTS` exchanges.
#[derive(Debug, Clone)]
pub struct BlockRequestTracker {
    pending: HashMap<BlockRequestKey, PendingBlockRequest>,
    max_pending: usize,
}

impl Default for BlockRequestTracker {
    fn default() -> Self {
        Self::new(MAX_IN_FLIGHT_BLOCK_REQUESTS)
    }
}

impl BlockRequestTracker {
    /// Creates a new tracker with a maximum in-flight capacity.
    pub fn new(max_pending: usize) -> Self {
        Self {
            pending: HashMap::with_capacity(max_pending.min(32)),
            max_pending,
        }
    }

    /// Returns true if the tracker has capacity for more requests.
    #[inline]
    pub fn can_request(&self) -> bool {
        self.pending.len() < self.max_pending
    }

    /// Returns the number of active in-flight block requests.
    #[inline]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Returns true if there are no pending block requests.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Checks if an exact request key is currently pending.
    pub fn is_pending(&self, key: &BlockRequestKey) -> bool {
        self.pending.contains_key(key)
    }

    /// Checks if any pending request for the given file hash overlaps [start..end].
    pub fn has_overlap(&self, file_hash: &[u8; 16], start: u64, end: u64) -> bool {
        self.pending.keys().any(|k| {
            &k.file_hash == file_hash && k.start < end && start < k.end
        })
    }

    /// Registers a newly sent block request.
    pub fn register_request(
        &mut self,
        key: BlockRequestKey,
        now: Instant,
    ) -> Result<(), Ed2kSessionError> {
        if key.start >= key.end {
            return Err(Ed2kSessionError::InvalidBlockRequest(format!(
                "start offset {} must be strictly less than end offset {}",
                key.start, key.end
            )));
        }

        if self.pending.len() >= self.max_pending {
            return Err(Ed2kSessionError::TooManyPendingBlockRequests(self.max_pending));
        }

        if self.has_overlap(&key.file_hash, key.start, key.end) {
            return Err(Ed2kSessionError::DuplicateBlockRequest(format!(
                "{}:{}-{}",
                hex::encode(key.file_hash),
                key.start,
                key.end
            )));
        }

        self.pending.insert(
            key.clone(),
            PendingBlockRequest {
                key,
                requested_at: now,
                attempt: 1,
                received_ranges: Vec::new(),
            },
        );

        Ok(())
    }

    /// Validates an incoming chunk response and removes the pending entry when fully received.
    pub fn validate_and_remove(
        &mut self,
        file_hash: &[u8; 16],
        start: u64,
        end: u64,
    ) -> Result<PendingBlockRequest, Ed2kSessionError> {
        if start >= end {
            return Err(Ed2kSessionError::UnsolicitedBlock(format!(
                "{}:{}-{}",
                hex::encode(file_hash),
                start,
                end
            )));
        }

        let matching_key = self
            .pending
            .keys()
            .find(|k| {
                &k.file_hash == file_hash && start >= k.start && end <= k.end
            })
            .cloned();

        let key = matching_key.ok_or_else(|| {
            Ed2kSessionError::UnsolicitedBlock(format!(
                "{}:{}-{}",
                hex::encode(file_hash),
                start,
                end
            ))
        })?;

        let is_complete = {
            let req = match self.pending.get_mut(&key) {
                Some(r) => r,
                None => {
                    return Err(Ed2kSessionError::UnsolicitedBlock(format!(
                        "{}:{}-{}",
                        hex::encode(file_hash),
                        start,
                        end
                    )));
                }
            };

            if req.received_ranges.iter().any(|&(s, e)| s == start && e == end) {
                return Err(Ed2kSessionError::DuplicateBlock(format!(
                    "{}:{}-{}",
                    hex::encode(file_hash),
                    start,
                    end
                )));
            }

            req.received_ranges.push((start, end));
            let total_received: u64 = req.received_ranges.iter().map(|&(s, e)| e - s).sum();
            total_received >= key.len()
        };

        if is_complete {
            self.pending.remove(&key).ok_or_else(|| {
                Ed2kSessionError::UnsolicitedBlock(format!(
                    "{}:{}-{}",
                    hex::encode(file_hash),
                    start,
                    end
                ))
            })
        } else {
            self.pending.get(&key).cloned().ok_or_else(|| {
                Ed2kSessionError::UnsolicitedBlock(format!(
                    "{}:{}-{}",
                    hex::encode(file_hash),
                    start,
                    end
                ))
            })
        }
    }

    /// Removes and returns all requests that exceeded the timeout threshold.
    pub fn reap_timeouts(&mut self, timeout: Duration, now: Instant) -> Vec<PendingBlockRequest> {
        let mut expired = Vec::new();
        self.pending.retain(|_, req| {
            if now.saturating_duration_since(req.requested_at) >= timeout {
                expired.push(req.clone());
                false
            } else {
                true
            }
        });
        expired
    }

    /// Clears all in-flight requests and returns them.
    pub fn cancel_all(&mut self) -> Vec<PendingBlockRequest> {
        self.pending.drain().map(|(_, req)| req).collect()
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

/// Pure deterministic state machine governing an eD2K peer session.
#[derive(Debug, Clone)]
pub struct Ed2kSession {
    /// Current state of the session.
    pub state: Ed2kSessionState,
    /// True if this node initiated the handshake.
    pub is_initiator: bool,
    /// Local 16-byte MD4 user hash.
    pub local_user_hash: [u8; 16],
    /// Local client ID.
    pub local_client_id: u32,
    /// Local listening TCP port.
    pub local_tcp_port: u16,
    /// Wire decoder with safety bounds.
    pub decoder: Ed2kDecoder,
    /// Wire encoder with safety bounds.
    pub encoder: Ed2kEncoder,
    /// Pending file status requests tracker.
    pub status_tracker: FileStatusTracker,
    /// Pending block requests tracker.
    pub block_tracker: BlockRequestTracker,
    /// Counter of unhandled, non-state-mutating raw or extension frames observed.
    pub unhandled_packets_count: usize,
    /// Cached eMule info received before or alongside handshake answer.
    pub pending_emule_info: Option<Ed2kEmuleInfo>,
}

impl Ed2kSession {
    /// Takes a deterministic snapshot of the session state and tracker counters.
    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            state_name: self.state.name(),
            is_ready: self.is_ready(),
            pending_status_count: self.status_tracker.pending_count(),
            pending_block_count: self.block_tracker.pending_count(),
            unhandled_packets_count: self.unhandled_packets_count,
        }
    }

    /// Creates a new session configured as the connection initiator with default profile (`Emule050a`).
    pub fn new_initiator(
        local_user_hash: [u8; 16],
        local_client_id: u32,
        local_tcp_port: u16,
    ) -> Self {
        Self::new_initiator_with_profile(
            local_user_hash,
            local_client_id,
            local_tcp_port,
            Ed2kCompatibilityProfile::default(),
        )
    }

    /// Creates a new session configured as the connection initiator with an explicit profile.
    pub fn new_initiator_with_profile(
        local_user_hash: [u8; 16],
        local_client_id: u32,
        local_tcp_port: u16,
        profile: Ed2kCompatibilityProfile,
    ) -> Self {
        Self {
            state: Ed2kSessionState::Connecting,
            is_initiator: true,
            local_user_hash,
            local_client_id,
            local_tcp_port,
            decoder: Ed2kDecoder::with_profile(profile),
            encoder: Ed2kEncoder::with_profile(profile),
            status_tracker: FileStatusTracker::default(),
            block_tracker: BlockRequestTracker::default(),
            unhandled_packets_count: 0,
            pending_emule_info: None,
        }
    }

    /// Creates a new session configured as a listener awaiting an incoming handshake with default profile.
    pub fn new_listener(
        local_user_hash: [u8; 16],
        local_client_id: u32,
        local_tcp_port: u16,
    ) -> Self {
        Self::new_listener_with_profile(
            local_user_hash,
            local_client_id,
            local_tcp_port,
            Ed2kCompatibilityProfile::default(),
        )
    }

    /// Creates a new session configured as a listener with an explicit profile.
    pub fn new_listener_with_profile(
        local_user_hash: [u8; 16],
        local_client_id: u32,
        local_tcp_port: u16,
        profile: Ed2kCompatibilityProfile,
    ) -> Self {
        Self {
            state: Ed2kSessionState::AwaitingHello,
            is_initiator: false,
            local_user_hash,
            local_client_id,
            local_tcp_port,
            decoder: Ed2kDecoder::with_profile(profile),
            encoder: Ed2kEncoder::with_profile(profile),
            status_tracker: FileStatusTracker::default(),
            block_tracker: BlockRequestTracker::default(),
            unhandled_packets_count: 0,
            pending_emule_info: None,
        }
    }

    /// Transitions to `AwaitingHelloAnswer` and constructs the outgoing `Ed2kHello` message.
    pub fn create_hello(
        &mut self,
        tags: Vec<Ed2kTag>,
        server_ip: u32,
        server_port: u16,
    ) -> Result<Ed2kHello, Ed2kSessionError> {
        match self.state {
            Ed2kSessionState::Disconnected | Ed2kSessionState::Connecting => {
                let hello = Ed2kHello {
                    user_hash: self.local_user_hash,
                    client_id: self.local_client_id,
                    tcp_port: self.local_tcp_port,
                    tags,
                    unknown_tags: vec![],
                    server_ip,
                    server_port,
                };
                self.state = Ed2kSessionState::AwaitingHelloAnswer {
                    local_handshake_id: self.local_user_hash,
                };
                Ok(hello)
            }
            _ => {
                let err = Ed2kSessionError::UnexpectedMessageInState {
                    state: self.state.name(),
                    opcode: OP_HELLO,
                };
                self.fail(err.clone());
                Err(err)
            }
        }
    }

    /// Processes an incoming `Ed2kHello` message on a listener session.
    pub fn on_hello_received(
        &mut self,
        hello: &Ed2kHello,
    ) -> Result<Ed2kPeerInfo, Ed2kSessionError> {
        match &self.state {
            Ed2kSessionState::AwaitingHello => {
                if hello.user_hash == [0u8; 16] {
                    let err = Ed2kSessionError::ZeroUserHash;
                    self.fail(err.clone());
                    return Err(err);
                }

                let mut peer = Ed2kPeerInfo::from_hello(hello);
                if peer.emule_info.is_none() {
                    peer.emule_info = self.pending_emule_info.take();
                }
                Ok(peer)
            }
            Ed2kSessionState::Ready { .. } | Ed2kSessionState::AwaitingHelloAnswer { .. } => {
                let err = Ed2kSessionError::DoubleHello;
                self.fail(err.clone());
                Err(err)
            }
            _ => {
                let err = Ed2kSessionError::UnexpectedMessageInState {
                    state: self.state.name(),
                    opcode: OP_HELLO,
                };
                self.fail(err.clone());
                Err(err)
            }
        }
    }

    /// Completes listener handshake by generating `Ed2kHelloAnswer` and transitioning to `Ready`.
    pub fn complete_handshake_as_listener(
        &mut self,
        remote_peer: Ed2kPeerInfo,
        tags: Vec<Ed2kTag>,
        server_ip: u32,
        server_port: u16,
    ) -> Result<Ed2kHelloAnswer, Ed2kSessionError> {
        match self.state {
            Ed2kSessionState::AwaitingHello => {
                let answer = Ed2kHelloAnswer {
                    user_hash: self.local_user_hash,
                    client_id: self.local_client_id,
                    tcp_port: self.local_tcp_port,
                    tags,
                    unknown_tags: vec![],
                    server_ip,
                    server_port,
                };
                let mut peer = remote_peer;
                if peer.emule_info.is_none() {
                    peer.emule_info = self.pending_emule_info.take();
                }
                self.state = Ed2kSessionState::Ready { peer };
                Ok(answer)
            }
            _ => {
                let err = Ed2kSessionError::UnexpectedMessageInState {
                    state: self.state.name(),
                    opcode: OP_HELLO_ANSWER,
                };
                self.fail(err.clone());
                Err(err)
            }
        }
    }

    /// Processes an incoming `Ed2kHelloAnswer` message on an initiator session.
    pub fn on_hello_answer_received(
        &mut self,
        answer: &Ed2kHelloAnswer,
    ) -> Result<Ed2kPeerInfo, Ed2kSessionError> {
        match &self.state {
            Ed2kSessionState::AwaitingHello => {
                let err = Ed2kSessionError::HelloAnswerBeforeHello;
                self.fail(err.clone());
                Err(err)
            }
            Ed2kSessionState::AwaitingHelloAnswer { .. } => {
                if answer.user_hash == [0u8; 16] {
                    let err = Ed2kSessionError::ZeroUserHash;
                    self.fail(err.clone());
                    return Err(err);
                }

                let mut peer = Ed2kPeerInfo::from_hello_answer(answer);
                if peer.emule_info.is_none() {
                    peer.emule_info = self.pending_emule_info.take();
                }
                self.state = Ed2kSessionState::Ready { peer: peer.clone() };
                Ok(peer)
            }
            _ => {
                let err = Ed2kSessionError::UnexpectedMessageInState {
                    state: self.state.name(),
                    opcode: OP_HELLO_ANSWER,
                };
                self.fail(err.clone());
                Err(err)
            }
        }
    }

    /// Processes any raw packet received from the wire transport.
    pub fn on_packet_received(
        &mut self,
        packet: &RawPacket,
    ) -> Result<Option<Ed2kMessage>, Ed2kSessionError> {
        if packet.protocol == PR_EMULE_COMPRESSED {
            let err = Ed2kSessionError::CompressedProtocolUnsupported;
            self.fail(err.clone());
            return Err(err);
        }

        if !is_valid_ed2k_protocol(packet.protocol) {
            let err = Ed2kSessionError::UnexpectedProtocol {
                protocol: packet.protocol,
            };
            self.fail(err.clone());
            return Err(err);
        }

        let decoded_msg = match self.decoder.decode_raw_packet(packet) {
            Ok(m) => m,
            Err(e) => {
                let err = Ed2kSessionError::Parse(e.to_string());
                self.fail(err.clone());
                return Err(err);
            }
        };

        match decoded_msg {
            Ed2kMessage::Hello(hello) => {
                let _peer = self.on_hello_received(&hello)?;
                Ok(Some(Ed2kMessage::Hello(hello)))
            }
            Ed2kMessage::HelloAnswer(answer) => {
                let _peer = self.on_hello_answer_received(&answer)?;
                Ok(Some(Ed2kMessage::HelloAnswer(answer)))
            }
            Ed2kMessage::EmuleInfo(info) => {
                match &mut self.state {
                    Ed2kSessionState::Ready { peer } => {
                        peer.emule_info = Some(info.clone());
                    }
                    _ => {
                        self.pending_emule_info = Some(info.clone());
                    }
                }
                self.unhandled_packets_count += 1;
                Ok(Some(Ed2kMessage::EmuleInfo(info)))
            }
            Ed2kMessage::FileStatus(status) => {
                if !self.is_ready() {
                    let err = Ed2kSessionError::ResponseBeforeReady(self.state.name());
                    self.fail(err.clone());
                    return Err(err);
                }
                self.on_file_status_received(&status)?;
                Ok(Some(Ed2kMessage::FileStatus(status)))
            }
            Ed2kMessage::FileReqAnsNoFile(ans) => {
                if !self.is_ready() {
                    let err = Ed2kSessionError::ResponseBeforeReady(self.state.name());
                    self.fail(err.clone());
                    return Err(err);
                }
                if let Err(err) = self.status_tracker.validate_and_remove(&ans.file_hash) {
                    self.fail(err.clone());
                    return Err(err);
                }
                Ok(Some(Ed2kMessage::FileReqAnsNoFile(ans)))
            }
            Ed2kMessage::FileStatusReq(req) => {
                if !self.is_ready() {
                    let err = Ed2kSessionError::RequestBeforeReady(self.state.name());
                    self.fail(err.clone());
                    return Err(err);
                }
                Ok(Some(Ed2kMessage::FileStatusReq(req)))
            }
            Ed2kMessage::RequestParts(req) => {
                if !self.is_ready() {
                    let err = Ed2kSessionError::RequestBeforeReady(self.state.name());
                    self.fail(err.clone());
                    return Err(err);
                }
                Ok(Some(Ed2kMessage::RequestParts(req)))
            }
            Ed2kMessage::SendingPart(part) => {
                if !self.is_ready() {
                    let err = Ed2kSessionError::ResponseBeforeReady(self.state.name());
                    self.fail(err.clone());
                    return Err(err);
                }
                let _fulfilled = match self.on_sending_part_received(&part) {
                    Ok(f) => f,
                    Err(e) => {
                        self.fail(e.clone());
                        return Err(e);
                    }
                };
                Ok(Some(Ed2kMessage::SendingPart(part)))
            }
            Ed2kMessage::PublicKey(pk) => Ok(Some(Ed2kMessage::PublicKey(pk))),
            Ed2kMessage::Signature(sig) => Ok(Some(Ed2kMessage::Signature(sig))),
            Ed2kMessage::SecIdentState(state) => Ok(Some(Ed2kMessage::SecIdentState(state))),
            Ed2kMessage::Raw(raw) => {
                if !self.is_ready() && raw.protocol != PR_EMULE {
                    let err = Ed2kSessionError::UnexpectedMessageInState {
                        state: self.state.name(),
                        opcode: raw.opcode,
                    };
                    self.fail(err.clone());
                    return Err(err);
                }
                self.unhandled_packets_count += 1;
                Ok(Some(Ed2kMessage::Raw(raw)))
            }
        }
    }

    /// Initiates an `OP_FILESTATUSREQ` exchange and returns the raw wire packet to transmit.
    pub fn send_file_status_request(
        &mut self,
        file_hash: [u8; 16],
        now: Instant,
    ) -> Result<RawPacket, Ed2kSessionError> {
        if !self.is_ready() {
            let err = Ed2kSessionError::RequestBeforeReady(self.state.name());
            self.fail(err.clone());
            return Err(err);
        }

        self.status_tracker.register_request(file_hash, now)?;
        let req = Ed2kFileStatusReq { file_hash };
        let packet = self.encoder.encode_file_status_req(&req)?;
        Ok(packet)
    }

    /// Validates an incoming `Ed2kFileStatus` message against in-flight requests.
    pub fn on_file_status_received(
        &mut self,
        status: &Ed2kFileStatus,
    ) -> Result<(), Ed2kSessionError> {
        if !self.is_ready() {
            let err = Ed2kSessionError::ResponseBeforeReady(self.state.name());
            self.fail(err.clone());
            return Err(err);
        }

        // Note: In official eD2K wire protocol (eMule/aMule), part_count == 0 indicates
        // that the remote peer possesses 100% of the completed file (CKnownFile),
        // meaning all parts are available and no bitfield is transmitted.

        // Must match an in-flight request and remove it
        if let Err(err) = self.status_tracker.validate_and_remove(&status.file_hash) {
            self.fail(err.clone());
            return Err(err);
        }

        Ok(())
    }

    /// Purges timed-out requests and returns their hashes.
    pub fn expire_file_status_requests(
        &mut self,
        timeout: Duration,
        now: Instant,
    ) -> Vec<[u8; 16]> {
        self.status_tracker.expire_timed_out(timeout, now)
    }

    /// Initiates an `OP_REQUESTPARTS` block request and returns the raw wire packet to transmit.
    pub fn send_block_request(
        &mut self,
        file_hash: [u8; 16],
        start: u64,
        end: u64,
        file_size: u64,
        now: Instant,
    ) -> Result<RawPacket, Ed2kSessionError> {
        if !self.is_ready() {
            let err = Ed2kSessionError::RequestBeforeReady(self.state.name());
            self.fail(err.clone());
            return Err(err);
        }

        if start >= end {
            return Err(Ed2kSessionError::InvalidBlockRequest(format!(
                "start offset {start} must be strictly less than end offset {end}"
            )));
        }

        if end > file_size {
            return Err(Ed2kSessionError::InvalidBlockRequest(format!(
                "end offset {end} exceeds total file size {file_size}"
            )));
        }

        let block_len = end - start;
        if block_len > 184_320 {
            return Err(Ed2kSessionError::InvalidBlockRequest(format!(
                "requested block length {block_len} exceeds maximum allowed 184,320 bytes"
            )));
        }

        let key = BlockRequestKey::new(file_hash, start, end);
        self.block_tracker.register_request(key, now)?;

        let req = Ed2kRequestParts::single(file_hash, start as u32, end as u32);
        self.encoder.encode_request_parts(&req).map_err(Into::into)
    }

    /// Processes an incoming `Ed2kSendingPart` block and removes the correlated pending request.
    pub fn on_sending_part_received(
        &mut self,
        part: &Ed2kSendingPart,
    ) -> Result<PendingBlockRequest, Ed2kSessionError> {
        if !self.is_ready() {
            let err = Ed2kSessionError::ResponseBeforeReady(self.state.name());
            self.fail(err.clone());
            return Err(err);
        }

        self.block_tracker
            .validate_and_remove(&part.file_hash, part.start_offset as u64, part.end_offset as u64)
    }

    /// Purges timed-out block requests and returns them for replanning.
    pub fn expire_block_requests(
        &mut self,
        timeout: Duration,
        now: Instant,
    ) -> Vec<PendingBlockRequest> {
        self.block_tracker.reap_timeouts(timeout, now)
    }

    /// Cancels all active in-flight block requests (e.g. upon disconnection).
    pub fn cancel_all_block_requests(&mut self) -> Vec<PendingBlockRequest> {
        self.block_tracker.cancel_all()
    }

    /// Marks the session as timed out.
    pub fn on_timeout(&mut self, duration: Duration) {
        self.status_tracker.clear();
        let _ = self.block_tracker.cancel_all();
        self.state = Ed2kSessionState::Failed {
            reason: Ed2kSessionError::Timeout(duration),
        };
    }

    /// Handles remote transport closure.
    pub fn on_remote_closed(&mut self) {
        self.status_tracker.clear();
        let _ = self.block_tracker.cancel_all();
        match self.state {
            Ed2kSessionState::Ready { .. } | Ed2kSessionState::Closing => {
                self.state = Ed2kSessionState::Closed;
            }
            Ed2kSessionState::Closed => {}
            _ => {
                self.state = Ed2kSessionState::Failed {
                    reason: Ed2kSessionError::Io("Connection closed prematurely by remote peer".to_string()),
                };
            }
        }
    }

    /// Transitions to irreversible terminal failed state.
    pub fn fail(&mut self, reason: Ed2kSessionError) {
        self.status_tracker.clear();
        let _ = self.block_tracker.cancel_all();
        self.state = Ed2kSessionState::Failed { reason };
    }

    /// Returns true if the session has successfully completed the handshake and is ready.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, Ed2kSessionState::Ready { .. })
    }

    /// Returns true if the session is in a terminal Failed state.
    pub fn is_failed(&self) -> bool {
        matches!(self.state, Ed2kSessionState::Failed { .. })
    }

    /// Returns the number of currently active in-flight block requests.
    pub fn pending_block_request_count(&self) -> usize {
        self.block_tracker.pending_count()
    }

    /// Returns a reference to the negotiated remote peer info if in `Ready` state.
    pub fn peer_info(&self) -> Option<&Ed2kPeerInfo> {
        match &self.state {
            Ed2kSessionState::Ready { peer } => Some(peer),
            _ => None,
        }
    }

    /// Returns a reference to the status tracker.
    pub fn status_tracker(&self) -> &FileStatusTracker {
        &self.status_tracker
    }

    /// Returns a mutable reference to the status tracker.
    pub fn status_tracker_mut(&mut self) -> &mut FileStatusTracker {
        &mut self.status_tracker
    }

    /// Returns a reference to the block tracker.
    pub fn block_tracker(&self) -> &BlockRequestTracker {
        &self.block_tracker
    }

    /// Returns a mutable reference to the block tracker.
    pub fn block_tracker_mut(&mut self) -> &mut BlockRequestTracker {
        &mut self.block_tracker
    }
}
