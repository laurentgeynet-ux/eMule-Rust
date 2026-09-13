//! Pure deterministic state machine for eD2K protocol sessions.
//! Enforces orderly handshake transitions, capability negotiation, and failure tracking.

use std::time::Duration;

use super::decoder::Ed2kDecoder;
use super::error::Ed2kParseError;
use super::opcodes::{OP_HELLO, OP_HELLO_ANSWER};
use super::packet::{Ed2kHello, Ed2kHelloAnswer, Ed2kMessage};
use super::peer_info::Ed2kPeerInfo;
use super::protocol::{is_valid_ed2k_protocol, PR_ED2K, PR_EMULE_COMPRESSED};
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

    /// Connection reset or I/O failure.
    #[error("I/O error: {0}")]
    Io(String),
}

impl From<Ed2kParseError> for Ed2kSessionError {
    fn from(err: Ed2kParseError) -> Self {
        Self::Parse(err.to_string())
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
}

impl Ed2kSession {
    /// Creates a new session configured as the connection initiator.
    pub fn new_initiator(
        local_user_hash: [u8; 16],
        local_client_id: u32,
        local_tcp_port: u16,
    ) -> Self {
        Self {
            state: Ed2kSessionState::Connecting,
            is_initiator: true,
            local_user_hash,
            local_client_id,
            local_tcp_port,
            decoder: Ed2kDecoder::new(),
        }
    }

    /// Creates a new session configured as a listener awaiting an incoming handshake.
    pub fn new_listener(
        local_user_hash: [u8; 16],
        local_client_id: u32,
        local_tcp_port: u16,
    ) -> Self {
        Self {
            state: Ed2kSessionState::AwaitingHello,
            is_initiator: false,
            local_user_hash,
            local_client_id,
            local_tcp_port,
            decoder: Ed2kDecoder::new(),
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

                let peer = Ed2kPeerInfo::from_hello(hello);
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
                self.state = Ed2kSessionState::Ready { peer: remote_peer };
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

                let peer = Ed2kPeerInfo::from_hello_answer(answer);
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

        match (packet.protocol, packet.opcode) {
            (PR_ED2K, OP_HELLO) => {
                let hello = self
                    .decoder
                    .decode_hello(packet.protocol, packet.opcode, &packet.payload, 0)?;
                let _peer = self.on_hello_received(&hello)?;
                Ok(Some(Ed2kMessage::Hello(hello)))
            }
            (PR_ED2K, OP_HELLO_ANSWER) => {
                let answer = self.decoder.decode_hello_answer(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                let _peer = self.on_hello_answer_received(&answer)?;
                Ok(Some(Ed2kMessage::HelloAnswer(answer)))
            }
            _ => {
                if !self.is_ready() {
                    let err = Ed2kSessionError::UnexpectedMessageInState {
                        state: self.state.name(),
                        opcode: packet.opcode,
                    };
                    self.fail(err.clone());
                    return Err(err);
                }
                // When in Ready state, other opcodes can be decoded or routed
                let msg = self.decoder.decode_raw_packet(packet)?;
                Ok(Some(msg))

            }
        }
    }

    /// Marks the session as timed out.
    pub fn on_timeout(&mut self, duration: Duration) {
        self.state = Ed2kSessionState::Failed {
            reason: Ed2kSessionError::Timeout(duration),
        };
    }

    /// Handles remote transport closure.
    pub fn on_remote_closed(&mut self) {
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
        self.state = Ed2kSessionState::Failed { reason };
    }

    /// Returns true if the session has successfully completed the handshake and is ready.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, Ed2kSessionState::Ready { .. })
    }

    /// Returns a reference to the negotiated remote peer info if in `Ready` state.
    pub fn peer_info(&self) -> Option<&Ed2kPeerInfo> {
        match &self.state {
            Ed2kSessionState::Ready { peer } => Some(peer),
            _ => None,
        }
    }
}
