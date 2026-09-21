//! Pure deterministic state machine for eD2K protocol sessions.
//! Enforces orderly handshake transitions, capability negotiation, and failure tracking.

use std::time::{Duration, Instant};

use super::decoder::Ed2kDecoder;
use super::encoder::Ed2kEncoder;
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

pub mod tracker;
pub mod types;

pub use tracker::{
    BlockRequestKey, BlockRequestTracker, FileStatusRequest, FileStatusTracker,
    PendingBlockRequest, MAX_IN_FLIGHT_BLOCK_REQUESTS,
};
pub use types::{Ed2kSessionError, Ed2kSessionState, SessionSnapshot};

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

    /// Creates a new session configured as the connection initiator with default profile (`Emule072a`).
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
            Ed2kMessage::HashSetReq(req) => Ok(Some(Ed2kMessage::HashSetReq(req))),
            Ed2kMessage::HashSetAnswer(ans) => Ok(Some(Ed2kMessage::HashSetAnswer(ans))),
            Ed2kMessage::AichFileHashReq(req) => Ok(Some(Ed2kMessage::AichFileHashReq(req))),
            Ed2kMessage::AichFileHashRes(res) => Ok(Some(Ed2kMessage::AichFileHashRes(res))),
            Ed2kMessage::AichRequest(req) => Ok(Some(Ed2kMessage::AichRequest(req))),
            Ed2kMessage::AichAnswer(ans) => Ok(Some(Ed2kMessage::AichAnswer(ans))),
            Ed2kMessage::RequestPartsI64(req) => Ok(Some(Ed2kMessage::RequestPartsI64(req))),
            Ed2kMessage::SendingPartI64(part) => Ok(Some(Ed2kMessage::SendingPartI64(part))),
            Ed2kMessage::CompressedPartI64(part) => Ok(Some(Ed2kMessage::CompressedPartI64(part))),
            // eMule 0.72a extended messages (hashset2, multipacket, queue, chat,
            // CAPTCHA…): decoded but not yet wired into the session state
            // machine — counted as unhandled like `Raw` packets.
            msg @ (Ed2kMessage::Message(_)
            | Ed2kMessage::QueueRank(_)
            | Ed2kMessage::QueueRanking(_)
            | Ed2kMessage::ChangeSlot(_)
            | Ed2kMessage::FileDesc(_)
            | Ed2kMessage::PortTest(_)
            | Ed2kMessage::CaptchaRequest(_)
            | Ed2kMessage::CaptchaResponse(_)
            | Ed2kMessage::HashSetReq2(_)
            | Ed2kMessage::HashSetAnswer2(_)
            | Ed2kMessage::MultiPacketRequest(_)
            | Ed2kMessage::MultiPacketAnswer(_)) => {
                if !self.is_ready() {
                    let err = Ed2kSessionError::UnexpectedMessageInState {
                        state: self.state.name(),
                        opcode: 0,
                    };
                    self.fail(err.clone());
                    return Err(err);
                }
                self.unhandled_packets_count += 1;
                Ok(Some(msg))
            }
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

        self.block_tracker.validate_and_remove(
            &part.file_hash,
            part.start_offset as u64,
            part.end_offset as u64,
        )
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
                    reason: Ed2kSessionError::Io(
                        "Connection closed prematurely by remote peer".to_string(),
                    ),
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
