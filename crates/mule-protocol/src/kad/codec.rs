//! Bounded encoding and decoding for Kad UDP packets (Ping, Pong, Hello, HelloAnswer, Bootstrap, Req/Res).

pub use super::decoder::KadDecoder;
pub use super::encoder::KadEncoder;
pub use super::opcodes::*;
pub use super::types::*;

use crate::ed2k::Ed2kParseError;
use bytes::Bytes;

/// Unified codec facade maintaining full backward compatibility.
pub struct KadCodec;

impl KadCodec {
    /// Decodes a raw UDP packet into a KadMessage.
    #[inline]
    pub fn decode(raw: &[u8]) -> Result<KadMessage, Ed2kParseError> {
        KadDecoder::decode(raw)
    }

    /// Encodes a KadPing request packet.
    #[inline]
    pub fn encode_ping(kad_id: &KadIdBytes, udp_port: u16) -> Bytes {
        KadEncoder::encode_ping(kad_id, udp_port)
    }

    /// Encodes a KadPong response packet.
    #[inline]
    pub fn encode_pong(client_port: u16) -> Bytes {
        KadEncoder::encode_pong(client_port)
    }

    /// Encodes a Kad2 Hello request packet.
    #[inline]
    pub fn encode_hello_req(
        kad_id: &KadIdBytes,
        udp_port: u16,
        version: u8,
        tcp_port: u16,
    ) -> Bytes {
        KadEncoder::encode_hello_req(kad_id, udp_port, version, tcp_port)
    }

    /// Encodes a Kad2 Hello response packet.
    #[inline]
    pub fn encode_hello_res(
        kad_id: &KadIdBytes,
        udp_port: u16,
        version: u8,
        tcp_port: u16,
    ) -> Bytes {
        KadEncoder::encode_hello_res(kad_id, udp_port, version, tcp_port)
    }

    /// Encodes a Kad2 Bootstrap request packet (0x01).
    #[inline]
    pub fn encode_bootstrap_req(kad_id: &KadIdBytes, udp_port: u16) -> Bytes {
        KadEncoder::encode_bootstrap_req(kad_id, udp_port)
    }

    /// Encodes a Kad2 Bootstrap response packet (0x09) with a list of contacts.
    #[inline]
    pub fn encode_bootstrap_res(
        bootstrap_id: &KadIdBytes,
        port: u16,
        version: u8,
        contacts: &[KadContactWire],
    ) -> Bytes {
        KadEncoder::encode_bootstrap_res(bootstrap_id, port, version, contacts)
    }

    /// Encodes a Kad2 Find Node request packet.
    #[inline]
    pub fn encode_find_node_req(target_id: &KadIdBytes, receiver_id: &KadIdBytes) -> Bytes {
        KadEncoder::encode_find_node_req(target_id, receiver_id)
    }

    /// Encodes a Kad2 Find Node response packet (0x29).
    #[inline]
    pub fn encode_find_node_res(target_id: &KadIdBytes, contacts: &[KadContactWire]) -> Bytes {
        KadEncoder::encode_find_node_res(target_id, contacts)
    }

    /// Encodes a Kad2 Search Source request packet (0x34).
    /// Official format: [target_file_hash 16] [start_pos u16] [file_size u64]
    #[inline]
    pub fn encode_search_source_req(
        target_file_hash: &KadIdBytes,
        start_pos: u16,
        file_size: u64,
    ) -> Bytes {
        KadEncoder::encode_search_source_req(target_file_hash, start_pos, file_size)
    }

    /// Encodes a Kad2 Search Source response packet (0x3B, unified search response).
    #[inline]
    pub fn encode_search_source_res(
        target_file_hash: &KadIdBytes,
        sources: &[KadSourceWire],
    ) -> Bytes {
        KadEncoder::encode_search_source_res(target_file_hash, sources)
    }

    /// Encodes a Kad2 Publish Source request packet (0x44).
    #[inline]
    pub fn encode_publish_source_req(
        target_file_hash: &KadIdBytes,
        contact: &KadContactWire,
        file_size: u64,
    ) -> Bytes {
        KadEncoder::encode_publish_source_req(target_file_hash, contact, file_size)
    }

    /// Encodes a Kad2 Publish Source response packet (0x4B, unified publish response).
    #[inline]
    pub fn encode_publish_source_res(target_file_hash: &KadIdBytes, status: u8) -> Bytes {
        KadEncoder::encode_publish_source_res(target_file_hash, status)
    }

    /// Encodes a Kad2 Search Keyword request packet (0x33).
    #[inline]
    pub fn encode_search_keyword_req(
        keyword_hash: &KadIdBytes,
        start_pos: u16,
        sender_id: &KadIdBytes,
    ) -> Bytes {
        KadEncoder::encode_search_keyword_req(keyword_hash, start_pos, sender_id)
    }

    /// Encodes a Kad2 Search Keyword response packet (0x3B, unified search response).
    #[inline]
    pub fn encode_search_keyword_res(
        keyword_hash: &KadIdBytes,
        entries: &[KadKeywordEntryWire],
    ) -> Bytes {
        KadEncoder::encode_search_keyword_res(keyword_hash, entries)
    }

    /// Encodes a Kad2 Publish Keyword request packet (0x43).
    #[inline]
    pub fn encode_publish_keyword_req(
        keyword_hash: &KadIdBytes,
        entries: &[KadKeywordEntryWire],
    ) -> Bytes {
        KadEncoder::encode_publish_keyword_req(keyword_hash, entries)
    }

    /// Encodes a Kad2 Publish Keyword response packet (0x4B, unified publish response).
    #[inline]
    pub fn encode_publish_keyword_res(keyword_hash: &KadIdBytes, status: u8) -> Bytes {
        KadEncoder::encode_publish_keyword_res(keyword_hash, status)
    }

    /// Encodes a KADEMLIA2_FIREWALLUDP (0x62) test packet.
    #[inline]
    pub fn encode_firewall_udp(error_code: u8, udp_port: u16) -> Bytes {
        KadEncoder::encode_firewall_udp(error_code, udp_port)
    }

    /// Encodes a KADEMLIA_FINDBUDDY_RES (0x5A) packet.
    #[inline]
    pub fn encode_findbuddy_res(
        buddy_id: &KadIdBytes,
        client_id: &KadIdBytes,
        tcp_port: u16,
        connect_options: u8,
    ) -> Bytes {
        KadEncoder::encode_findbuddy_res(buddy_id, client_id, tcp_port, connect_options)
    }

    /// Encodes a KADEMLIA_CALLBACK_REQ (0x52) packet.
    #[inline]
    pub fn encode_callback_req(
        buddy_id: &KadIdBytes,
        file_id: &KadIdBytes,
        tcp_port: u16,
    ) -> Bytes {
        KadEncoder::encode_callback_req(buddy_id, file_id, tcp_port)
    }
}
