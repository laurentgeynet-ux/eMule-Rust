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
pub mod peer_info;
pub mod profile;
pub mod protocol;
pub mod sequencer;
pub mod session;
pub mod tags;

#[cfg(feature = "ed2k-loopback")]
pub mod loopback;

#[cfg(feature = "ed2k-reference-loopback")]
pub mod reference;

pub use decoder::Ed2kDecoder;
pub use encoder::Ed2kEncoder;
pub use error::{Ed2kEncodeError, Ed2kErrorKind, Ed2kParseError};
pub use limits::Ed2kLimits;
pub use opcodes::{
    OP_EMULEINFO, OP_EMULE_INFO, OP_FILEREQANSNOFILE, OP_FILESTATUS, OP_FILESTATUSREQ, OP_HELLO,
    OP_HELLOANSWER, OP_HELLO_ANSWER, OP_REQUESTPARTS, OP_REQUESTPARTS_EMULE, OP_SENDINGPART,
    OP_SETREQFILEID, OP_SETREQFILEID_EMULE,
};
pub use packet::{
    expected_mask_bytes, Ed2kBlockRange, Ed2kEmuleInfo, Ed2kFileReqAnsNoFile, Ed2kFileStatus,
    Ed2kFileStatusReq, Ed2kHello, Ed2kHelloAnswer, Ed2kMessage, Ed2kRequestParts,
    Ed2kSendingPart, PartAvailabilityMask,
};
pub use peer_info::Ed2kPeerInfo;
pub use profile::Ed2kCompatibilityProfile;
pub use protocol::{is_valid_ed2k_protocol, PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED};
pub use sequencer::{
    Ed2kDownloadSequencer, Ed2kSequencerError, ED2K_BLOCK_SIZE, ED2K_PART_SIZE,
    MAX_REQUEST_RANGES_PER_PACKET,
};
pub use session::{
    BlockRequestKey, BlockRequestTracker, Ed2kSession, Ed2kSessionError, Ed2kSessionState,
    FileStatusRequest, FileStatusTracker, PendingBlockRequest, SessionSnapshot,
    MAX_IN_FLIGHT_BLOCK_REQUESTS,
};
pub use tags::{
    Ed2kTag, RawTag, TagName, TagReader, TagValue, TAG_SPECIAL_EMULE_VERSION,
    TAG_SPECIAL_FILESIZE, TAG_SPECIAL_FILETYPE, TAG_SPECIAL_MISCOPTIONS1,
    TAG_SPECIAL_MISCOPTIONS2, TAG_SPECIAL_NAME, TAG_SPECIAL_UDPOPTIONS, TAG_SPECIAL_VERSION,
};

#[cfg(feature = "ed2k-loopback")]
pub use loopback::{Ed2kLoopbackClient, Ed2kTestPeer, TestPeerMode};

#[cfg(feature = "ed2k-reference-loopback")]
pub use reference::{
    is_bidi_or_formatting_control, opcode_to_str, sanitize_unicode_for_display, DivergenceReport,
    EmulatedReferencePeer, ReferenceCompatibleTestPeer, ReferenceInteropClient,
    ReferencePeerConfig, TraceDirection, TraceRecord,
};


