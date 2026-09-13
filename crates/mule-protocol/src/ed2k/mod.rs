//! Passive eD2K protocol archetype: read-only decoders, bounds, and observation types.
//!
//! # Safety and Offline Policy
//! This module is strictly read-only and passive. It contains:
//! - No network sockets (`TcpStream`, `UdpSocket`).
//! - No public address listening or peer discovery.
//! - No packet encoders destined for transmission to real peers.
//! - No connected session state machines.

pub mod decoder;
pub mod encoder;
pub mod error;
pub mod limits;
pub mod opcodes;
pub mod packet;
pub mod protocol;
pub mod tags;

#[cfg(feature = "ed2k-loopback")]
pub mod peer_info;
#[cfg(feature = "ed2k-loopback")]
pub mod session;
#[cfg(feature = "ed2k-loopback")]
pub mod loopback;

pub use decoder::Ed2kDecoder;
pub use encoder::Ed2kEncoder;
pub use error::{Ed2kEncodeError, Ed2kErrorKind, Ed2kParseError};
pub use limits::Ed2kLimits;
pub use packet::{Ed2kFileStatus, Ed2kHello, Ed2kHelloAnswer, Ed2kMessage};
pub use protocol::{is_valid_ed2k_protocol, PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED};
pub use tags::{
    Ed2kTag, RawTag, TagName, TagReader, TagValue, TAG_SPECIAL_EMULE_VERSION,
    TAG_SPECIAL_FILESIZE, TAG_SPECIAL_FILETYPE, TAG_SPECIAL_MISCOPTIONS1,
    TAG_SPECIAL_MISCOPTIONS2, TAG_SPECIAL_NAME, TAG_SPECIAL_UDPOPTIONS, TAG_SPECIAL_VERSION,
};

#[cfg(feature = "ed2k-loopback")]
pub use peer_info::Ed2kPeerInfo;
#[cfg(feature = "ed2k-loopback")]
pub use session::{Ed2kSession, Ed2kSessionError, Ed2kSessionState};
#[cfg(feature = "ed2k-loopback")]
pub use loopback::{Ed2kLoopbackClient, Ed2kTestPeer, TestPeerMode};


