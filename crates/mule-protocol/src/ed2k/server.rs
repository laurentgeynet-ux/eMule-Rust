//! eD2K client-server wire message definitions and bounded codecs.
//!
//! Provides deterministic structures and parsers for connecting to an eD2K server,
//! logging in, and decoding server responses without unbounded allocations.

pub mod search;
pub mod sources;
pub mod types;

use bytes::Bytes;

pub use search::{SearchQuery, SearchResult, SearchResultFile};
pub use sources::{
    FoundSourcePeer, FoundSourcePeerObfu, FoundSources, FoundSourcesObfu, GetSourcesRequest,
};
pub use types::{
    ed2k_file_type_term, CallbackRequest, CallbackRequested, OfferFiles, OfferedFile,
    ServerIdChange, ServerIdent, ServerLoginRequest, ServerMessage, ServerResponse, ServerStatus,
    OFFER_COMPLETE_CLIENT_ID, OFFER_COMPLETE_CLIENT_PORT, SRV_TCPFLG_COMPRESSION,
    SRV_TCPFLG_LARGEFILES,
};

use super::error::{Ed2kErrorKind, Ed2kParseError};
use super::limits::Ed2kLimits;
use super::opcodes::{
    OP_CALLBACKREQUESTED, OP_CALLBACK_FAIL, OP_FOUNDSOURCES, OP_FOUNDSOURCES_OBFU, OP_IDCHANGE,
    OP_SEARCH_RESULT, OP_SERVERIDENT, OP_SERVERLIST, OP_SERVERMESSAGE, OP_SERVERSTATUS,
    OP_SERVERSTATUS_LEGACY,
};
use super::protocol::PR_ED2K;
use super::tags::TagReader;

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

        OP_SERVERLIST => {
            let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));
            let count = reader.read_u8("server_list_count")? as usize;
            let mut servers = Vec::with_capacity(count.min(256));
            for _ in 0..count {
                let ip = reader.read_u32_le("server_list_ip")?;
                let port = reader.read_u16_le("server_list_port")?;
                servers.push((ip, port));
            }
            Ok(ServerResponse::ServerList { servers })
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

        OP_CALLBACKREQUESTED => {
            if payload.len() < 6 {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    0,
                    "callback_requested",
                    Ed2kErrorKind::UnexpectedEof {
                        needed: 6,
                        available: payload.len(),
                    },
                ));
            }
            let ip = u32::from_le_bytes(payload[0..4].try_into().unwrap());
            let port = u16::from_le_bytes(payload[4..6].try_into().unwrap());
            let (crypt_options, user_hash) = if payload.len() >= 23 {
                let mut h = [0u8; 16];
                h.copy_from_slice(&payload[7..23]);
                (Some(payload[6]), Some(h))
            } else {
                (None, None)
            };
            Ok(ServerResponse::CallbackRequested(CallbackRequested {
                ip,
                port,
                crypt_options,
                user_hash,
            }))
        }

        OP_CALLBACK_FAIL => {
            let client_id = if payload.len() >= 4 {
                Some(u32::from_le_bytes(payload[0..4].try_into().unwrap()))
            } else {
                None
            };
            Ok(ServerResponse::CallbackFail { client_id })
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
