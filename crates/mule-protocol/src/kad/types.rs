// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Wire types and constants for Kad UDP packets.

use crate::ed2k::tags::{Ed2kTag, RawTag, TagName, TagValue};

/// 128-bit Kad ID alias (16 bytes).
pub type KadIdBytes = [u8; 16];

/// Maximum contacts in a Kad2 Bootstrap or Find Node response (standard K = 20, bounded to 50).
pub const MAX_KAD_RES_CONTACTS: usize = 50;

/// Maximum sources returned in a Kad2 Search Source response
/// (eMule `iMaxResults = 300`, `Indexed.cpp:707`).
pub const MAX_KAD_SEARCH_SOURCES: usize = 300;

/// eMule `UDP_KAD_MAXFRAGMENT` (`Opcodes.h:134`) — maximum Kad UDP datagram
/// size before eMule splits a multi-entry answer into several packets
/// (conservative 1500-MTU bound leaving room for IP/UDP/tunnel/Kad headers).
pub const UDP_KAD_MAXFRAGMENT: usize = 1420;

/// Wire representation of a Kad contact in responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadContactWire {
    /// 128-bit Kad ID.
    pub kad_id: KadIdBytes,
    /// IPv4 address as u32 in network order.
    pub ip: [u8; 4],
    /// UDP port.
    pub udp_port: u16,
    /// TCP port (optional, 0 if not set).
    pub tcp_port: u16,
    /// Kad version.
    pub version: u8,
}

/// Kad2 Ping packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadPing {
    /// Sender/receiver Kad ID.
    pub kad_id: KadIdBytes,
    /// UDP port.
    pub udp_port: u16,
}

/// Kad2 Pong packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadPong {
    /// Port seen by remote peer.
    pub client_port: u16,
}

/// Kad2 Hello Request packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadHelloReq {
    /// Sender Kad ID.
    pub kad_id: KadIdBytes,
    /// Sender UDP port announced via `TAG_SOURCEUPORT` (0 = use packet source).
    pub udp_port: u16,
    /// Kad version.
    pub version: u8,
    /// Sender TCP port (0 if none).
    pub tcp_port: u16,
    /// `TAG_KADMISCOPTIONS` byte if present (Kad >= 8): bit0 UDP firewalled,
    /// bit1 TCP firewalled, bit2 = requesting `KADEMLIA2_HELLO_RES_ACK`.
    pub misc_options: Option<u8>,
}

impl KadHelloReq {
    /// True if the sender asked us to answer with `KADEMLIA2_HELLO_RES_ACK`.
    pub fn requests_ack(&self) -> bool {
        self.misc_options.is_some_and(|o| o & 0x04 != 0)
    }
}

/// Kad2 Hello Response packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadHelloRes {
    /// Responder Kad ID.
    pub kad_id: KadIdBytes,
    /// Responder UDP port announced via `TAG_SOURCEUPORT` (0 = packet source).
    pub udp_port: u16,
    /// Kad version.
    pub version: u8,
    /// Responder TCP port (0 if none).
    pub tcp_port: u16,
    /// `TAG_KADMISCOPTIONS` byte if present (Kad >= 8).
    pub misc_options: Option<u8>,
}

impl KadHelloRes {
    /// True if the responder asked us to send `KADEMLIA2_HELLO_RES_ACK`.
    pub fn requests_ack(&self) -> bool {
        self.misc_options.is_some_and(|o| o & 0x04 != 0)
    }
}

/// Kad2 Hello response acknowledgement (`KADEMLIA2_HELLO_RES_ACK`, 0x22).
///
/// Sent after a `KADEMLIA2_HELLO_RES` whose `TAG_KADMISCOPTIONS` requests it —
/// completes the three-way handshake proving our IP is not spoofed
/// (eMule `CKademliaUDPListener::Process_KADEMLIA2_HELLO_RES_ACK`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadHelloResAck {
    /// Sender Kad ID.
    pub kad_id: KadIdBytes,
}

/// Legacy Kad TCP firewall check request (`KADEMLIA_FIREWALLED_REQ`, 0x50):
/// the sender asks us to TCP-connect to `tcp_port` to test their reachability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadFirewalledReq {
    /// TCP port the sender wants probed.
    pub tcp_port: u16,
}

/// Kad TCP firewall check request v2 (`KADEMLIA_FIREWALLED2_REQ`, 0x53):
/// `[tcp_port u16][user_hash 16][connect_options u8]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadFirewalledReq2 {
    /// TCP port the sender wants probed.
    pub tcp_port: u16,
    /// Sender's eD2K user hash.
    pub user_hash: [u8; 16],
    /// Sender's connect options byte (`GetMyConnectOptions`).
    pub connect_options: u8,
}

/// Kad firewall check response (`KADEMLIA_FIREWALLED_RES`, 0x58): echoes the
/// requester's IPv4 address as seen by the responder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadFirewalledRes {
    /// Requester's IPv4 address (little-endian on the wire).
    pub ip: u32,
}

/// Kad2 Bootstrap Request packet (0x21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadBootstrapReq {
    /// Sender Kad ID.
    pub kad_id: KadIdBytes,
    /// Sender UDP port.
    pub udp_port: u16,
}

/// Kad2 Bootstrap Response / Find Node Response packet (0x29).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadBootstrapRes {
    /// Responder Kad ID.
    pub kad_id: KadIdBytes,
    /// Responder UDP port.
    pub udp_port: u16,
    /// Kad version.
    pub version: u8,
    /// List of contacts returned (bounded to MAX_KAD_RES_CONTACTS).
    pub contacts: Vec<KadContactWire>,
}

/// Kad2 Request packet (Find Node / Search, 0x21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadReq {
    /// Request type (e.g. 0 for FIND_NODE).
    pub req_type: u8,
    /// Target Kad ID sought.
    pub target_id: KadIdBytes,
    /// Sender Kad ID.
    pub sender_id: KadIdBytes,
}

/// Wire representation of a source in a Kad2 Search Source response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadSourceWire {
    /// 128-bit Kad/Client ID.
    pub kad_id: KadIdBytes,
    /// IPv4 address as bytes in network order.
    pub ip: [u8; 4],
    /// UDP port.
    pub udp_port: u16,
    /// TCP port (0 if none — Kad-LowID source reachable only via buddy).
    pub tcp_port: u16,
    /// Encryption/capability flags.
    pub encryption: u8,
    /// Buddy Kad ID advertised via `TAG_BUDDYHASH`/`FT_BUDDYHASH` (0xF8) —
    /// present on LowID sources that can only be reached through a Kad
    /// buddy callback (`KADEMLIA_CALLBACK_REQ`).
    pub buddy_id: Option<KadIdBytes>,
}

/// Kad2 Search Source Request (0x34).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadSearchSourceReq {
    /// Target file hash (16 bytes).
    pub target_file_hash: KadIdBytes,
    /// Sender Kad ID (16 bytes).
    pub sender_id: KadIdBytes,
    /// Start position for pagination.
    pub start_pos: u16,
    /// File size in bytes.
    pub file_size: u64,
}

/// Kad2 Search Source Response (0x3B, unified search response).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadSearchSourceRes {
    /// Target file hash (16 bytes).
    pub target_file_hash: KadIdBytes,
    /// Discovered sources (bounded to MAX_KAD_SEARCH_SOURCES).
    pub sources: Vec<KadSourceWire>,
}

/// Kad2 Publish Source Request (0x44).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadPublishSourceReq {
    /// Target file hash (16 bytes).
    pub target_file_hash: KadIdBytes,
    /// Contact announcing the file.
    pub contact: KadContactWire,
    /// File size in bytes.
    pub file_size: u64,
}

/// Kad2 Publish Source Response (0x4B, unified publish response).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadPublishSourceRes {
    /// Target file hash (16 bytes).
    pub target_file_hash: KadIdBytes,
    /// Result status code (0 = success).
    pub status: u8,
    /// Trailing options byte bit 0 (`KademliaUDPListener.cpp:1438`) — the
    /// indexer requests a `KADEMLIA2_PUBLISH_RES_ACK` datagram in return.
    /// "For future use": no released client sets it today.
    pub request_ack: bool,
}

/// Kad version that introduced AICH root hashes in keyword storage —
/// eMule `KADEMLIA_VERSION9_50a` (`Opcodes.h`).
pub const KADEMLIA_VERSION_AICH: u8 = 9;

/// Current Kad protocol version — eMule `KADEMLIA_VERSION` (`Opcodes.h`),
/// advertised by eMule 0.72a.
pub const KADEMLIA_VERSION_CURRENT: u8 = 10;

/// `TAG_KADAICHHASHPUB` (`\x36`) — BSOB carrying the publisher's 20-byte
/// AICH root hash inside a `PUBLISH_KEY_REQ` keyword entry. Introduced by
/// Kad v9 (`KADEMLIA_VERSION9_50a`, eMule `Opcodes.h`).
pub const TAG_KADAICHHASH_PUB: u8 = 0x36;

/// `TAG_KADAICHHASHRESULT` (`\x37`) — BSOB carrying the AICH popularity
/// histogram synthesized by the storing node inside `SEARCH_RES` keyword
/// results (eMule `CKeyEntry::WriteTagListWithPublishInfo`).
pub const TAG_KADAICHHASH_RESULT: u8 = 0x37;

/// `TAG_PUBLISHINFO` (`\x33`) — u32 `<names u8><publishers u8><trust×100 u16>`
/// appended by the storing node to keyword search results.
pub const TAG_PUBLISHINFO: u8 = 0x33;

/// 20-byte AICH root hash (SHA-1) as published via `TAG_KADAICHHASHPUB`.
pub const AICH_ROOT_HASH_SIZE: usize = 20;

/// Maximum byte size of the `TAG_KADAICHHASHRESULT` BSOB written by eMule
/// (`CKeyEntry::WriteTagListWithPublishInfo` writes into a 255-byte buffer
/// and keeps one byte for the count prefix).
pub const MAX_AICH_RESULT_BSOB: usize = 255;

/// Maximum AICH hashes written into one `TAG_KADAICHHASHRESULT` BSOB —
/// eMule stops counting when the next `<u8 pop><20B hash>` pair would push
/// the payload past 250 bytes (`WriteTagListWithPublishInfo`), yielding at
/// most 11 entries (`1 + 21*11 = 232`).
pub const MAX_AICH_RESULT_VOTES: usize = 11;

/// One AICH root hash and its publisher popularity inside
/// `TAG_KADAICHHASHRESULT` (`<u8 count><u8 pop><20B hash>…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AichHashVote {
    /// Number of distinct publishers claiming this root hash.
    pub popularity: u8,
    /// 20-byte AICH root hash.
    pub root: [u8; AICH_ROOT_HASH_SIZE],
}

/// Wire representation of a keyword search entry.
///
/// Not `Eq` because `tags` may contain `Float32` values.
#[derive(Debug, Clone, PartialEq)]
pub struct KadKeywordEntryWire {
    /// 128-bit file hash.
    pub file_hash: KadIdBytes,
    /// File name (UTF-8).
    pub file_name: String,
    /// File size in bytes.
    pub file_size: u64,
    /// Number of sources advertising this file.
    pub source_count: u32,
    /// Published metadata tag list retained verbatim from `PUBLISH_KEY_REQ`
    /// or the `SEARCH_RES` tag list (eMule `CKeyEntry::m_listTag`). Used to
    /// evaluate restrictive search expressions
    /// (`CKeyEntry::SearchTermsMatch`). Empty for entries decoded from the
    /// legacy flat format.
    pub tags: Vec<Ed2kTag>,
    /// `TAG_KADAICHHASHPUB` (0x36) decoded from a `PUBLISH_KEY_REQ` entry —
    /// the AICH root hash the publisher claims for this file (Kad v9+).
    /// Never set on entries received via `SEARCH_RES` (eMule ignores/strips
    /// the pub tag in results); the encoder emits it only for our own
    /// publish requests.
    pub aich_pub: Option<[u8; AICH_ROOT_HASH_SIZE]>,
    /// `TAG_KADAICHHASHRESULT` (0x37) histogram decoded from a `SEARCH_RES`
    /// entry, or synthesized by the storage node when serving results.
    /// Entries with `popularity == 0` are filtered on decode (eMule
    /// `ProcessResultKeyword`).
    pub aich_votes: Vec<AichHashVote>,
    /// `TAG_PUBLISHINFO` (0x33) raw u32 decoded from a `SEARCH_RES` entry,
    /// or synthesized by the storage node when serving results.
    pub publish_info: Option<u32>,
}

/// Kad2 Search Keyword Request (0x33).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadSearchKeywordReq {
    /// 128-bit MD4 hash of the keyword.
    pub keyword_hash: KadIdBytes,
    /// `start_pos & 0x8000` on the wire — the requester appended a
    /// restrictive search-expression tree (`CreateSearchExpressionTree`).
    pub restrictive: bool,
    /// Start position for pagination (`start_pos & 0x7FFF` on the wire).
    pub start_pos: u16,
    /// Raw search-expression bytes trailing the request when `restrictive`
    /// is set. Bounded by the frame limit. Parsed via
    /// `crate::kad::search_expr::KadSearchExpression` and applied by the
    /// daemon before pagination (eMule `StartSearchTermsMatch`).
    pub search_expression: Option<Vec<u8>>,
}

/// Kad2 Search Keyword Response (0x3B, unified search response, or legacy 0x2C).
///
/// Not Eq: entries carry metadata tags which may be floats.
#[derive(Debug, Clone, PartialEq)]
pub struct KadSearchKeywordRes {
    /// 128-bit MD4 hash of the keyword.
    pub keyword_hash: KadIdBytes,
    /// Discovered file entries.
    pub entries: Vec<KadKeywordEntryWire>,
}

/// Kad2 Publish Keyword Request (0x43).
///
/// Not Eq: entries carry metadata tags which may be floats.
#[derive(Debug, Clone, PartialEq)]
pub struct KadPublishKeywordReq {
    /// 128-bit MD4 hash of the keyword.
    pub keyword_hash: KadIdBytes,
    /// File entries published for this keyword.
    pub entries: Vec<KadKeywordEntryWire>,
}

/// Kad2 Firewall UDP test packet (0x62).
/// Sent by a Kad contact in response to a TCP `OP_FWCHECKUDPREQ`, to probe
/// whether the requester's Kad UDP port is reachable (eMule
/// `Process_KADEMLIA2_FIREWALLUDP` / `ProcessFirewallCheckUDPRequest`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadFirewallUdp {
    /// Error/already-known flag (0 = clean result, non-zero = remote reported
    /// that the result may be biased and the test should be discarded).
    pub error_code: u8,
    /// UDP port that was probed (intern or extern Kad port of the requester).
    pub udp_port: u16,
}

/// Kad2 Publish Keyword Response (0x4B, unified publish response).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadPublishKeywordRes {
    /// 128-bit MD4 hash of the keyword.
    pub keyword_hash: KadIdBytes,
    /// Status code (0 = success).
    pub status: u8,
}

/// Kad buddy search request (0x51) — a firewalled client asks us to act as
/// its buddy (`Process_KADEMLIA_FINDBUDDY_REQ`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadFindBuddyReq {
    /// Buddy ID the requester wants us to register under (its contact target).
    pub buddy_id: KadIdBytes,
    /// Requester's Kad client ID.
    pub user_id: KadIdBytes,
    /// Requester's TCP listening port (where the buddy link will come from).
    pub tcp_port: u16,
}

/// Kad buddy search response (0x5A) — acceptance of a buddy request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadFindBuddyRes {
    /// Echoed buddy ID.
    pub buddy_id: KadIdBytes,
    /// Responder's Kad client ID.
    pub client_id: KadIdBytes,
    /// Responder's TCP listening port (buddy connects there).
    pub tcp_port: u16,
    /// Connection option flags (post-0.49a senders; 0 when absent).
    pub connect_options: u8,
}

/// Kad buddy callback relay request (0x52) — a HighID requester asks us to
/// relay a callback to our buddy client (`Process_KADEMLIA_CALLBACK_REQ`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadCallbackReq {
    /// Buddy ID check field (forwarded verbatim to the buddy client).
    pub buddy_id: KadIdBytes,
    /// File hash the requester wants from the buddy client.
    pub file_id: KadIdBytes,
    /// Requester's TCP port where the buddy should connect back.
    pub tcp_port: u16,
}

// ---------------------------------------------------------------------------
// Kad notes (file comments / ratings) — eMule `CEntry` with `m_bSource=false`.
//
// Notes are third-party-supplied, display-only data: eMule explicitly treats
// note tag strings as the one place where non-UTF8/codepage content is allowed
// "for viewing only". We therefore keep the full tag list verbatim (typed
// `Ed2kTag` + `RawTag` for unknown types) and expose convenience accessors
// instead of a closed struct that would drop unknown metadata.
// ---------------------------------------------------------------------------

/// `TAG_FILENAME` (`\x01`) — file name tag inside a note entry.
pub const TAG_NOTE_FILENAME: u8 = 0x01;
/// `TAG_FILESIZE` (`\x02`) — file size tag inside a note entry.
pub const TAG_NOTE_FILESIZE: u8 = 0x02;
/// `TAG_DESCRIPTION` (`\x0B`) — user comment text of a note entry.
pub const TAG_NOTE_DESCRIPTION: u8 = 0x0B;
/// `TAG_FILERATING` (`\xF7`) — 0..5 user rating of a note entry.
pub const TAG_NOTE_FILERATING: u8 = 0xF7;

/// Maximum tag count accepted in a notes entry/request (anti-abuse bound).
pub const MAX_NOTE_TAG_COUNT: usize = 32;

/// Maximum note entries accepted in one search response
/// (eMule `SendValidNoteResult` caps at 150 per packet).
pub const MAX_NOTE_SEARCH_RESULTS: usize = 150;

/// A single note entry (`CEntry`) as carried by `KADEMLIA2_SEARCH_RES` (0x3B)
/// note results and `KADEMLIA2_PUBLISH_NOTES_REQ` (0x45).
#[derive(Debug, Clone, PartialEq)]
pub struct KadNoteWire {
    /// Publisher's Kad ID (`m_uSourceID`) — the "answer hash" in search
    /// results, the `source` field in publish requests.
    pub source_id: KadIdBytes,
    /// Typed tags (filename, filesize, description, rating, …).
    pub tags: Vec<Ed2kTag>,
    /// Unrecognized tags preserved raw (no data loss).
    pub unknown_tags: Vec<RawTag>,
}

impl KadNoteWire {
    fn note_tag(&self, id: u8) -> Option<&TagValue> {
        self.tags.iter().find_map(|t| {
            if t.name == TagName::Id(id) {
                Some(&t.value)
            } else {
                None
            }
        })
    }

    /// `TAG_FILENAME` string, if present.
    pub fn file_name(&self) -> Option<&str> {
        match self.note_tag(TAG_NOTE_FILENAME) {
            Some(TagValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// `TAG_FILESIZE` value, if present.
    pub fn file_size(&self) -> Option<u64> {
        match self.note_tag(TAG_NOTE_FILESIZE) {
            Some(TagValue::UInt8(v)) => Some(*v as u64),
            Some(TagValue::UInt16(v)) => Some(*v as u64),
            Some(TagValue::UInt32(v)) => Some(*v as u64),
            Some(TagValue::UInt64(v)) => Some(*v),
            _ => None,
        }
    }

    /// `TAG_DESCRIPTION` — the user comment. Untrusted display-only text.
    pub fn comment(&self) -> Option<&str> {
        match self.note_tag(TAG_NOTE_DESCRIPTION) {
            Some(TagValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// `TAG_FILERATING` — 0..5 rating, if present.
    pub fn rating(&self) -> Option<u8> {
        match self.note_tag(TAG_NOTE_FILERATING) {
            Some(TagValue::UInt8(v)) => Some(*v),
            Some(TagValue::UInt16(v)) => Some(*v as u8),
            Some(TagValue::UInt32(v)) => Some(*v as u8),
            _ => None,
        }
    }
}

/// Kad2 Search Notes request (`KADEMLIA2_SEARCH_NOTES_REQ`, 0x35):
/// `[target_file_hash:16][file_size:u64]` — eMule
/// `Process_KADEMLIA2_SEARCH_NOTES_REQ`.
#[derive(Debug, Clone, PartialEq)]
pub struct KadSearchNotesReq {
    /// Target file hash (16 bytes).
    pub target_file_hash: KadIdBytes,
    /// File size used to filter note entries (0 = accept any).
    pub file_size: u64,
}

/// Notes search response. On the wire this is the unified
/// `KADEMLIA2_SEARCH_RES` (0x3B) — `[sender_id:16][target:16][count:u16]
/// ([source_id:16][tag_count:u8][tags])*` — see eMule
/// `CIndexed::SendValidNoteResult`. The legacy Kad1 opcode
/// `KADEMLIA_SEARCH_NOTES_RES` (0x3A) carries the same entries without the
/// sender ID.
#[derive(Debug, Clone, PartialEq)]
pub struct KadSearchNotesRes {
    /// Responder Kad ID (zero for the legacy 0x3A form).
    pub sender_id: KadIdBytes,
    /// Target file hash (16 bytes).
    pub target_file_hash: KadIdBytes,
    /// Note entries (bounded).
    pub notes: Vec<KadNoteWire>,
}

/// Kad2 Publish Notes request (`KADEMLIA2_PUBLISH_NOTES_REQ`, 0x45):
/// `[target_file_hash:16][source_id:16][tag_count:u8][tags]` — eMule
/// `Process_KADEMLIA2_PUBLISH_NOTES_REQ`.
#[derive(Debug, Clone, PartialEq)]
pub struct KadPublishNotesReq {
    /// Target file hash (16 bytes).
    pub target_file_hash: KadIdBytes,
    /// Publisher's Kad ID (`m_uSourceID`).
    pub source_id: KadIdBytes,
    /// Note tags (typed).
    pub tags: Vec<Ed2kTag>,
    /// Unrecognized tags preserved raw.
    pub unknown_tags: Vec<RawTag>,
}

/// Kad2 Publish Notes response (`KADEMLIA2_PUBLISH_RES`, 0x4B):
/// `[target_file_hash:16][load:u8]` — same wire shape as source/keyword
/// publish responses; the byte is the indexer's store load.
#[derive(Debug, Clone, PartialEq)]
pub struct KadPublishNotesRes {
    /// Target file hash (16 bytes).
    pub target_file_hash: KadIdBytes,
    /// Indexer load report (eMule `uLoad`).
    pub load: u8,
}

/// Enum representing decoded Kad UDP messages.
#[derive(Debug, Clone, PartialEq)]
pub enum KadMessage {
    /// Kad2 Ping request.
    Ping(KadPing),
    /// Kad2 Pong response.
    Pong(KadPong),
    /// Kad2 Hello request.
    HelloReq(KadHelloReq),
    /// Kad2 Hello response.
    HelloRes(KadHelloRes),
    /// Kad2 Hello response acknowledgement (0x22, Kad >= 8).
    HelloResAck(KadHelloResAck),
    /// Legacy Kad TCP firewall check request (0x50).
    FirewalledReq(KadFirewalledReq),
    /// Kad TCP firewall check request v2 (0x53, Kad >= 7).
    FirewalledReq2(KadFirewalledReq2),
    /// Kad firewall check response (0x58) — echoes our public IP.
    FirewalledRes(KadFirewalledRes),
    /// Kad firewall check ack over UDP (0x59, deprecated since Kad 7).
    FirewalledAckRes,
    /// Kad2 Bootstrap request.
    BootstrapReq(KadBootstrapReq),
    /// Kad2 Bootstrap / Find Node response.
    BootstrapRes(KadBootstrapRes),
    /// Kad2 Generic Find Node / Search request.
    Req(KadReq),
    /// Kad2 Search Source request.
    SearchSourceReq(KadSearchSourceReq),
    /// Kad2 Search Source response.
    SearchSourceRes(KadSearchSourceRes),
    /// Kad2 Publish Source request.
    PublishSourceReq(KadPublishSourceReq),
    /// Kad2 Publish Source response.
    PublishSourceRes(KadPublishSourceRes),
    /// Kad2 Search Keyword request.
    SearchKeywordReq(KadSearchKeywordReq),
    /// Kad2 Search Keyword response.
    SearchKeywordRes(KadSearchKeywordRes),
    /// Kad2 Publish Keyword request.
    PublishKeywordReq(KadPublishKeywordReq),
    /// Kad2 Publish Keyword response.
    PublishKeywordRes(KadPublishKeywordRes),
    /// Kad2 Firewall UDP test packet.
    FirewallUdp(KadFirewallUdp),
    /// Kad buddy search request.
    FindBuddyReq(KadFindBuddyReq),
    /// Kad buddy search response.
    FindBuddyRes(KadFindBuddyRes),
    /// Kad buddy callback relay request.
    CallbackReq(KadCallbackReq),
    /// Kad2 Search Notes request.
    SearchNotesReq(KadSearchNotesReq),
    /// Notes search response (0x3B unified or legacy 0x3A).
    SearchNotesRes(KadSearchNotesRes),
    /// Kad2 Publish Notes request.
    PublishNotesReq(KadPublishNotesReq),
    /// Kad2 Publish Notes response.
    PublishNotesRes(KadPublishNotesRes),
}
