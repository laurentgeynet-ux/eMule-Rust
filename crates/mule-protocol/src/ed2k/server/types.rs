//! Core types and structures for eD2K server communication.

use bytes::Bytes;
use crate::framing::RawPacket;
use crate::ed2k::encoder::Ed2kEncoder;
use crate::ed2k::error::{Ed2kEncodeError, Ed2kErrorKind};
use crate::ed2k::limits::Ed2kLimits;
use crate::ed2k::opcodes::OP_LOGINREQUEST;
use crate::ed2k::protocol::PR_ED2K;
use crate::ed2k::tags::{
    Ed2kTag, RawTag, TagName, TagValue, SRVCAP_LARGEFILES, SRVCAP_NEWTAGS,
    SRVCAP_UNICODE, TAG_SPECIAL_CT_EMULE_VERSION, TAG_SPECIAL_EMULE_MISCOPTIONS1,
    TAG_SPECIAL_EMULE_VERSION, TAG_SPECIAL_NAME, TAG_SPECIAL_SERVER_FLAGS,
    TAG_SPECIAL_VERSION,
};

/// Client login request sent to an eD2K server (`OP_LOGINREQUEST` = `0x01` under `PR_ED2K`).
#[derive(Debug, Clone, PartialEq)]
pub struct ServerLoginRequest {
    /// 16-byte MD4 User Hash.
    pub user_hash: [u8; 16],
    /// Current client ID (usually 0 before server assigns one).
    pub client_id: u32,
    /// Local listening TCP port.
    pub tcp_port: u16,
    /// Client tags (e.g. Nickname, Version, Flags).
    pub tags: Vec<Ed2kTag>,
}

impl ServerLoginRequest {
    /// Creates a new default login request with user hash, port, and standard modern nickname and version tags (eMule 0.60d compatible).
    pub fn new(user_hash: [u8; 16], tcp_port: u16, nickname: &str, version: u32) -> Self {
        let emule_ver_packed: u32 = (0 << 17) | (60 << 10) | (4 << 7);
        let emule_ver_ext: u32 = (0x3c << 24) | (60 << 17) | 4;

        let tags = vec![
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_NAME),
                value: TagValue::String(nickname.to_string()),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_VERSION),
                value: TagValue::UInt32(version),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_SERVER_FLAGS),
                value: TagValue::UInt32(SRVCAP_NEWTAGS | SRVCAP_UNICODE | SRVCAP_LARGEFILES),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_EMULE_VERSION),
                value: TagValue::UInt32(emule_ver_ext),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_CT_EMULE_VERSION),
                value: TagValue::UInt32(emule_ver_packed),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_EMULE_MISCOPTIONS1),
                value: TagValue::UInt32(0x01),
            },
        ];
        Self {
            user_hash,
            client_id: 0,
            tcp_port,
            tags,
        }
    }

    /// Encodes the login request into a `RawPacket`.
    pub fn encode(&self, limits: &Ed2kLimits) -> Result<RawPacket, Ed2kEncodeError> {
        let encoder = Ed2kEncoder::with_limits(limits.clone());
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.user_hash);
        buf.extend_from_slice(&self.client_id.to_le_bytes());
        buf.extend_from_slice(&self.tcp_port.to_le_bytes());
        buf.extend_from_slice(&(self.tags.len() as u32).to_le_bytes());

        for tag in &self.tags {
            encoder.encode_tag(OP_LOGINREQUEST, tag, &mut buf)?;
        }

        let payload_len = buf.len() + 1;
        if payload_len > limits.max_frame_bytes {
            return Err(Ed2kEncodeError::new(
                OP_LOGINREQUEST,
                "login_request_payload",
                Ed2kErrorKind::FrameTooLarge {
                    size: payload_len,
                    limit: limits.max_frame_bytes,
                },
            ));
        }

        Ok(RawPacket::with_vec(PR_ED2K, OP_LOGINREQUEST, buf))
    }
}

/// Server message containing server greetings or MOTD (`OP_SERVERMESSAGE` = `0x38`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerMessage {
    /// Message string sent by the server.
    pub message: String,
}

/// Server ID change response (`OP_IDCHANGE` = `0x40`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerIdChange {
    /// Assigned Client ID.
    /// If `>= 16777216`, client is assigned HighID.
    /// If `< 16777216`, client is behind NAT/firewall and assigned LowID.
    pub client_id: u32,
    /// Server TCP flags / capabilities (historical).
    pub tcp_flags: u32,
}

impl ServerIdChange {
    /// Returns true if the client ID qualifies as HighID (`>= 16_777_216`).
    #[inline]
    pub fn is_high_id(&self) -> bool {
        self.client_id >= 16_777_216
    }
}

/// Server status report (`OP_SERVERSTATUS` = `0x14`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerStatus {
    /// Total active users connected to the server.
    pub current_users: u32,
    /// Total files currently indexed by the server.
    pub current_files: u32,
}

/// Server identification response (`OP_SERVERIDENT` = `0x41`).
#[derive(Debug, Clone, PartialEq)]
pub struct ServerIdent {
    /// 16-byte server hash.
    pub server_hash: [u8; 16],
    /// Server IPv4 address in network order.
    pub ip: u32,
    /// Server listening port.
    pub port: u16,
    /// Server tags (name, description, etc.).
    pub tags: Vec<Ed2kTag>,
    /// Unknown / raw tags.
    pub unknown_tags: Vec<RawTag>,
}

impl ServerIdent {
    /// Helper to find server name in tags.
    pub fn name(&self) -> Option<String> {
        self.tags.iter().find_map(|t| match (&t.name, &t.value) {
            (TagName::Id(TAG_SPECIAL_NAME), TagValue::String(s)) => Some(s.clone()),
            _ => None,
        })
    }
}

/// Decoded server-to-client message.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerResponse {
    /// Server notice / MOTD string.
    Message(ServerMessage),
    /// Assigned ClientID / HighID or LowID.
    IdChange(ServerIdChange),
    /// Current users and files statistics.
    Status(ServerStatus),
    /// Server identification and metadata.
    Ident(ServerIdent),
    /// Search results returned by server.
    SearchResult(super::search::SearchResult),
    /// Sources returned by server for a requested file hash.
    FoundSources(super::sources::FoundSources),
    /// Sources returned by server with obfuscation metadata (OP_FOUNDSOURCES_OBFU).
    FoundSourcesObfu(super::sources::FoundSourcesObfu),
    /// Other unparsed packet.
    Other {
        /// Opcode of unparsed packet.
        opcode: u8,
        /// Raw payload bytes.
        payload: Bytes,
    },
}
