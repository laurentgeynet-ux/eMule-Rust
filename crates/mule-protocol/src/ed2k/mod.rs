// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Passive eD2K protocol archetype: read-only decoders, bounds, and observation types.
//!
//! # Safety and Offline Policy
//! This module is strictly read-only and passive. It contains:
//! - No network sockets (`TcpStream`, `UdpSocket`).
//! - No public address listening or peer discovery.
//! - No packet encoders destined for transmission to real peers.
//! - No connected session state machines.

pub mod client_udp;
pub mod decoder;
pub mod encoder;
pub mod error;
pub mod file_identifier;
pub mod limits;
pub mod obfuscation;
pub mod opcodes;
pub mod packet;
pub mod peer_info;
pub mod profile;
pub mod protocol;
pub mod sequencer;
pub mod server;
pub mod server_udp;
pub mod session;
pub mod source_exchange;
#[cfg(feature = "ed2k-loopback")]
pub mod storage;
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
    OP_ACCEPTUPLOADREQ, OP_AICHFILEHASHREQ, OP_AICHFILEHASHRES, OP_AICH_ANSWER, OP_AICH_REQUEST,
    OP_ANSWERSOURCES, OP_ANSWERSOURCES2, OP_BUDDYPING, OP_BUDDYPONG, OP_CALLBACK,
    OP_CALLBACKREQUEST, OP_CALLBACKREQUESTED, OP_CALLBACK_FAIL, OP_CANCELTRANSFER,
    OP_COMPRESSEDPART, OP_COMPRESSEDPART_I64, OP_EMULEINFO, OP_EMULEINFOANSWER, OP_EMULE_INFO,
    OP_EMULE_INFO_ANSWER, OP_END_OF_DOWNLOAD, OP_FILEREQANSNOFILE, OP_FILESTATUS, OP_FILESTATUSREQ,
    OP_FOUNDSOURCES, OP_FOUNDSOURCES_OBFU, OP_FWCHECKUDPREQ, OP_GETSOURCES, OP_GETSOURCES_OBFU,
    OP_HASHSETANSWER, OP_HASHSETREQUEST, OP_HELLO, OP_HELLOANSWER, OP_HELLO_ANSWER, OP_IDCHANGE,
    OP_KAD_FWTCPCHECK_ACK, OP_LOGINREQUEST, OP_OFFERFILES, OP_OUTOFPARTS, OP_PUBLICKEY,
    OP_QUEUERANKING, OP_REASKCALLBACKTCP, OP_REASKCALLBACKUDP, OP_REQFILENAME,
    OP_REQFILENAME_ANSWER, OP_REQUESTPARTS, OP_REQUESTPARTS_EMULE, OP_REQUESTPARTS_I64,
    OP_REQUESTSOURCES, OP_REQUESTSOURCES2, OP_SEARCHREQUEST, OP_SEARCH_RESULT, OP_SECIDENTSTATE,
    OP_SENDINGPART, OP_SENDINGPART_I64, OP_SERVERIDENT, OP_SERVERLIST, OP_SERVERMESSAGE,
    OP_SERVERSTATUS, OP_SETREQFILEID, OP_SETREQFILEID_EMULE, OP_SIGNATURE, OP_STARTUPLOADREQ,
    OP_CHANGE_SLOT, OP_CHATCAPTCHAREQ, OP_CHATCAPTCHARES, OP_FILEDESC, OP_HASHSETANSWER2,
    OP_HASHSETREQUEST2, OP_MESSAGE, OP_MULTIPACKET, OP_MULTIPACKETANSWER,
    OP_MULTIPACKETANSWER_EXT2, OP_MULTIPACKET_EXT, OP_MULTIPACKET_EXT2, OP_PEERCACHE_ACK,
    OP_PEERCACHE_ANSWER, OP_PEERCACHE_QUERY, OP_PORTTEST, OP_QUEUERANK,
};
pub use file_identifier::{
    Ed2kFileIdentifier, AICH_HASH_LEN, FILE_IDENTIFIER_OPT_AICH, FILE_IDENTIFIER_OPT_MD4,
    FILE_IDENTIFIER_OPT_SIZE, FILE_IDENTIFIER_OPTIONAL_MASK, FILE_IDENTIFIER_REQUIRED_MASK,
    MD4_HASH_LEN,
};
pub use packet::{
    expected_mask_bytes, Ed2kAichAnswer, Ed2kAichFileHashReq, Ed2kAichFileHashRes,
    Ed2kAichRecovery, Ed2kAichRequest, Ed2kBlockRange, Ed2kBlockRange64, Ed2kCompressedPartI64,
    Ed2kEmuleInfo, Ed2kFileReqAnsNoFile, Ed2kFileStatus, Ed2kFileStatusReq, Ed2kHashSetAnswer,
    Ed2kHashSetReq, Ed2kHello, Ed2kHelloAnswer, Ed2kMessage, Ed2kPublicKey, Ed2kRequestParts,
    Ed2kRequestPartsI64, Ed2kSecIdentState, Ed2kSendingPart, Ed2kSendingPartI64, Ed2kSignature,
    Ed2kAichPartHashSet, Ed2kCaptchaRequest, Ed2kCaptchaResponse, Ed2kChangeSlot, Ed2kChatMessage,
    Ed2kFileDesc, Ed2kHashSetAnswer2, Ed2kHashSetReq2, Ed2kMultiPacketAnswer,
    Ed2kMultiPacketFileRef, Ed2kMultiPacketRequest, Ed2kMultiPacketSubAnswer,
    Ed2kMultiPacketSubRequest, Ed2kPortTest, Ed2kQueueRank, Ed2kQueueRanking, Ed2kRequestExtInfo,
    PartAvailabilityMask,
};
pub use peer_info::Ed2kPeerInfo;
pub use profile::Ed2kCompatibilityProfile;
pub use protocol::{is_valid_ed2k_protocol, PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED};
pub use sequencer::{
    Ed2kBlockSink, Ed2kDownloadSequencer, Ed2kSequencerError, InFlightBlock, ED2K_BLOCK_SIZE,
    ED2K_PART_SIZE, MAX_REQUEST_RANGES_PER_PACKET,
};
pub use server::{
    decode_server_response, ed2k_file_type_term, CallbackRequest, CallbackRequested,
    FoundSourcePeer, FoundSourcePeerObfu, FoundSources, FoundSourcesObfu, GetSourcesRequest,
    OfferFiles, OfferedFile, SearchQuery, SearchResult, SearchResultFile, ServerIdChange,
    ServerIdent, ServerLoginRequest, ServerMessage, ServerResponse, ServerStatus,
    SRV_TCPFLG_COMPRESSION, SRV_TCPFLG_LARGEFILES,
};
pub use session::{
    BlockRequestKey, BlockRequestTracker, Ed2kSession, Ed2kSessionError, Ed2kSessionState,
    FileStatusRequest, FileStatusTracker, PendingBlockRequest, SessionSnapshot,
    MAX_IN_FLIGHT_BLOCK_REQUESTS,
};
pub use source_exchange::{
    decode_answer_sources, decode_request_sources, encode_answer_sources, encode_request_sources,
    Ed2kAnswerSources, Ed2kRequestSources, SxSource, MAX_SX_SOURCES_PER_PACKET,
    SOURCE_EXCHANGE2_VERSION,
};
#[cfg(feature = "ed2k-loopback")]
pub use storage::{Ed2kBlockWriter, Ed2kPartDownloadStatus, Ed2kPartFileSink, Ed2kPartResumeState};
pub use tags::{
    Ed2kTag, RawTag, TagName, TagReader, TagValue, TAG_SPECIAL_EMULE_VERSION, TAG_SPECIAL_FILESIZE,
    TAG_SPECIAL_FILETYPE, TAG_SPECIAL_MISCOPTIONS1, TAG_SPECIAL_MISCOPTIONS2, TAG_SPECIAL_NAME,
    TAG_SPECIAL_UDPOPTIONS, TAG_SPECIAL_VERSION,
};

#[cfg(feature = "ed2k-loopback")]
pub use loopback::{Ed2kLoopbackClient, Ed2kTestPeer, TestPeerMode};

#[cfg(feature = "ed2k-reference-loopback")]
pub use reference::{
    is_bidi_or_formatting_control, opcode_to_str, sanitize_unicode_for_display, DivergenceReport,
    EmulatedReferencePeer, ReferenceCompatibleTestPeer, ReferenceInteropClient,
    ReferencePeerConfig, TraceDirection, TraceRecord,
};
