// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Pure deterministic state machine for laboratory peer-to-peer sessions.

use thiserror::Error;

use super::messages::{LabMessage, LAB_OP_HELLO, LAB_OP_HELLO_ACK};
use crate::framing::RawPacket;
use crate::traits::{ReceivedBlock, SourceStateUpdate};
use crate::types::{PeerId, PR_EMULE_COMPRESSED, PR_LAB};

/// Protocol violation errors triggering immediate session termination.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ProtocolViolation {
    /// Message arrived in an illegal state.
    #[error("Unexpected message opcode 0x{opcode:02X} in state {state}")]
    UnexpectedMessageInState {
        /// Current state name.
        state: &'static str,
        /// Message opcode received.
        opcode: u8,
    },

    /// Negotiated protocol version is incompatible.
    #[error("Incompatible protocol version: client={client}, server={server}")]
    IncompatibleVersion {
        /// Client version.
        client: u32,
        /// Server version.
        server: u32,
    },

    /// Nonce returned in HelloAck did not match the original challenge nonce.
    #[error("Handshake nonce mismatch")]
    NonceMismatch,

    /// Hello message was sent more than once.
    #[error("Double Hello rejected: session already initialized")]
    DoubleHello,

    /// Attempted block transfer before session was in Ready state.
    #[error("Transfer message received before Ready state")]
    PrematureTransferMessage,

    /// Payload structure was malformed.
    #[error("Malformed message payload: {0}")]
    Malformed(String),

    /// Compressed frame (0xD4) rejected pending bounded decompression stage.
    #[error("Compressed frame 0xD4 (PR_EMULE_COMPRESSED) is not supported prior to bounded decompression stage")]
    CompressedProtocolUnsupported,

    /// Unexpected or unsupported protocol byte.
    #[error("Unexpected protocol byte 0x{protocol:02X} (expected 0xEE PR_LAB)")]
    UnexpectedProtocol {
        /// Rejected protocol byte.
        protocol: u8,
    },
}

/// Cause of terminal session failure.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum PeerFailure {
    /// Protocol violation error.
    #[error(transparent)]
    Violation(#[from] ProtocolViolation),

    /// Handshake or idle timeout expired.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Socket I/O failure.
    #[error("I/O error: {0}")]
    Io(String),
}

/// Lifecycle states of a laboratory peer connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerState {
    /// Initial disconnected state.
    Disconnected,
    /// Actively establishing transport connection.
    Connecting,
    /// Listener waiting for initiator's Hello.
    AwaitingHello,
    /// Initiator waiting for receiver's HelloAck.
    AwaitingHelloAck {
        /// Local challenge nonce awaiting confirmation.
        local_nonce: [u8; 16],
    },
    /// Fully authenticated and negotiated session ready for data transfer.
    Ready {
        /// Remote peer identifier.
        remote_peer_id: PeerId,
        /// Mutually agreed lab protocol version.
        negotiated_version: u32,
    },
    /// Gracefully terminating connection.
    Closing,
    /// Session is cleanly terminated.
    Closed,
    /// Irreversible terminal failure state.
    Failed {
        /// Reason for failure.
        reason: PeerFailure,
    },
}

impl PeerState {
    /// Returns the human-readable name of the current state.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::AwaitingHello => "AwaitingHello",
            Self::AwaitingHelloAck { .. } => "AwaitingHelloAck",
            Self::Ready { .. } => "Ready",
            Self::Closing => "Closing",
            Self::Closed => "Closed",
            Self::Failed { .. } => "Failed",
        }
    }

    /// True if the session is ready for transfers.
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    /// True if the session is in a terminal state (Closed or Failed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Closed | Self::Failed { .. })
    }
}

/// Actions produced by the state machine to be executed by the transport layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerAction {
    /// Transmit a message to the remote peer.
    SendMessage(LabMessage),
    /// Forward a downloaded block to the storage sink.
    NotifySink(ReceivedBlock),
    /// Forward remote peer metadata to the download planner.
    NotifyPlanner(SourceStateUpdate),
    /// Close the physical network transport.
    CloseConnection,
}

/// State machine controller managing session progression.
pub struct PeerStateMachine {
    state: PeerState,
    local_peer_id: PeerId,
    supported_version: u32,
}

impl PeerStateMachine {
    /// Creates a new state machine for a listener waiting for incoming connections.
    pub fn new_listener(local_peer_id: PeerId, supported_version: u32) -> Self {
        Self {
            state: PeerState::AwaitingHello,
            local_peer_id,
            supported_version,
        }
    }

    /// Creates a new state machine for an initiator starting a connection.
    pub fn new_initiator(
        local_peer_id: PeerId,
        supported_version: u32,
        challenge_nonce: [u8; 16],
    ) -> (Self, LabMessage) {
        let sm = Self {
            state: PeerState::AwaitingHelloAck {
                local_nonce: challenge_nonce,
            },
            local_peer_id,
            supported_version,
        };
        let hello = LabMessage::Hello {
            client_version: supported_version,
            nonce: challenge_nonce,
            peer_id: local_peer_id,
        };
        (sm, hello)
    }

    /// Returns a reference to the current state.
    pub fn state(&self) -> &PeerState {
        &self.state
    }

    /// Forces the state machine into the Failed state with the given reason.
    pub fn fail(&mut self, failure: PeerFailure) {
        self.state = PeerState::Failed { reason: failure };
    }

    /// Pure transition step handling an incoming message.
    pub fn on_message(&mut self, msg: LabMessage) -> Result<Vec<PeerAction>, ProtocolViolation> {
        let (next_state, actions) = Self::step(
            self.state.clone(),
            msg,
            self.local_peer_id,
            self.supported_version,
        )?;
        self.state = next_state;
        Ok(actions)
    }

    /// Pure transition handling an incoming raw packet before decoding.
    ///
    /// Validates protocol byte (rejecting PR_EMULE_COMPRESSED or unexpected protocols)
    /// and decodes into a [`LabMessage`].
    pub fn on_raw_packet(
        &mut self,
        packet: &RawPacket,
    ) -> Result<Vec<PeerAction>, ProtocolViolation> {
        if packet.protocol == PR_EMULE_COMPRESSED {
            self.state = PeerState::Failed {
                reason: PeerFailure::Violation(ProtocolViolation::CompressedProtocolUnsupported),
            };
            return Err(ProtocolViolation::CompressedProtocolUnsupported);
        }

        if packet.protocol != PR_LAB {
            let violation = ProtocolViolation::UnexpectedProtocol {
                protocol: packet.protocol,
            };
            self.state = PeerState::Failed {
                reason: PeerFailure::Violation(violation.clone()),
            };
            return Err(violation);
        }

        let msg = match LabMessage::from_raw_packet(packet) {
            Ok(m) => m,
            Err(e) => {
                let violation = ProtocolViolation::Malformed(e.to_string());
                self.state = PeerState::Failed {
                    reason: PeerFailure::Violation(violation.clone()),
                };
                return Err(violation);
            }
        };

        self.on_message(msg)
    }

    /// Pure transition function testable without network or side-effects.
    pub fn step(
        state: PeerState,
        msg: LabMessage,
        local_peer_id: PeerId,
        supported_version: u32,
    ) -> Result<(PeerState, Vec<PeerAction>), ProtocolViolation> {
        // Universal Goodbye handler: can arrive in any non-terminal state.
        if let LabMessage::Goodbye { .. } = &msg {
            return match state {
                PeerState::Closed | PeerState::Failed { .. } => {
                    Err(ProtocolViolation::UnexpectedMessageInState {
                        state: state.name(),
                        opcode: 0x07,
                    })
                }
                _ => Ok((PeerState::Closed, vec![PeerAction::CloseConnection])),
            };
        }

        // Universal Premature transfer check: RequestBlock and BlockData are forbidden before Ready.
        if !state.is_ready()
            && matches!(
                msg,
                LabMessage::RequestBlock { .. } | LabMessage::BlockData { .. }
            )
        {
            return Err(ProtocolViolation::PrematureTransferMessage);
        }

        match state {
            PeerState::AwaitingHello => match msg {
                LabMessage::Hello {
                    client_version,
                    nonce,
                    peer_id,
                } => {
                    if client_version != supported_version {
                        return Err(ProtocolViolation::IncompatibleVersion {
                            client: client_version,
                            server: supported_version,
                        });
                    }
                    let ack = LabMessage::HelloAck {
                        server_version: supported_version,
                        nonce_ack: nonce,
                        peer_id: local_peer_id,
                    };
                    let next = PeerState::Ready {
                        remote_peer_id: peer_id,
                        negotiated_version: supported_version,
                    };
                    Ok((next, vec![PeerAction::SendMessage(ack)]))
                }
                _ => Err(ProtocolViolation::UnexpectedMessageInState {
                    state: "AwaitingHello",
                    opcode: LAB_OP_HELLO_ACK,
                }),
            },

            PeerState::AwaitingHelloAck { local_nonce } => match msg {
                LabMessage::HelloAck {
                    server_version,
                    nonce_ack,
                    peer_id,
                } => {
                    if server_version != supported_version {
                        return Err(ProtocolViolation::IncompatibleVersion {
                            client: supported_version,
                            server: server_version,
                        });
                    }
                    if nonce_ack != local_nonce {
                        return Err(ProtocolViolation::NonceMismatch);
                    }
                    let next = PeerState::Ready {
                        remote_peer_id: peer_id,
                        negotiated_version: supported_version,
                    };
                    Ok((next, vec![]))
                }
                _ => Err(ProtocolViolation::UnexpectedMessageInState {
                    state: "AwaitingHelloAck",
                    opcode: LAB_OP_HELLO,
                }),
            },

            PeerState::Ready {
                remote_peer_id,
                negotiated_version,
            } => match msg {
                LabMessage::Hello { .. } => Err(ProtocolViolation::DoubleHello),
                LabMessage::HelloAck { .. } => Err(ProtocolViolation::UnexpectedMessageInState {
                    state: "Ready",
                    opcode: LAB_OP_HELLO_ACK,
                }),
                LabMessage::FileOffer {
                    file_hash,
                    file_size: _,
                    name: _,
                } => {
                    let update = SourceStateUpdate {
                        peer_id: remote_peer_id,
                        file_hash,
                        available_parts: vec![true],
                        queue_rank: None,
                    };
                    let next = PeerState::Ready {
                        remote_peer_id,
                        negotiated_version,
                    };
                    Ok((next, vec![PeerAction::NotifyPlanner(update)]))
                }
                LabMessage::RequestBlock { .. } => {
                    // RequestBlock is handled by source engine
                    let next = PeerState::Ready {
                        remote_peer_id,
                        negotiated_version,
                    };
                    Ok((next, vec![]))
                }
                LabMessage::BlockData {
                    file_hash,
                    start_offset,
                    data,
                } => {
                    let block = ReceivedBlock::new(file_hash, start_offset, data);
                    let next = PeerState::Ready {
                        remote_peer_id,
                        negotiated_version,
                    };
                    Ok((next, vec![PeerAction::NotifySink(block)]))
                }
                LabMessage::BlockRejected { .. } => {
                    let next = PeerState::Ready {
                        remote_peer_id,
                        negotiated_version,
                    };
                    Ok((next, vec![]))
                }
                LabMessage::ProtocolError { .. } => {
                    let next = PeerState::Closed;
                    Ok((next, vec![PeerAction::CloseConnection]))
                }
                LabMessage::Goodbye { .. } => unreachable!(),
            },

            other => match msg {
                LabMessage::RequestBlock { .. } | LabMessage::BlockData { .. } => {
                    Err(ProtocolViolation::PrematureTransferMessage)
                }
                _ => Err(ProtocolViolation::UnexpectedMessageInState {
                    state: other.name(),
                    opcode: 0,
                }),
            },
        }
    }
}
