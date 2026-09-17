//! Bounded encoding and decoding for Kad UDP packets (Ping, Pong, Hello, HelloAnswer, Bootstrap, Req/Res).

use bytes::{Buf, BufMut, Bytes, BytesMut};
use crate::ed2k::{Ed2kErrorKind, Ed2kParseError};
use crate::kad::opcodes::*;

/// 128-bit Kad ID alias (16 bytes).
pub type KadIdBytes = [u8; 16];

/// Maximum contacts in a Kad2 Bootstrap or Find Node response (standard K = 20, bounded to 50).
pub const MAX_KAD_RES_CONTACTS: usize = 50;

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

/// Maximum sources returned in a Kad2 Search Source response.
pub const MAX_KAD_SEARCH_SOURCES: usize = 50;

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

/// Kad2 Search Source Request (0x22).
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

/// Kad2 Search Source Response (0x2A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadSearchSourceRes {
    /// Target file hash (16 bytes).
    pub target_file_hash: KadIdBytes,
    /// Discovered sources (bounded to MAX_KAD_SEARCH_SOURCES).
    pub sources: Vec<KadSourceWire>,
}

/// Kad2 Publish Source Request (0x23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadPublishSourceReq {
    /// Target file hash (16 bytes).
    pub target_file_hash: KadIdBytes,
    /// Contact announcing the file.
    pub contact: KadContactWire,
    /// File size in bytes.
    pub file_size: u64,
}

/// Kad2 Publish Source Response (0x2B).
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

/// Kad2 Search Keyword Request (0x24).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadSearchKeywordReq {
    /// 128-bit MD4 hash of the keyword.
    pub keyword_hash: KadIdBytes,
    /// Sender Kad ID.
    pub sender_id: KadIdBytes,
    /// Start position for pagination.
    pub start_pos: u16,
}

/// Kad2 Search Keyword Response (0x2C).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadSearchKeywordRes {
    /// 128-bit MD4 hash of the keyword.
    pub keyword_hash: KadIdBytes,
    /// Discovered file entries.
    pub entries: Vec<KadKeywordEntryWire>,
}

/// Kad2 Publish Keyword Request (0x25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadPublishKeywordReq {
    /// 128-bit MD4 hash of the keyword.
    pub keyword_hash: KadIdBytes,
    /// File entries published for this keyword.
    pub entries: Vec<KadKeywordEntryWire>,
}

/// Kad2 Publish Keyword Response (0x2D).
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

/// Encoder and decoder for Kad UDP wire datagrams.
pub struct KadCodec;

impl KadCodec {
    /// Decodes a raw UDP packet into a KadMessage.
    /// Invariant: raw_packet must not exceed MAX_KAD_UDP_PACKET_SIZE.
    pub fn decode(raw: &[u8]) -> Result<KadMessage, Ed2kParseError> {
        if raw.len() > MAX_KAD_UDP_PACKET_SIZE {
            return Err(Ed2kParseError::new(
                raw.first().copied().unwrap_or(PR_KADEMLIA),
                None,
                0,
                "KadUdpPacket",
                Ed2kErrorKind::FrameTooLarge {
                    size: raw.len(),
                    limit: MAX_KAD_UDP_PACKET_SIZE,
                },
            ));
        }
        if raw.len() < 2 {
            return Err(Ed2kParseError::new(
                raw.first().copied().unwrap_or(PR_KADEMLIA),
                None,
                0,
                "KadUdpPacket",
                Ed2kErrorKind::UnexpectedEof {
                    needed: 2,
                    available: raw.len(),
                },
            ));
        }

        let protocol = raw[0];
        if !is_valid_kad_protocol(protocol) {
            return Err(Ed2kParseError::new(
                protocol,
                None,
                0,
                "KadUdpPacket::protocol",
                Ed2kErrorKind::UnsupportedProtocol { byte: protocol },
            ));
        }

        let opcode = raw[1];
        let raw_payload = &raw[2..];
        let decompressed_holder;
        let mut payload = if protocol == PR_KADEMLIA_PACKED
            || (raw_payload.len() >= 2 && raw_payload[0] == 0x78 && (raw_payload[1] == 0x9C || raw_payload[1] == 0xDA || raw_payload[1] == 0x01 || raw_payload[1] == 0x5E))
        {
            use std::io::Read;
            use flate2::read::ZlibDecoder;
            let mut decoder = ZlibDecoder::new(raw_payload);
            let mut decomp = Vec::new();
            if decoder.read_to_end(&mut decomp).is_ok() && !decomp.is_empty() {
                decompressed_holder = decomp;
                &decompressed_holder[..]
            } else {
                raw_payload
            }
        } else {
            raw_payload
        };

        match opcode {
            KADEMLIA2_PING => {
                // Sender Kad ID (16 bytes) + UDP port (2 bytes) = 18 bytes
                if payload.len() < 18 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadPing",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 18,
                            available: payload.len(),
                        },
                    ));
                }
                let mut kad_id = [0u8; 16];
                payload.copy_to_slice(&mut kad_id);
                let udp_port = payload.get_u16_le();
                Ok(KadMessage::Ping(KadPing { kad_id, udp_port }))
            }
            KADEMLIA2_PONG => {
                // Client port (2 bytes)
                if payload.len() < 2 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadPong",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 2,
                            available: payload.len(),
                        },
                    ));
                }
                let client_port = payload.get_u16_le();
                Ok(KadMessage::Pong(KadPong { client_port }))
            }
            KADEMLIA2_HELLO_REQ | KADEMLIA_HELLO_REQ => {
                if payload.len() < 19 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadHelloReq",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 19,
                            available: payload.len(),
                        },
                    ));
                }
                let mut kad_id = [0u8; 16];
                payload.copy_to_slice(&mut kad_id);
                let udp_port = payload.get_u16_le();
                let version = payload.get_u8();
                let tcp_port = if payload.len() >= 2 {
                    payload.get_u16_le()
                } else {
                    0
                };
                Ok(KadMessage::HelloReq(KadHelloReq {
                    kad_id,
                    udp_port,
                    version,
                    tcp_port,
                }))
            }
            KADEMLIA2_HELLO_RES | KADEMLIA_HELLO_RES => {
                if payload.len() < 19 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadHelloRes",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 19,
                            available: payload.len(),
                        },
                    ));
                }
                let mut kad_id = [0u8; 16];
                payload.copy_to_slice(&mut kad_id);
                let udp_port = payload.get_u16_le();
                let version = payload.get_u8();
                let tcp_port = if payload.len() >= 2 {
                    payload.get_u16_le()
                } else {
                    0
                };
                Ok(KadMessage::HelloRes(KadHelloRes {
                    kad_id,
                    udp_port,
                    version,
                    tcp_port,
                }))
            }
            KADEMLIA2_BOOTSTRAP_REQ => {
                if payload.is_empty() {
                    Ok(KadMessage::BootstrapReq(KadBootstrapReq {
                        kad_id: [0u8; 16],
                        udp_port: 0,
                    }))
                } else if payload.len() >= 18 {
                    // Bootstrap req: Kad ID (16 bytes) + UDP port (2 bytes) + optional version (1 byte)
                    let mut kad_id = [0u8; 16];
                    payload.copy_to_slice(&mut kad_id);
                    let udp_port = payload.get_u16_le();
                    Ok(KadMessage::BootstrapReq(KadBootstrapReq { kad_id, udp_port }))
                } else {
                    Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadBootstrapReq",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 18,
                            available: payload.len(),
                        },
                    ))
                }
            }
            KADEMLIA2_BOOTSTRAP_RES => {
                // Kad ID (16 bytes) + UDP port (2 bytes) + Version (1 byte) + Count (2 bytes) = 21 bytes header
                if payload.len() < 21 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadBootstrapRes",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 21,
                            available: payload.len(),
                        },
                    ));
                }
                let mut kad_id = [0u8; 16];
                payload.copy_to_slice(&mut kad_id);
                let udp_port = payload.get_u16_le();
                let version = payload.get_u8();
                let count = payload.get_u16_le() as usize;

                if count > 100 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        21,
                        "KadBootstrapRes::contacts",
                        Ed2kErrorKind::FrameTooLarge {
                            size: count,
                            limit: 100,
                        },
                    ));
                }

                // Decode available contacts bounded by packet length and MAX_KAD_RES_CONTACTS
                let max_contacts = (payload.len() / 25).min(count).min(MAX_KAD_RES_CONTACTS);
                let mut contacts = Vec::with_capacity(max_contacts);
                for _ in 0..max_contacts {
                    let mut c_id = [0u8; 16];
                    payload.copy_to_slice(&mut c_id);
                    let mut ip = [0u8; 4];
                    payload.copy_to_slice(&mut ip);
                    let c_udp = payload.get_u16_le();
                    let c_tcp = payload.get_u16_le();
                    let c_ver = payload.get_u8();
                    contacts.push(KadContactWire {
                        kad_id: c_id,
                        ip,
                        udp_port: c_udp,
                        tcp_port: c_tcp,
                        version: c_ver,
                    });
                }

                Ok(KadMessage::BootstrapRes(KadBootstrapRes {
                    kad_id,
                    udp_port,
                    version,
                    contacts,
                }))
            }
            KADEMLIA2_REQ => {
                // Find Node Req: req_type (1 byte) + target_id (16 bytes) + sender_id (16 bytes)
                if payload.len() < 33 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadReq",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 33,
                            available: payload.len(),
                        },
                    ));
                }
                let req_type = payload.get_u8();
                let mut target_id = [0u8; 16];
                payload.copy_to_slice(&mut target_id);
                let mut sender_id = [0u8; 16];
                payload.copy_to_slice(&mut sender_id);
                Ok(KadMessage::Req(KadReq {
                    req_type,
                    target_id,
                    sender_id,
                }))
            }
            KADEMLIA2_RES => {
                // Kad2 Find Node Response (0x29)
                // May follow standard 21-byte header or 17-byte header (target_id 16B + count 1B)
                if payload.len() >= 21 {
                    let mut kad_id = [0u8; 16];
                    payload.copy_to_slice(&mut kad_id);
                    let udp_port = payload.get_u16_le();
                    let version = payload.get_u8();
                    let count = payload.get_u16_le() as usize;
                    let max_contacts = (payload.len() / 25).min(count).min(MAX_KAD_RES_CONTACTS);
                    let mut contacts = Vec::with_capacity(max_contacts);
                    for _ in 0..max_contacts {
                        let mut c_id = [0u8; 16];
                        payload.copy_to_slice(&mut c_id);
                        let mut ip = [0u8; 4];
                        payload.copy_to_slice(&mut ip);
                        let c_udp = payload.get_u16_le();
                        let c_tcp = payload.get_u16_le();
                        let c_ver = payload.get_u8();
                        contacts.push(KadContactWire {
                            kad_id: c_id,
                            ip,
                            udp_port: c_udp,
                            tcp_port: c_tcp,
                            version: c_ver,
                        });
                    }
                    Ok(KadMessage::BootstrapRes(KadBootstrapRes {
                        kad_id,
                        udp_port,
                        version,
                        contacts,
                    }))
                } else if payload.len() >= 17 {
                    let mut target_id = [0u8; 16];
                    payload.copy_to_slice(&mut target_id);
                    let count = payload.get_u8() as usize;
                    let max_contacts = (payload.len() / 25).min(count).min(MAX_KAD_RES_CONTACTS);
                    let mut contacts = Vec::with_capacity(max_contacts);
                    for _ in 0..max_contacts {
                        let mut c_id = [0u8; 16];
                        payload.copy_to_slice(&mut c_id);
                        let mut ip = [0u8; 4];
                        payload.copy_to_slice(&mut ip);
                        let c_udp = payload.get_u16_le();
                        let c_tcp = payload.get_u16_le();
                        let c_ver = payload.get_u8();
                        contacts.push(KadContactWire {
                            kad_id: c_id,
                            ip,
                            udp_port: c_udp,
                            tcp_port: c_tcp,
                            version: c_ver,
                        });
                    }
                    Ok(KadMessage::BootstrapRes(KadBootstrapRes {
                        kad_id: target_id,
                        udp_port: 0,
                        version: 2,
                        contacts,
                    }))
                } else {
                    Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadRes",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 17,
                            available: payload.len(),
                        },
                    ))
                }
            }
            KADEMLIA2_SEARCH_SOURCE_REQ => {
                // target_file_hash (16B) + start_pos (2B) + file_size (8B) + sender_id (16B) = 42B
                if payload.len() < 34 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadSearchSourceReq",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 34,
                            available: payload.len(),
                        },
                    ));
                }
                let mut target_file_hash = [0u8; 16];
                payload.copy_to_slice(&mut target_file_hash);
                let start_pos = payload.get_u16_le();
                let file_size = if payload.len() >= 8 {
                    payload.get_u64_le()
                } else {
                    0
                };
                let mut sender_id = [0u8; 16];
                if payload.len() >= 16 {
                    payload.copy_to_slice(&mut sender_id);
                }
                Ok(KadMessage::SearchSourceReq(KadSearchSourceReq {
                    target_file_hash,
                    sender_id,
                    start_pos,
                    file_size,
                }))
            }
            KADEMLIA2_SEARCH_SOURCE_RES => {
                // target_file_hash (16B) + count (2B) = 18B header
                if payload.len() < 18 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadSearchSourceRes",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 18,
                            available: payload.len(),
                        },
                    ));
                }
                let mut target_file_hash = [0u8; 16];
                payload.copy_to_slice(&mut target_file_hash);
                let count = payload.get_u16_le() as usize;

                if count > MAX_KAD_SEARCH_SOURCES {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        18,
                        "KadSearchSourceRes::sources",
                        Ed2kErrorKind::FrameTooLarge {
                            size: count,
                            limit: MAX_KAD_SEARCH_SOURCES,
                        },
                    ));
                }

                // Each source entry: kad_id (16B) + ip (4B) + udp_port (2B) + tcp_port (2B) + encryption (1B) = 25B
                let needed_bytes = count * 25;
                if payload.len() < needed_bytes {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        18,
                        "KadSearchSourceRes::sources",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: needed_bytes,
                            available: payload.len(),
                        },
                    ));
                }

                let mut sources = Vec::with_capacity(count);
                for _ in 0..count {
                    let mut kad_id = [0u8; 16];
                    payload.copy_to_slice(&mut kad_id);
                    let mut ip = [0u8; 4];
                    payload.copy_to_slice(&mut ip);
                    let udp_port = payload.get_u16_le();
                    let tcp_port = payload.get_u16_le();
                    let encryption = payload.get_u8();
                    sources.push(KadSourceWire {
                        kad_id,
                        ip,
                        udp_port,
                        tcp_port,
                        encryption,
                    });
                }

                Ok(KadMessage::SearchSourceRes(KadSearchSourceRes {
                    target_file_hash,
                    sources,
                }))
            }
            KADEMLIA2_PUBLISH_SOURCE_REQ => {
                // target_file_hash (16B) + contact (25B) + file_size (8B) = 49B
                if payload.len() < 49 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadPublishSourceReq",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 49,
                            available: payload.len(),
                        },
                    ));
                }
                let mut target_file_hash = [0u8; 16];
                payload.copy_to_slice(&mut target_file_hash);

                let mut kad_id = [0u8; 16];
                payload.copy_to_slice(&mut kad_id);
                let mut ip = [0u8; 4];
                payload.copy_to_slice(&mut ip);
                let udp_port = payload.get_u16_le();
                let tcp_port = payload.get_u16_le();
                let version = payload.get_u8();
                let file_size = payload.get_u64_le();

                let contact = KadContactWire {
                    kad_id,
                    ip,
                    udp_port,
                    tcp_port,
                    version,
                };

                Ok(KadMessage::PublishSourceReq(KadPublishSourceReq {
                    target_file_hash,
                    contact,
                    file_size,
                }))
            }
            KADEMLIA2_PUBLISH_SOURCE_RES => {
                // target_file_hash (16B) + status (1B) = 17B
                if payload.len() < 17 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadPublishSourceRes",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 17,
                            available: payload.len(),
                        },
                    ));
                }
                let mut target_file_hash = [0u8; 16];
                payload.copy_to_slice(&mut target_file_hash);
                let status = payload.get_u8();
                Ok(KadMessage::PublishSourceRes(KadPublishSourceRes {
                    target_file_hash,
                    status,
                }))
            }
            KADEMLIA2_SEARCH_KEYWORD_REQ => {
                // keyword_hash (16B) + start_pos (2B) + sender_id (16B) = 34B
                if payload.len() < 18 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadSearchKeywordReq",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 18,
                            available: payload.len(),
                        },
                    ));
                }
                let mut keyword_hash = [0u8; 16];
                payload.copy_to_slice(&mut keyword_hash);
                let start_pos = payload.get_u16_le();
                let mut sender_id = [0u8; 16];
                if payload.len() >= 16 {
                    payload.copy_to_slice(&mut sender_id);
                }
                Ok(KadMessage::SearchKeywordReq(KadSearchKeywordReq {
                    keyword_hash,
                    sender_id,
                    start_pos,
                }))
            }
            KADEMLIA2_SEARCH_KEYWORD_RES => {
                // keyword_hash (16B) + count (2B)
                if payload.len() < 18 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadSearchKeywordRes",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 18,
                            available: payload.len(),
                        },
                    ));
                }
                let mut keyword_hash = [0u8; 16];
                payload.copy_to_slice(&mut keyword_hash);
                let count = (payload.get_u16_le() as usize).min(MAX_KAD_SEARCH_KEYWORDS);

                let mut entries = Vec::with_capacity(count);
                for _ in 0..count {
                    if payload.len() < 16 + 2 + 8 + 4 {
                        break;
                    }
                    let mut file_hash = [0u8; 16];
                    payload.copy_to_slice(&mut file_hash);
                    let name_len = payload.get_u16_le() as usize;
                    if payload.len() < name_len + 8 + 4 {
                        break;
                    }
                    let name_bytes = payload.copy_to_bytes(name_len);
                    let capped_len = name_bytes.len().min(255);
                    let file_name: String = String::from_utf8_lossy(&name_bytes[..capped_len])
                        .chars()
                        .filter(|c| !c.is_control())
                        .collect();
                    let file_size = payload.get_u64_le();
                    let source_count = payload.get_u32_le();
                    entries.push(KadKeywordEntryWire {
                        file_hash,
                        file_name,
                        file_size,
                        source_count,
                    });
                }

                Ok(KadMessage::SearchKeywordRes(KadSearchKeywordRes {
                    keyword_hash,
                    entries,
                }))
            }
            KADEMLIA2_PUBLISH_KEYWORD_REQ => {
                // keyword_hash (16B) + count (2B)
                if payload.len() < 18 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadPublishKeywordReq",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 18,
                            available: payload.len(),
                        },
                    ));
                }
                let mut keyword_hash = [0u8; 16];
                payload.copy_to_slice(&mut keyword_hash);
                let count = (payload.get_u16_le() as usize).min(MAX_KAD_SEARCH_KEYWORDS);

                let mut entries = Vec::with_capacity(count);
                for _ in 0..count {
                    if payload.len() < 16 + 2 + 8 + 4 {
                        break;
                    }
                    let mut file_hash = [0u8; 16];
                    payload.copy_to_slice(&mut file_hash);
                    let name_len = payload.get_u16_le() as usize;
                    if payload.len() < name_len + 8 + 4 {
                        break;
                    }
                    let name_bytes = payload.copy_to_bytes(name_len);
                    let capped_len = name_bytes.len().min(255);
                    let file_name: String = String::from_utf8_lossy(&name_bytes[..capped_len])
                        .chars()
                        .filter(|c| !c.is_control())
                        .collect();
                    let file_size = payload.get_u64_le();
                    let source_count = payload.get_u32_le();
                    entries.push(KadKeywordEntryWire {
                        file_hash,
                        file_name,
                        file_size,
                        source_count,
                    });
                }

                Ok(KadMessage::PublishKeywordReq(KadPublishKeywordReq {
                    keyword_hash,
                    entries,
                }))
            }
            KADEMLIA2_PUBLISH_KEYWORD_RES => {
                if payload.len() < 17 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadPublishKeywordRes",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 17,
                            available: payload.len(),
                        },
                    ));
                }
                let mut keyword_hash = [0u8; 16];
                payload.copy_to_slice(&mut keyword_hash);
                let status = payload.get_u8();
                Ok(KadMessage::PublishKeywordRes(KadPublishKeywordRes {
                    keyword_hash,
                    status,
                }))
            }
            _ => Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                1,
                "KadUdpPacket::opcode",
                Ed2kErrorKind::MalformedField {
                    message: format!("unknown kad opcode 0x{opcode:02X}"),
                },
            )),
        }
    }

    /// Encodes a KadPing request packet.
    pub fn encode_ping(kad_id: &KadIdBytes, udp_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(20);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_PING);
        buf.put_slice(kad_id);
        buf.put_u16_le(udp_port);
        buf.freeze()
    }

    /// Encodes a KadPong response packet.
    pub fn encode_pong(client_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(4);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_PONG);
        buf.put_u16_le(client_port);
        buf.freeze()
    }

    /// Encodes a Kad2 Hello request packet.
    pub fn encode_hello_req(kad_id: &KadIdBytes, udp_port: u16, version: u8, tcp_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(23);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_HELLO_REQ);
        buf.put_slice(kad_id);
        buf.put_u16_le(udp_port);
        buf.put_u8(version);
        buf.put_u16_le(tcp_port);
        buf.freeze()
    }

    /// Encodes a Kad2 Hello response packet.
    pub fn encode_hello_res(kad_id: &KadIdBytes, udp_port: u16, version: u8, tcp_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(23);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_HELLO_RES);
        buf.put_slice(kad_id);
        buf.put_u16_le(udp_port);
        buf.put_u8(version);
        buf.put_u16_le(tcp_port);
        buf.freeze()
    }

    /// Encodes a Kad2 Bootstrap request packet (0x01).
    pub fn encode_bootstrap_req(kad_id: &KadIdBytes, udp_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(21);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_BOOTSTRAP_REQ);
        buf.put_slice(kad_id);
        buf.put_u16_le(udp_port);
        buf.put_u8(2);
        buf.freeze()
    }

    /// Encodes a Kad2 Bootstrap / Find Node response packet.
    pub fn encode_bootstrap_res(
        kad_id: &KadIdBytes,
        udp_port: u16,
        version: u8,
        contacts: &[KadContactWire],
    ) -> Bytes {
        let count = contacts.len().min(MAX_KAD_RES_CONTACTS);
        let mut buf = BytesMut::with_capacity(2 + 16 + 2 + 1 + 2 + count * 25);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_BOOTSTRAP_RES);
        buf.put_slice(kad_id);
        buf.put_u16_le(udp_port);
        buf.put_u8(version);
        buf.put_u16_le(count as u16);
        for c in &contacts[..count] {
            buf.put_slice(&c.kad_id);
            buf.put_slice(&c.ip);
            buf.put_u16_le(c.udp_port);
            buf.put_u16_le(c.tcp_port);
            buf.put_u8(c.version);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Find Node request packet.
    pub fn encode_find_node_req(target_id: &KadIdBytes, sender_id: &KadIdBytes) -> Bytes {
        let mut buf = BytesMut::with_capacity(35);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_REQ);
        buf.put_u8(0); // 0 = Find Node
        buf.put_slice(target_id);
        buf.put_slice(sender_id);
        buf.freeze()
    }

    /// Encodes a Kad2 Search Source request packet.
    pub fn encode_search_source_req(
        target_file_hash: &KadIdBytes,
        start_pos: u16,
        file_size: u64,
        sender_id: &KadIdBytes,
    ) -> Bytes {
        let mut buf = BytesMut::with_capacity(44);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_SEARCH_SOURCE_REQ);
        buf.put_slice(target_file_hash);
        buf.put_u16_le(start_pos);
        buf.put_u64_le(file_size);
        buf.put_slice(sender_id);
        buf.freeze()
    }

    /// Encodes a Kad2 Search Source response packet.
    pub fn encode_search_source_res(
        target_file_hash: &KadIdBytes,
        sources: &[KadSourceWire],
    ) -> Bytes {
        let count = sources.len().min(MAX_KAD_SEARCH_SOURCES);
        let mut buf = BytesMut::with_capacity(2 + 16 + 2 + count * 25);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_SEARCH_SOURCE_RES);
        buf.put_slice(target_file_hash);
        buf.put_u16_le(count as u16);
        for s in &sources[..count] {
            buf.put_slice(&s.kad_id);
            buf.put_slice(&s.ip);
            buf.put_u16_le(s.udp_port);
            buf.put_u16_le(s.tcp_port);
            buf.put_u8(s.encryption);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Publish Source request packet.
    pub fn encode_publish_source_req(
        target_file_hash: &KadIdBytes,
        contact: &KadContactWire,
        file_size: u64,
    ) -> Bytes {
        let mut buf = BytesMut::with_capacity(51);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_PUBLISH_SOURCE_REQ);
        buf.put_slice(target_file_hash);
        buf.put_slice(&contact.kad_id);
        buf.put_slice(&contact.ip);
        buf.put_u16_le(contact.udp_port);
        buf.put_u16_le(contact.tcp_port);
        buf.put_u8(contact.version);
        buf.put_u64_le(file_size);
        buf.freeze()
    }

    /// Encodes a Kad2 Publish Source response packet.
    pub fn encode_publish_source_res(target_file_hash: &KadIdBytes, status: u8) -> Bytes {
        let mut buf = BytesMut::with_capacity(19);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_PUBLISH_SOURCE_RES);
        buf.put_slice(target_file_hash);
        buf.put_u8(status);
        buf.freeze()
    }

    /// Encodes a Kad2 Search Keyword request packet.
    pub fn encode_search_keyword_req(
        keyword_hash: &KadIdBytes,
        start_pos: u16,
        sender_id: &KadIdBytes,
    ) -> Bytes {
        let mut buf = BytesMut::with_capacity(36);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_SEARCH_KEYWORD_REQ);
        buf.put_slice(keyword_hash);
        buf.put_u16_le(start_pos);
        buf.put_slice(sender_id);
        buf.freeze()
    }

    /// Encodes a Kad2 Search Keyword response packet.
    pub fn encode_search_keyword_res(
        keyword_hash: &KadIdBytes,
        entries: &[KadKeywordEntryWire],
    ) -> Bytes {
        let count = entries.len().min(MAX_KAD_SEARCH_KEYWORDS);
        let mut buf = BytesMut::with_capacity(2 + 16 + 2 + count * 64);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_SEARCH_KEYWORD_RES);
        buf.put_slice(keyword_hash);
        buf.put_u16_le(count as u16);
        for entry in &entries[..count] {
            buf.put_slice(&entry.file_hash);
            let name_bytes = entry.file_name.as_bytes();
            let name_len = name_bytes.len().min(255) as u16;
            buf.put_u16_le(name_len);
            buf.put_slice(&name_bytes[..name_len as usize]);
            buf.put_u64_le(entry.file_size);
            buf.put_u32_le(entry.source_count);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Publish Keyword request packet.
    pub fn encode_publish_keyword_req(
        keyword_hash: &KadIdBytes,
        entries: &[KadKeywordEntryWire],
    ) -> Bytes {
        let count = entries.len().min(MAX_KAD_SEARCH_KEYWORDS);
        let mut buf = BytesMut::with_capacity(2 + 16 + 2 + count * 64);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_PUBLISH_KEYWORD_REQ);
        buf.put_slice(keyword_hash);
        buf.put_u16_le(count as u16);
        for entry in &entries[..count] {
            buf.put_slice(&entry.file_hash);
            let name_bytes = entry.file_name.as_bytes();
            let name_len = name_bytes.len().min(255) as u16;
            buf.put_u16_le(name_len);
            buf.put_slice(&name_bytes[..name_len as usize]);
            buf.put_u64_le(entry.file_size);
            buf.put_u32_le(entry.source_count);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Publish Keyword response packet.
    pub fn encode_publish_keyword_res(keyword_hash: &KadIdBytes, status: u8) -> Bytes {
        let mut buf = BytesMut::with_capacity(19);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_PUBLISH_KEYWORD_RES);
        buf.put_slice(keyword_hash);
        buf.put_u8(status);
        buf.freeze()
    }
}
