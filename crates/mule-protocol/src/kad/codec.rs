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
    ///
    /// `misc_options`: eMule `KADMISCOPTIONS` byte (bit0 = UDP firewalled,
    /// bit1 = TCP firewalled, bit2 = request HELLO_RES_ACK) — sent only for
    /// Kad >= 8, pass `None` to omit.
    #[inline]
    pub fn encode_hello_req(
        kad_id: &KadIdBytes,
        udp_port: u16,
        version: u8,
        tcp_port: u16,
        misc_options: Option<u8>,
    ) -> Bytes {
        KadEncoder::encode_hello_req(kad_id, udp_port, version, tcp_port, misc_options)
    }

    /// Encodes a Kad2 Hello response packet (same layout as the request).
    #[inline]
    pub fn encode_hello_res(
        kad_id: &KadIdBytes,
        udp_port: u16,
        version: u8,
        tcp_port: u16,
        misc_options: Option<u8>,
    ) -> Bytes {
        KadEncoder::encode_hello_res(kad_id, udp_port, version, tcp_port, misc_options)
    }

    /// Encodes a `KADEMLIA2_HELLO_RES_ACK` (0x22) — `[kadID 16][tag_count u8][tags]`.
    #[inline]
    pub fn encode_hello_res_ack(kad_id: &KadIdBytes) -> Bytes {
        KadEncoder::encode_hello_res_ack(kad_id)
    }

    /// Encodes a `KADEMLIA_FIREWALLED_RES` (0x58) — `[ip u32 LE]`.
    #[inline]
    pub fn encode_firewalled_res(ip: u32) -> Bytes {
        KadEncoder::encode_firewalled_res(ip)
    }

    /// Encodes a Kad2 Bootstrap request packet (0x01).
    #[inline]
    pub fn encode_bootstrap_req(kad_id: &KadIdBytes, udp_port: u16) -> Bytes {
        KadEncoder::encode_bootstrap_req(kad_id, udp_port)
    }

    /// Encodes a Kad2 Bootstrap response packet (0x09) with a list of contacts.
    ///
    /// `tcp_port` is the responder's eD2K **TCP** port (eMule
    /// `thePrefs.GetPort()` in the response header).
    #[inline]
    pub fn encode_bootstrap_res(
        bootstrap_id: &KadIdBytes,
        tcp_port: u16,
        version: u8,
        contacts: &[KadContactWire],
    ) -> Bytes {
        KadEncoder::encode_bootstrap_res(bootstrap_id, tcp_port, version, contacts)
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

    /// Encodes a KADEMLIA2_SEARCH_NOTES_REQ (0x35) packet.
    #[inline]
    pub fn encode_search_notes_req(target_file_hash: &KadIdBytes, file_size: u64) -> Bytes {
        super::encoder::encode_search_notes_req_payload(target_file_hash, file_size)
    }

    /// Encodes a notes search response (unified 0x3B form).
    #[inline]
    pub fn encode_search_notes_res(
        sender_id: &KadIdBytes,
        target_file_hash: &KadIdBytes,
        notes: &[KadNoteWire],
    ) -> Option<Bytes> {
        super::encoder::encode_search_notes_res(sender_id, target_file_hash, notes)
    }

    /// Encodes a KADEMLIA2_PUBLISH_NOTES_REQ (0x45) packet.
    #[inline]
    pub fn encode_publish_notes_req(
        target_file_hash: &KadIdBytes,
        source_id: &KadIdBytes,
        tags: &[crate::ed2k::tags::Ed2kTag],
    ) -> Option<Bytes> {
        super::encoder::encode_publish_notes_req(target_file_hash, source_id, tags)
    }

    /// Encodes a KADEMLIA2_PUBLISH_RES (0x4B) notes response.
    #[inline]
    pub fn encode_publish_notes_res(target_file_hash: &KadIdBytes, load: u8) -> Bytes {
        super::encoder::encode_publish_notes_res(target_file_hash, load)
    }

    /// Re-decodes a raw datagram payload as a notes search response — used
    /// when the pending transaction context says the `0x3B` reply belongs to
    /// a notes search (the generic decoder cannot distinguish it from a
    /// keyword response by content alone).
    #[inline]
    pub fn decode_search_notes_res(
        protocol: u8,
        opcode: u8,
        payload: &[u8],
    ) -> Result<KadMessage, Ed2kParseError> {
        super::decoder::decode_search_notes_res(protocol, opcode, payload)
    }
}
