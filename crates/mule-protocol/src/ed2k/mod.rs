//! Passive eD2K protocol archetype: read-only decoders, bounds, and observation types.
//!
//! # Safety and Offline Policy
//! This module is strictly read-only and passive. It contains:
//! - No network sockets (`TcpStream`, `UdpSocket`).
//! - No public address listening or peer discovery.
//! - No packet encoders destined for transmission to real peers.
//! - No connected session state machines.

pub mod decoder;
pub mod error;
pub mod limits;
pub mod opcodes;
pub mod packet;
pub mod protocol;
pub mod tags;

pub use decoder::Ed2kDecoder;
pub use error::{Ed2kErrorKind, Ed2kParseError};
pub use limits::Ed2kLimits;
pub use packet::{Ed2kFileStatus, Ed2kHello, Ed2kHelloAnswer, Ed2kMessage};
pub use protocol::{is_valid_ed2k_protocol, PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED};
pub use tags::{Ed2kTag, RawTag, TagName, TagReader, TagValue};
