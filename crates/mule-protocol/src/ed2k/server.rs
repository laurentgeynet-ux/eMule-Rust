//! eD2K client-server wire message definitions and bounded codecs.
//!
//! Provides deterministic structures and parsers for connecting to an eD2K server,
//! logging in, and decoding server responses without unbounded allocations.

use bytes::Bytes;

use super::encoder::Ed2kEncoder;
use super::error::{Ed2kEncodeError, Ed2kErrorKind, Ed2kParseError};
use super::limits::Ed2kLimits;
use super::opcodes::{
    OP_FOUNDSOURCES, OP_FOUNDSOURCES_OBFU, OP_GETSOURCES, OP_GETSOURCES_OBFU, OP_IDCHANGE, OP_LOGINREQUEST,
    OP_SEARCHREQUEST, OP_SEARCH_RESULT, OP_SERVERIDENT, OP_SERVERMESSAGE, OP_SERVERSTATUS,
    OP_SERVERSTATUS_LEGACY,
};
use super::protocol::PR_ED2K;
use super::tags::{
    Ed2kTag, RawTag, TagName, TagReader, TagValue, SRVCAP_LARGEFILES, SRVCAP_NEWTAGS,
    SRVCAP_UNICODE, TAG_SPECIAL_CT_EMULE_VERSION, TAG_SPECIAL_EMULE_MISCOPTIONS1,
    TAG_SPECIAL_EMULE_VERSION, TAG_SPECIAL_FILESIZE, TAG_SPECIAL_NAME,
    TAG_SPECIAL_SERVER_FLAGS, TAG_SPECIAL_VERSION,
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
    /// Creates a new default login request with user hash, port, and standard modern nickname and version tags (eMule 0.60d compatible).
    pub fn new(user_hash: [u8; 16], tcp_port: u16, nickname: &str, version: u32) -> Self {
        // eMule 0.60d version encodings:
        // Packed integer: (major << 17) | (minor << 10) | (update << 7)
        // 0.60d => major=0, minor=60, update=4 ('d')
        let emule_ver_packed: u32 = (0 << 17) | (60 << 10) | (4 << 7);
        // Extended format: (vMajor << 24) | (vMinor << 17) | (vUpdate)
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
                value: TagValue::UInt32(0x01), // Support extended requests
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
    /// Search results returned by server.
    SearchResult(SearchResult),
    /// Sources returned by server for a requested file hash.
    FoundSources(FoundSources),
    /// Sources returned by server with obfuscation metadata (OP_FOUNDSOURCES_OBFU).
    FoundSourcesObfu(FoundSourcesObfu),
    /// Other unparsed packet.
    Other {
        /// Opcode of unparsed packet.
        opcode: u8,
        /// Raw payload bytes.
        payload: Bytes,
    },
}

/// Request to search files on an eD2K server (`OP_SEARCHREQUEST` = `0x16`).
#[derive(Debug, Clone, PartialEq)]
pub struct SearchQuery {
    /// Text search query string.
    pub term: String,
    /// Optional minimum file size filter in bytes.
    pub min_size: Option<u64>,
    /// Optional maximum file size filter in bytes.
    pub max_size: Option<u64>,
    /// Optional file extension / format filter (e.g. "iso", "zip").
    pub extension: Option<String>,
}

impl SearchQuery {
    /// Creates a simple search query by keyword.
    pub fn new(term: impl Into<String>) -> Self {
        Self {
            term: term.into(),
            min_size: None,
            max_size: None,
            extension: None,
        }
    }

    /// Encodes the search request into a `RawPacket` as a canonical eD2K Query Tree.
    ///
    /// The root is either a string leaf (0x01) or a chain of binary boolean AND operators (0x00, 0x00)
    /// combining the keyword string leaf and any constraint leaves (min_size, max_size, extension).
    pub fn encode(&self, limits: &Ed2kLimits) -> Result<RawPacket, Ed2kEncodeError> {
        let term_bytes = self.term.as_bytes();
        if term_bytes.len() > limits.max_string_bytes {
            return Err(Ed2kEncodeError::new(
                OP_SEARCHREQUEST,
                "term",
                Ed2kErrorKind::OversizedString {
                    size: term_bytes.len(),
                    limit: limits.max_string_bytes,
                },
            ));
        }

        // 1. Encode term leaf: 0x01 (EDONKEY_SEARCH_NAME) + u16 len + term bytes
        let mut term_leaf = Vec::with_capacity(3 + term_bytes.len());
        term_leaf.push(0x01);
        term_leaf.extend_from_slice(&(term_bytes.len() as u16).to_le_bytes());
        term_leaf.extend_from_slice(term_bytes);

        // 2. Prepare filter leaves
        let mut filter_leaves = Vec::new();

        // Min size filter leaf:
        // 0x03 (EDONKEY_SEARCH_LIMIT) + limit (u32 le) + 0x01 (EDONKEY_SEARCH_MIN) + tag_name_len (u16 le = 1) + 0x02 (TAG_SPECIAL_FILESIZE)
        if let Some(min) = self.min_size {
            let mut leaf = Vec::with_capacity(9);
            leaf.push(0x03);
            let val = (min.min(u32::MAX as u64)) as u32;
            leaf.extend_from_slice(&val.to_le_bytes());
            leaf.push(0x01); // MIN
            leaf.extend_from_slice(&1u16.to_le_bytes());
            leaf.push(TAG_SPECIAL_FILESIZE);
            filter_leaves.push(leaf);
        }

        // Max size filter leaf:
        // 0x03 (EDONKEY_SEARCH_LIMIT) + limit (u32 le) + 0x02 (EDONKEY_SEARCH_MAX) + tag_name_len (u16 le = 1) + 0x02 (TAG_SPECIAL_FILESIZE)
        if let Some(max) = self.max_size {
            let mut leaf = Vec::with_capacity(9);
            leaf.push(0x03);
            let val = (max.min(u32::MAX as u64)) as u32;
            leaf.extend_from_slice(&val.to_le_bytes());
            leaf.push(0x02); // MAX
            leaf.extend_from_slice(&1u16.to_le_bytes());
            leaf.push(TAG_SPECIAL_FILESIZE);
            filter_leaves.push(leaf);
        }

        // Extension / Format filter leaf:
        // 0x02 (EDONKEY_SEARCH_META) + str_len (u16 le) + str_bytes + tag_name_len (u16 le = 1) + 0x04 (EDONKEY_STAG_FORMAT)
        if let Some(ext) = &self.extension {
            let ext_bytes = ext.as_bytes();
            let mut leaf = Vec::with_capacity(6 + ext_bytes.len());
            leaf.push(0x02); // META
            leaf.extend_from_slice(&(ext_bytes.len() as u16).to_le_bytes());
            leaf.extend_from_slice(ext_bytes);
            leaf.extend_from_slice(&1u16.to_le_bytes());
            leaf.push(0x04); // EDONKEY_STAG_FORMAT
            filter_leaves.push(leaf);
        }

        // 3. Assemble binary tree in preorder traversal:
        // If no filters: [term_leaf]
        // If 1 filter: [0x00, 0x00] (AND) + [term_leaf] + [filter_1]
        // If N filters: AND( ... AND(term_leaf, filter_1) ..., filter_N)
        let mut current_tree = term_leaf;
        for filter in filter_leaves {
            let mut new_tree = Vec::with_capacity(2 + current_tree.len() + filter.len());
            new_tree.push(0x00); // EDONKEY_SEARCH_BOOL
            new_tree.push(0x00); // EDONKEY_SEARCH_AND
            new_tree.extend_from_slice(&current_tree);
            new_tree.extend_from_slice(&filter);
            current_tree = new_tree;
        }

        let buf = current_tree;

        let payload_len = buf.len() + 1; // + 1 for opcode
        if payload_len > limits.max_frame_bytes {
            return Err(Ed2kEncodeError::new(
                OP_SEARCHREQUEST,
                "search_query",
                Ed2kErrorKind::FrameTooLarge {
                    size: payload_len,
                    limit: limits.max_frame_bytes,
                },
            ));
        }

        Ok(RawPacket::with_vec(PR_ED2K, OP_SEARCHREQUEST, buf))
    }
}

/// A single file result returned by the server search.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResultFile {
    /// 16-byte file hash.
    pub file_hash: [u8; 16],
    /// Client ID of the publisher or 0.
    pub client_id: u32,
    /// Client port of the publisher or 0.
    pub client_port: u16,
    /// File tags (Name, Size, Sources count, etc.).
    pub tags: Vec<Ed2kTag>,
}

impl SearchResultFile {
    /// Helper to extract filename from tags.
    pub fn name(&self) -> Option<String> {
        self.tags.iter().find_map(|t| match (&t.name, &t.value) {
            (TagName::Id(TAG_SPECIAL_NAME), TagValue::String(s)) => Some(s.clone()),
            _ => None,
        })
    }

    /// Helper to extract file size in bytes from tags.
    pub fn size(&self) -> Option<u64> {
        self.tags.iter().find_map(|t| match (&t.name, &t.value) {
            (TagName::Id(TAG_SPECIAL_FILESIZE), TagValue::UInt32(v)) => Some(*v as u64),
            (TagName::Id(TAG_SPECIAL_FILESIZE), TagValue::UInt64(v)) => Some(*v),
            _ => None,
        })
    }

    /// Helper to extract source count from tags.
    pub fn source_count(&self) -> Option<u32> {
        self.tags.iter().find_map(|t| {
            let is_source_tag = match &t.name {
                TagName::Id(0x15) | TagName::Id(0x30) => true,
                TagName::Text(txt) => {
                    let lower = txt.to_lowercase();
                    lower.contains("source") || lower.contains("availability") || lower.contains("complete")
                }
                _ => false,
            };
            if is_source_tag {
                match &t.value {
                    TagValue::UInt8(v) => Some(*v as u32),
                    TagValue::UInt16(v) => Some(*v as u32),
                    TagValue::UInt32(v) => Some(*v),
                    TagValue::UInt64(v) => Some(*v as u32),
                    _ => None,
                }
            } else {
                None
            }
        })
    }
}

/// Search result packet payload (`OP_SEARCH_RESULT` = `0x33`).
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    /// List of matched files.
    pub files: Vec<SearchResultFile>,
}

/// Request to find sources for a specific file (`OP_GETSOURCES` = `0x19` or `OP_GETSOURCES_OBFU` = `0x23`).
#[derive(Debug, Clone, PartialEq)]
pub struct GetSourcesRequest {
    /// 16-byte MD4 file hash.
    pub file_hash: [u8; 16],
    /// File size in bytes.
    pub file_size: u64,
    /// Whether to request sources with obfuscation parameters (`OP_GETSOURCES_OBFU` = `0x23`).
    pub obfuscation: bool,
}

impl GetSourcesRequest {
    /// Creates a new GetSourcesRequest (legacy plaintext OP_GETSOURCES = 0x19).
    pub fn new(file_hash: [u8; 16], file_size: u64) -> Self {
        Self {
            file_hash,
            file_size,
            obfuscation: false,
        }
    }

    /// Creates a new GetSourcesRequest with obfuscation toggle (0x23 if true, 0x19 if false).
    pub fn with_obfuscation(file_hash: [u8; 16], file_size: u64, obfuscation: bool) -> Self {
        Self {
            file_hash,
            file_size,
            obfuscation,
        }
    }

    /// Encodes the request into a `RawPacket`.
    pub fn encode(&self, limits: &Ed2kLimits) -> Result<RawPacket, Ed2kEncodeError> {
        let opcode = if self.obfuscation {
            OP_GETSOURCES_OBFU
        } else {
            OP_GETSOURCES
        };
        let mut buf = Vec::with_capacity(24);
        buf.extend_from_slice(&self.file_hash);

        // eD2K standard encodes size as 32-bit uint if <= 4GB, or 64-bit if large
        if self.file_size <= u32::MAX as u64 {
            buf.extend_from_slice(&(self.file_size as u32).to_le_bytes());
        } else {
            buf.extend_from_slice(&0u32.to_le_bytes());
            buf.extend_from_slice(&self.file_size.to_le_bytes());
        }

        let payload_len = buf.len() + 1;
        if payload_len > limits.max_frame_bytes {
            return Err(Ed2kEncodeError::new(
                opcode,
                "get_sources",
                Ed2kErrorKind::FrameTooLarge {
                    size: payload_len,
                    limit: limits.max_frame_bytes,
                },
            ));
        }

        Ok(RawPacket::with_vec(PR_ED2K, opcode, buf))
    }
}

/// A peer source entry returned by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundSourcePeer {
    /// IPv4 address in network byte order or host integer.
    pub ip: u32,
    /// Port number.
    pub port: u16,
}

impl FoundSourcePeer {
    /// Converts the source address to "A.B.C.D:port" string format.
    pub fn to_address_string(&self) -> String {
        let bytes = self.ip.to_le_bytes();
        format!(
            "{}.{}.{}.{}:{}",
            bytes[0], bytes[1], bytes[2], bytes[3], self.port
        )
    }
}

/// Found sources response from server (`OP_FOUNDSOURCES` = `0x42`).
#[derive(Debug, Clone, PartialEq)]
pub struct FoundSources {
    /// 16-byte MD4 file hash.
    pub file_hash: [u8; 16],
    /// Sources list.
    pub sources: Vec<FoundSourcePeer>,
}

/// A peer source entry with obfuscation flags and optional UserHash (`OP_FOUNDSOURCES_OBFU` = `0x44`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundSourcePeerObfu {
    /// IPv4 address in network byte order / host integer.
    pub ip: u32,
    /// Port number.
    pub port: u16,
    /// Obfuscation settings byte.
    pub obf_settings: u8,
    /// Optional 16-byte user hash (present if obf_settings & 0x80 != 0).
    pub user_hash: Option<[u8; 16]>,
}

impl FoundSourcePeerObfu {
    /// Converts the source address to "A.B.C.D:port" string format.
    pub fn to_address_string(&self) -> String {
        let bytes = self.ip.to_le_bytes();
        format!(
            "{}.{}.{}.{}:{}",
            bytes[0], bytes[1], bytes[2], bytes[3], self.port
        )
    }

    /// Whether this peer supports protocol obfuscation (bit 0).
    #[inline]
    pub fn supports_crypt(&self) -> bool {
        (self.obf_settings & 0x01) != 0
    }

    /// Whether this peer requests protocol obfuscation (bit 1).
    #[inline]
    pub fn requests_crypt(&self) -> bool {
        (self.obf_settings & 0x02) != 0
    }

    /// Whether this peer strictly requires protocol obfuscation (bit 2).
    #[inline]
    pub fn requires_crypt(&self) -> bool {
        (self.obf_settings & 0x04) != 0
    }
}

/// Found sources response with obfuscation from server (`OP_FOUNDSOURCES_OBFU` = `0x44`).
#[derive(Debug, Clone, PartialEq)]
pub struct FoundSourcesObfu {
    /// 16-byte MD4 file hash.
    pub file_hash: [u8; 16],
    /// Sources list with obfuscation parameters.
    pub sources: Vec<FoundSourcePeerObfu>,
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

        OP_SERVERSTATUS | OP_SERVERSTATUS_LEGACY => {
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

        OP_SEARCH_RESULT => {
            let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));
            let count = reader.read_u32_le("search_result_count")? as usize;

            if count > limits.max_tags * 4 {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    reader.pos() - 4,
                    "search_result_count",
                    Ed2kErrorKind::TooManyTags {
                        count,
                        limit: limits.max_tags * 4,
                    },
                ));
            }

            let mut files = Vec::with_capacity(count.min(256));
            for _ in 0..count {
                let file_hash = reader.read_hash("file_hash")?;
                let client_id = reader.read_u32_le("client_id")?;
                let client_port = reader.read_u16_le("client_port")?;
                let tag_count = reader.read_u32_le("tag_count")? as usize;

                if tag_count > limits.max_tags {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        reader.pos() - 4,
                        "file_tag_count",
                        Ed2kErrorKind::TooManyTags {
                            count: tag_count,
                            limit: limits.max_tags,
                        },
                    ));
                }

                let mut tags = Vec::with_capacity(tag_count.min(32));
                for _ in 0..tag_count {
                    let (typed_opt, _) = reader.read_tag(limits)?;
                    if let Some(tag) = typed_opt {
                        tags.push(tag);
                    }
                }

                files.push(SearchResultFile {
                    file_hash,
                    client_id,
                    client_port,
                    tags,
                });
            }

            Ok(ServerResponse::SearchResult(SearchResult { files }))
        }

        OP_FOUNDSOURCES => {
            let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));
            let file_hash = reader.read_hash("file_hash")?;
            let count = reader.read_u8("sources_count")? as usize;

            let mut sources = Vec::with_capacity(count);
            for _ in 0..count {
                let ip = reader.read_u32_le("source_ip")?;
                let port = reader.read_u16_le("source_port")?;
                sources.push(FoundSourcePeer { ip, port });
            }

            Ok(ServerResponse::FoundSources(FoundSources {
                file_hash,
                sources,
            }))
        }

        OP_FOUNDSOURCES_OBFU => {
            let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));
            let file_hash = reader.read_hash("file_hash")?;
            let count = reader.read_u8("sources_count")? as usize;

            let mut sources = Vec::with_capacity(count);
            for _ in 0..count {
                let ip = reader.read_u32_le("source_ip")?;
                let port = reader.read_u16_le("source_port")?;
                let obf_settings = reader.read_u8("obf_settings")?;
                let user_hash = if (obf_settings & 0x80) != 0 {
                    Some(reader.read_hash("source_user_hash")?)
                } else {
                    None
                };
                sources.push(FoundSourcePeerObfu {
                    ip,
                    port,
                    obf_settings,
                    user_hash,
                });
            }

            Ok(ServerResponse::FoundSourcesObfu(FoundSourcesObfu {
                file_hash,
                sources,
            }))
        }

        _ => Ok(ServerResponse::Other {
            opcode,
            payload: Bytes::copy_from_slice(payload),
        }),
    }
}

