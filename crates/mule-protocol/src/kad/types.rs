//! Wire types and constants for Kad UDP packets.

/// 128-bit Kad ID alias (16 bytes).
pub type KadIdBytes = [u8; 16];

/// Maximum contacts in a Kad2 Bootstrap or Find Node response (standard K = 20, bounded to 50).
pub const MAX_KAD_RES_CONTACTS: usize = 50;

/// Maximum sources returned in a Kad2 Search Source response.
pub const MAX_KAD_SEARCH_SOURCES: usize = 50;

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
    /// Sender UDP port.
    pub udp_port: u16,
    /// Kad version (e.g., 2).
    pub version: u8,
    /// Sender TCP port (optional, 0 if none).
    pub tcp_port: u16,
}

/// Kad2 Hello Response packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadHelloRes {
    /// Responder Kad ID.
    pub kad_id: KadIdBytes,
    /// Responder UDP port.
    pub udp_port: u16,
    /// Kad version.
    pub version: u8,
    /// Responder TCP port (optional).
    pub tcp_port: u16,
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
    /// TCP port (0 if none).
    pub tcp_port: u16,
    /// Encryption/capability flags.
    pub encryption: u8,
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
}

/// Wire representation of a keyword search entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadKeywordEntryWire {
    /// 128-bit file hash.
    pub file_hash: KadIdBytes,
    /// File name (UTF-8).
    pub file_name: String,
    /// File size in bytes.
    pub file_size: u64,
    /// Number of sources advertising this file.
    pub source_count: u32,
}

/// Kad2 Search Keyword Request (0x33).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadSearchKeywordReq {
    /// 128-bit MD4 hash of the keyword.
    pub keyword_hash: KadIdBytes,
    /// Sender Kad ID.
    pub sender_id: KadIdBytes,
    /// Start position for pagination.
    pub start_pos: u16,
}

/// Kad2 Search Keyword Response (0x3B, unified search response, or legacy 0x2C).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadSearchKeywordRes {
    /// 128-bit MD4 hash of the keyword.
    pub keyword_hash: KadIdBytes,
    /// Discovered file entries.
    pub entries: Vec<KadKeywordEntryWire>,
}

/// Kad2 Publish Keyword Request (0x43).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadPublishKeywordReq {
    /// 128-bit MD4 hash of the keyword.
    pub keyword_hash: KadIdBytes,
    /// File entries published for this keyword.
    pub entries: Vec<KadKeywordEntryWire>,
}

/// Kad2 Publish Keyword Response (0x4B, unified publish response).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadPublishKeywordRes {
    /// 128-bit MD4 hash of the keyword.
    pub keyword_hash: KadIdBytes,
    /// Status code (0 = success).
    pub status: u8,
}

/// Enum representing decoded Kad UDP messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KadMessage {
    /// Kad2 Ping request.
    Ping(KadPing),
    /// Kad2 Pong response.
    Pong(KadPong),
    /// Kad2 Hello request.
    HelloReq(KadHelloReq),
    /// Kad2 Hello response.
    HelloRes(KadHelloRes),
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
}
