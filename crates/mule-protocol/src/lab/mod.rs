//! Laboratory loopback peer protocol for controlled, safe transfer validation.
//!
//! Note: This module is strictly separated from historical eD2K wire formats to prevent
//! premature claims of external network interoperation.

pub mod in_flight;
pub mod messages;
pub mod source_engine;
pub mod state_machine;

pub use in_flight::{InFlightError, InFlightTracker};
pub use messages::{LabMessage, LabRejectReason, LAB_OP_HELLO, LAB_OP_HELLO_ACK};
pub use source_engine::{HostedFile, LabSourceEngine};
pub use state_machine::{PeerAction, PeerFailure, PeerState, PeerStateMachine, ProtocolViolation};
