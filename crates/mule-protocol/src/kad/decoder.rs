// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Bounded decoding for Kad UDP wire datagrams.

use crate::ed2k::{Ed2kErrorKind, Ed2kParseError};

use super::opcodes::*;
use super::types::*;

pub mod routing;
pub mod search;

pub use routing::{
    decode_bootstrap_req, decode_bootstrap_res, decode_firewalled2_req, decode_firewalled_req,
    decode_firewalled_res, decode_hello_req, decode_hello_res, decode_hello_res_ack, decode_ping,
    decode_pong, decode_req, decode_res,
};
pub use search::{
    decode_note_tags_blob, decode_publish_keyword_req, decode_publish_keyword_res,
    decode_publish_notes_req, decode_publish_source_req, decode_publish_source_res,
    decode_search_keyword_req, decode_search_keyword_res_legacy, decode_search_notes_req,
    decode_search_notes_res, decode_search_res, decode_search_source_req,
};

/// Bounded decoder for Kad UDP wire datagrams.
pub struct KadDecoder;

impl KadDecoder {
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
        let payload = if protocol == PR_KADEMLIA_PACKED
            || (raw_payload.len() >= 2
                && raw_payload[0] == 0x78
                && (raw_payload[1] == 0x9C
                    || raw_payload[1] == 0xDA
                    || raw_payload[1] == 0x01
                    || raw_payload[1] == 0x5E))
        {
            use flate2::read::ZlibDecoder;
            use std::io::Read;
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
            KADEMLIA2_PING => decode_ping(protocol, opcode, payload),
            KADEMLIA2_PONG => decode_pong(protocol, opcode, payload),
            KADEMLIA2_HELLO_REQ | KADEMLIA_HELLO_REQ => decode_hello_req(protocol, opcode, payload),
            KADEMLIA2_HELLO_RES | KADEMLIA_HELLO_RES => decode_hello_res(protocol, opcode, payload),
            // 0x22 = HELLO_RES_ACK (Kad >= 8 contact verification reply)
            KADEMLIA2_HELLO_RES_ACK => decode_hello_res_ack(protocol, opcode, payload),
            // 0x50 = FIREWALLED_REQ: [tcp_port u16]
            KADEMLIA_FIREWALLED_REQ => decode_firewalled_req(protocol, opcode, payload),
            // 0x53 = FIREWALLED2_REQ: [tcp_port u16][user_hash 16][connect_options u8]
            KADEMLIA_FIREWALLED2_REQ => decode_firewalled2_req(protocol, opcode, payload),
            // 0x58 = FIREWALLED_RES: [ip u32]
            KADEMLIA_FIREWALLED_RES => decode_firewalled_res(protocol, opcode, payload),
            // 0x59 = FIREWALLED_ACK_RES: empty payload (deprecated since Kad 7,
            // replaced by TCP OP_KAD_FWTCPCHECK_ACK)
            KADEMLIA_FIREWALLED_ACK_RES => {
                if !payload.is_empty() {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadFirewalledAckRes",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 0,
                            available: payload.len(),
                        },
                    ));
                }
                Ok(KadMessage::FirewalledAckRes)
            }
            KADEMLIA_BOOTSTRAP_REQ_OLD | KADEMLIA2_BOOTSTRAP_REQ => {
                decode_bootstrap_req(protocol, opcode, payload)
            }
            KADEMLIA2_BOOTSTRAP_RES => decode_bootstrap_res(protocol, opcode, payload),
            KADEMLIA2_REQ => decode_req(protocol, opcode, payload),
            KADEMLIA2_RES => decode_res(protocol, opcode, payload),
            // 0x33 = SEARCH_KEY_REQ = SEARCH_KEYWORD_REQ (aliased)
            KADEMLIA2_SEARCH_KEY_REQ => decode_search_keyword_req(protocol, opcode, payload),
            // 0x34 = SEARCH_SOURCE_REQ
            KADEMLIA2_SEARCH_SOURCE_REQ => decode_search_source_req(protocol, opcode, payload),
            // 0x35 = SEARCH_NOTES_REQ (Kad2)
            KADEMLIA2_SEARCH_NOTES_REQ => decode_search_notes_req(protocol, opcode, payload),
            // 0x3A = SEARCH_NOTES_RES (legacy Kad1 form)
            KADEMLIA_SEARCH_NOTES_RES => decode_search_notes_res(protocol, opcode, payload),
            // 0x3B = SEARCH_RES (unified for keyword, source, and notes search)
            KADEMLIA2_SEARCH_RES => decode_search_res(protocol, opcode, payload),
            // 0x43 = PUBLISH_KEY_REQ = PUBLISH_KEYWORD_REQ (aliased)
            KADEMLIA2_PUBLISH_KEY_REQ => decode_publish_keyword_req(protocol, opcode, payload),
            // 0x44 = PUBLISH_SOURCE_REQ
            KADEMLIA2_PUBLISH_SOURCE_REQ => decode_publish_source_req(protocol, opcode, payload),
            // 0x45 = PUBLISH_NOTES_REQ
            KADEMLIA2_PUBLISH_NOTES_REQ => decode_publish_notes_req(protocol, opcode, payload),
            // 0x4B = PUBLISH_RES (unified for keyword, source, and notes publish)
            KADEMLIA2_PUBLISH_RES => decode_publish_source_res(protocol, opcode, payload),
            // 0x2C = legacy search keyword response (internal tests)
            KADEMLIA2_SEARCH_KEYWORD_RES_LEGACY => {
                decode_search_keyword_res_legacy(protocol, opcode, payload)
            }
            // 0x62 = FIREWALLUDP: [u8 error_code][u16 tested_port]
            KADEMLIA2_FIREWALLUDP => {
                if payload.len() < 3 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadFirewallUdp",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 3,
                            available: payload.len(),
                        },
                    ));
                }
                Ok(KadMessage::FirewallUdp(KadFirewallUdp {
                    error_code: payload[0],
                    udp_port: u16::from_le_bytes([payload[1], payload[2]]),
                }))
            }
            // 0x51 = FINDBUDDY_REQ: [buddy_id:16][user_id:16][tcp_port:2]
            KADEMLIA_FINDBUDDY_REQ => {
                if payload.len() < 34 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadFindBuddyReq",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 34,
                            available: payload.len(),
                        },
                    ));
                }
                let mut buddy_id = [0u8; 16];
                buddy_id.copy_from_slice(&payload[0..16]);
                let mut user_id = [0u8; 16];
                user_id.copy_from_slice(&payload[16..32]);
                Ok(KadMessage::FindBuddyReq(KadFindBuddyReq {
                    buddy_id,
                    user_id,
                    tcp_port: u16::from_le_bytes([payload[32], payload[33]]),
                }))
            }
            // 0x52 = CALLBACK_REQ: [buddy_id:16][file_id:16][tcp_port:2]
            KADEMLIA_CALLBACK_REQ => {
                if payload.len() < 34 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadCallbackReq",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 34,
                            available: payload.len(),
                        },
                    ));
                }
                let mut buddy_id = [0u8; 16];
                buddy_id.copy_from_slice(&payload[0..16]);
                let mut file_id = [0u8; 16];
                file_id.copy_from_slice(&payload[16..32]);
                Ok(KadMessage::CallbackReq(KadCallbackReq {
                    buddy_id,
                    file_id,
                    tcp_port: u16::from_le_bytes([payload[32], payload[33]]),
                }))
            }
            // 0x5A = FINDBUDDY_RES: [buddy_id:16][client_id:16][tcp_port:2][opts:1]
            KADEMLIA_FINDBUDDY_RES => {
                if payload.len() < 34 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadFindBuddyRes",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 34,
                            available: payload.len(),
                        },
                    ));
                }
                let mut buddy_id = [0u8; 16];
                buddy_id.copy_from_slice(&payload[0..16]);
                let mut client_id = [0u8; 16];
                client_id.copy_from_slice(&payload[16..32]);
                Ok(KadMessage::FindBuddyRes(KadFindBuddyRes {
                    buddy_id,
                    client_id,
                    tcp_port: u16::from_le_bytes([payload[32], payload[33]]),
                    connect_options: payload.get(34).copied().unwrap_or(0),
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
}
