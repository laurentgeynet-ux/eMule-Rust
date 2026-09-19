//! Kad UDP wire packet encoder.

use bytes::{BufMut, Bytes, BytesMut};

use super::opcodes::*;
use super::types::*;

/// Bounded wire packet encoder for Kad UDP packets.
pub struct KadEncoder;

impl KadEncoder {
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

    /// Encodes a KADEMLIA2_FIREWALLUDP (0x62) test packet:
    /// [u8 error_code][u16 tested_udp_port] — official eMule format
    /// (`ProcessFirewallCheckUDPRequest` in BaseClient.cpp).
    pub fn encode_firewall_udp(error_code: u8, udp_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(5);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_FIREWALLUDP);
        buf.put_u8(error_code);
        buf.put_u16_le(udp_port);
        buf.freeze()
    }

    /// Encodes a Kad2 Hello request packet.
    pub fn encode_hello_req(
        kad_id: &KadIdBytes,
        udp_port: u16,
        version: u8,
        tcp_port: u16,
    ) -> Bytes {
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
    pub fn encode_hello_res(
        kad_id: &KadIdBytes,
        udp_port: u16,
        version: u8,
        tcp_port: u16,
    ) -> Bytes {
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
    pub fn encode_bootstrap_req(_kad_id: &KadIdBytes, _udp_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(2);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_BOOTSTRAP_REQ);
        buf.freeze()
    }

    /// Encodes a Kad2 Bootstrap response packet (0x09).
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
            buf.put_u8(c.ip[3]);
            buf.put_u8(c.ip[2]);
            buf.put_u8(c.ip[1]);
            buf.put_u8(c.ip[0]);
            buf.put_u16_le(c.udp_port);
            buf.put_u16_le(c.tcp_port);
            buf.put_u8(c.version);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Find Node response packet (0x29, KADEMLIA2_RES).
    /// Official format: [target_id: 16] [count: u8] followed by count * 25-byte contact records.
    pub fn encode_find_node_res(target_id: &KadIdBytes, contacts: &[KadContactWire]) -> Bytes {
        let count = contacts.len().min(MAX_KAD_RES_CONTACTS);
        let mut buf = BytesMut::with_capacity(2 + 16 + 1 + count * 25);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_RES);
        buf.put_slice(target_id);
        buf.put_u8(count as u8);
        for c in &contacts[..count] {
            buf.put_slice(&c.kad_id);
            buf.put_u8(c.ip[3]);
            buf.put_u8(c.ip[2]);
            buf.put_u8(c.ip[1]);
            buf.put_u8(c.ip[0]);
            buf.put_u16_le(c.udp_port);
            buf.put_u16_le(c.tcp_port);
            buf.put_u8(c.version);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Find Node request packet.
    pub fn encode_find_node_req(target_id: &KadIdBytes, receiver_id: &KadIdBytes) -> Bytes {
        let mut buf = BytesMut::with_capacity(35);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_REQ);
        buf.put_u8(KADEMLIA_FIND_NODE);
        buf.put_slice(target_id);
        buf.put_slice(receiver_id);
        buf.freeze()
    }

    /// Encodes a Kad2 Search Source request packet (0x34).
    /// Official format: [target_file_hash 16] [start_pos u16] [file_size u64]
    pub fn encode_search_source_req(
        target_file_hash: &KadIdBytes,
        start_pos: u16,
        file_size: u64,
    ) -> Bytes {
        let mut buf = BytesMut::with_capacity(28);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_SEARCH_SOURCE_REQ);
        buf.put_slice(target_file_hash);
        buf.put_u16_le(start_pos);
        buf.put_u64_le(file_size);
        buf.freeze()
    }

    /// Encodes a Kad2 Search Source response packet (0x3B, unified search response).
    pub fn encode_search_source_res(
        target_file_hash: &KadIdBytes,
        sources: &[KadSourceWire],
    ) -> Bytes {
        let count = sources.len().min(MAX_KAD_SEARCH_SOURCES);
        let mut buf = BytesMut::with_capacity(2 + 34 + count * 64);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_SEARCH_RES);
        buf.put_slice(&[0u8; 16]); // sender Kad ID
        buf.put_slice(target_file_hash);
        buf.put_u16_le(count as u16);
        for s in &sources[..count] {
            buf.put_slice(&s.kad_id);
            buf.put_u8(5); // 5 tags

            // 1. TAG_SOURCETYPE (0xFF) -> TAGTYPE_UINT8 (0x09)
            buf.put_u8(0x09);
            buf.put_u16_le(1);
            buf.put_u8(0xFF);
            buf.put_u8(1);

            // 2. TAG_SOURCEIP (0xFE) -> TAGTYPE_UINT32 (0x03)
            buf.put_u8(0x03);
            buf.put_u16_le(1);
            buf.put_u8(0xFE);
            buf.put_u32_le(u32::from_be_bytes(s.ip));

            // 3. TAG_SOURCEPORT (0xFD) -> TAGTYPE_UINT16 (0x08)
            buf.put_u8(0x08);
            buf.put_u16_le(1);
            buf.put_u8(0xFD);
            buf.put_u16_le(s.tcp_port);

            // 4. TAG_SOURCEUPORT (0xFC) -> TAGTYPE_UINT16 (0x08)
            buf.put_u8(0x08);
            buf.put_u16_le(1);
            buf.put_u8(0xFC);
            buf.put_u16_le(s.udp_port);

            // 5. TAG_ENCRYPTION (0xF3) -> TAGTYPE_UINT8 (0x09)
            buf.put_u8(0x09);
            buf.put_u16_le(1);
            buf.put_u8(0xF3);
            buf.put_u8(s.encryption);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Publish Source request packet (0x44).
    pub fn encode_publish_source_req(
        target_file_hash: &KadIdBytes,
        contact: &KadContactWire,
        file_size: u64,
    ) -> Bytes {
        let mut buf = BytesMut::with_capacity(2 + 32 + 1 + 5 * 10);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_PUBLISH_SOURCE_REQ);
        buf.put_slice(target_file_hash);
        buf.put_slice(&contact.kad_id);
        buf.put_u8(5); // 5 tags

        // 1. TAG_SOURCETYPE (0xFF) -> TAGTYPE_UINT8 (0x09)
        buf.put_u8(0x09);
        buf.put_u16_le(1);
        buf.put_u8(0xFF);
        buf.put_u8(if file_size > 0xFFFFFFFF { 4 } else { 1 });

        // 2. TAG_SOURCEPORT (0xFD) -> TAGTYPE_UINT16 (0x08)
        buf.put_u8(0x08);
        buf.put_u16_le(1);
        buf.put_u8(0xFD);
        buf.put_u16_le(contact.tcp_port);

        // 3. TAG_SOURCEUPORT (0xFC) -> TAGTYPE_UINT16 (0x08)
        buf.put_u8(0x08);
        buf.put_u16_le(1);
        buf.put_u8(0xFC);
        buf.put_u16_le(contact.udp_port);

        // 4. TAG_FILESIZE (0x02) -> TAGTYPE_UINT64 (0x0B)
        buf.put_u8(0x0B);
        buf.put_u16_le(1);
        buf.put_u8(0x02);
        buf.put_u64_le(file_size);

        // 5. TAG_ENCRYPTION (0xF3) -> TAGTYPE_UINT8 (0x09)
        buf.put_u8(0x09);
        buf.put_u16_le(1);
        buf.put_u8(0xF3);
        buf.put_u8(1);

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

    /// Encodes a Kad2 Search Keyword request packet (0x33).
    /// Official format: [keyword_hash 16] [start_pos u16]
    pub fn encode_search_keyword_req(
        keyword_hash: &KadIdBytes,
        start_pos: u16,
        _sender_id: &KadIdBytes,
    ) -> Bytes {
        let mut buf = BytesMut::with_capacity(20);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_SEARCH_KEYWORD_REQ);
        buf.put_slice(keyword_hash);
        buf.put_u16_le(start_pos);
        buf.freeze()
    }

    /// Encodes a Kad2 Search Keyword response packet with TagList.
    pub fn encode_search_keyword_res(
        keyword_hash: &KadIdBytes,
        entries: &[KadKeywordEntryWire],
    ) -> Bytes {
        let count = entries.len().min(MAX_KAD_SEARCH_KEYWORDS);
        let mut buf = BytesMut::with_capacity(2 + 34 + count * 64);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_SEARCH_RES);
        buf.put_slice(&[0u8; 16]);
        buf.put_slice(keyword_hash);
        buf.put_u16_le(count as u16);
        for entry in &entries[..count] {
            buf.put_slice(&entry.file_hash);
            buf.put_u8(3);

            let name_bytes = entry.file_name.as_bytes();
            let name_len = name_bytes.len().min(255) as u16;
            // TAG_FILENAME (0x01) -> TAGTYPE_STRING (0x02)
            buf.put_u8(0x02);
            buf.put_u16_le(1);
            buf.put_u8(0x01);
            buf.put_u16_le(name_len);
            buf.put_slice(&name_bytes[..name_len as usize]);

            // TAG_FILESIZE (0x02) -> TAGTYPE_UINT64 (0x0B)
            buf.put_u8(0x0B);
            buf.put_u16_le(1);
            buf.put_u8(0x02);
            buf.put_u64_le(entry.file_size);

            // TAG_SOURCES (0x15) -> TAGTYPE_UINT32 (0x03)
            buf.put_u8(0x03);
            buf.put_u16_le(1);
            buf.put_u8(0x15);
            buf.put_u32_le(entry.source_count);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Publish Keyword request packet (0x43).
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
            buf.put_u8(3);

            let name_bytes = entry.file_name.as_bytes();
            let name_len = name_bytes.len().min(255) as u16;
            // TAG_FILENAME (0x01) -> TAGTYPE_STRING (0x02)
            buf.put_u8(0x02);
            buf.put_u16_le(1);
            buf.put_u8(0x01);
            buf.put_u16_le(name_len);
            buf.put_slice(&name_bytes[..name_len as usize]);

            // TAG_FILESIZE (0x02) -> TAGTYPE_UINT64 (0x0B)
            buf.put_u8(0x0B);
            buf.put_u16_le(1);
            buf.put_u8(0x02);
            buf.put_u64_le(entry.file_size);

            // TAG_SOURCES (0x15) -> TAGTYPE_UINT32 (0x03)
            buf.put_u8(0x03);
            buf.put_u16_le(1);
            buf.put_u8(0x15);
            buf.put_u32_le(entry.source_count);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Publish Keyword response packet (0x4B, unified publish response).
    pub fn encode_publish_keyword_res(keyword_hash: &KadIdBytes, status: u8) -> Bytes {
        let mut buf = BytesMut::with_capacity(19);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_PUBLISH_KEYWORD_RES);
        buf.put_slice(keyword_hash);
        buf.put_u8(status);
        buf.freeze()
    }
}
