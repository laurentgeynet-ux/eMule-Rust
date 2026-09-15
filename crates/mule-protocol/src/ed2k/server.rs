//! eD2K client-server wire message definitions and bounded codecs.
//!
//! Provides deterministic structures and parsers for connecting to an eD2K server,
//! logging in, and decoding server responses without unbounded allocations.

use bytes::Bytes;

use super::encoder::Ed2kEncoder;
use super::error::{Ed2kEncodeError, Ed2kErrorKind, Ed2kParseError};
use super::limits::Ed2kLimits;
use super::opcodes::{
    OP_IDCHANGE, OP_LOGINREQUEST, OP_SERVERIDENT, OP_SERVERMESSAGE, OP_SERVERSTATUS,
};
use super::protocol::PR_ED2K;
use super::tags::{
    Ed2kTag, RawTag, TagName, TagReader, TagValue, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
};
use crate::framing::RawPacket;

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
    /// Creates a new default login request with user hash, port, and standard nickname and version tags.
    pub fn new(user_hash: [u8; 16], tcp_port: u16, nickname: &str, version: u32) -> Self {
        let tags = vec![
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_NAME),
                value: TagValue::String(nickname.to_string()),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_VERSION),
                value: TagValue::UInt32(version),
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

        let payload_len = buf.len() + 1; // + 1 for opcode
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
    /// Other unparsed packet.
    Other {
        /// Opcode of unparsed packet.
        opcode: u8,
        /// Raw payload bytes.
        payload: Bytes,
    },
}

/// Decodes a server packet payload given its opcode and protocol.
pub fn decode_server_response(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    limits: &Ed2kLimits,
) -> Result<ServerResponse, Ed2kParseError> {
    if protocol != PR_ED2K {
        return Ok(ServerResponse::Other {
            opcode,
            payload: Bytes::copy_from_slice(payload),
        });
    }

    match opcode {
        OP_SERVERMESSAGE => {
            let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));
            let message = reader.read_string(limits.max_string_bytes, "server_message")?;
            Ok(ServerResponse::Message(ServerMessage { message }))
        }

        OP_IDCHANGE => {
            let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));
            let client_id = reader.read_u32_le("server_client_id")?;
            let tcp_flags = if reader.remaining() >= 4 {
                reader.read_u32_le("server_tcp_flags")?
            } else {
                0
            };
            Ok(ServerResponse::IdChange(ServerIdChange {
                client_id,
                tcp_flags,
            }))
        }

        OP_SERVERSTATUS => {
            let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));
            let current_users = reader.read_u32_le("server_current_users")?;
            let current_files = reader.read_u32_le("server_current_files")?;
            Ok(ServerResponse::Status(ServerStatus {
                current_users,
                current_files,
            }))
        }

        OP_SERVERIDENT => {
            let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));
            let server_hash = reader.read_hash("server_hash")?;
            let ip = reader.read_u32_le("server_ip")?;
            let port = reader.read_u16_le("server_port")?;
            let tag_count = reader.read_u32_le("server_tag_count")? as usize;

            if tag_count > limits.max_tags {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    reader.pos() - 4,
                    "server_tag_count",
                    Ed2kErrorKind::TooManyTags {
                        count: tag_count,
                        limit: limits.max_tags,
                    },
                ));
            }

            let mut tags = Vec::with_capacity(tag_count.min(64));
            let mut unknown_tags = Vec::new();

            for _ in 0..tag_count {
                let (typed_opt, raw_opt) = reader.read_tag(limits)?;
                if let Some(tag) = typed_opt {
                    tags.push(tag);
                }
                if let Some(raw) = raw_opt {
                    unknown_tags.push(raw);
                }
            }

            Ok(ServerResponse::Ident(ServerIdent {
                server_hash,
                ip,
                port,
                tags,
                unknown_tags,
            }))
        }

        _ => Ok(ServerResponse::Other {
            opcode,
            payload: Bytes::copy_from_slice(payload),
        }),
    }
}

