//! Bounded decoding for Kad UDP wire datagrams.

use crate::ed2k::{Ed2kErrorKind, Ed2kParseError};

use super::opcodes::*;
use super::types::*;

pub mod routing;
pub mod search;

pub use routing::{
    decode_bootstrap_req, decode_bootstrap_res, decode_hello_req, decode_hello_res, decode_ping,
    decode_pong, decode_req, decode_res,
};
pub use search::{
    decode_publish_keyword_req, decode_publish_keyword_res, decode_publish_source_req,
    decode_publish_source_res, decode_search_keyword_req, decode_search_keyword_res_legacy,
    decode_search_res, decode_search_source_req,
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
            KADEMLIA2_PING => decode_ping(protocol, opcode, payload),
            KADEMLIA2_PONG => decode_pong(protocol, opcode, payload),
            KADEMLIA2_HELLO_REQ | KADEMLIA_HELLO_REQ => decode_hello_req(protocol, opcode, payload),
            KADEMLIA2_HELLO_RES | KADEMLIA_HELLO_RES => decode_hello_res(protocol, opcode, payload),
            KADEMLIA_BOOTSTRAP_REQ_OLD | KADEMLIA2_BOOTSTRAP_REQ => decode_bootstrap_req(protocol, opcode, payload),
            KADEMLIA2_BOOTSTRAP_RES => decode_bootstrap_res(protocol, opcode, payload),
            KADEMLIA2_REQ => decode_req(protocol, opcode, payload),
            KADEMLIA2_RES => decode_res(protocol, opcode, payload),
            // 0x33 = SEARCH_KEY_REQ = SEARCH_KEYWORD_REQ (aliased)
            KADEMLIA2_SEARCH_KEY_REQ => decode_search_keyword_req(protocol, opcode, payload),
            // 0x34 = SEARCH_SOURCE_REQ
            KADEMLIA2_SEARCH_SOURCE_REQ => decode_search_source_req(protocol, opcode, payload),
            // 0x3B = SEARCH_RES (unified for keyword, source, and notes search)
            KADEMLIA2_SEARCH_RES => decode_search_res(protocol, opcode, payload),
            // 0x43 = PUBLISH_KEY_REQ = PUBLISH_KEYWORD_REQ (aliased)
            KADEMLIA2_PUBLISH_KEY_REQ => decode_publish_keyword_req(protocol, opcode, payload),
            // 0x44 = PUBLISH_SOURCE_REQ
            KADEMLIA2_PUBLISH_SOURCE_REQ => decode_publish_source_req(protocol, opcode, payload),
            // 0x4B = PUBLISH_RES (unified for keyword, source, and notes publish)
            KADEMLIA2_PUBLISH_RES => decode_publish_source_res(protocol, opcode, payload),
            // 0x2C = legacy search keyword response (internal tests)
            KADEMLIA2_SEARCH_KEYWORD_RES_LEGACY => decode_search_keyword_res_legacy(protocol, opcode, payload),
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
