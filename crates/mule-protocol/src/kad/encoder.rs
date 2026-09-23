// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

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

    /// Encodes a KADEMLIA_FINDBUDDY_RES (0x5A) acceptance packet:
    /// [buddy_id:16][client_id:16][tcp_port:2][connect_opts:1] — official
    /// eMule format (`Process_KADEMLIA_FINDBUDDY_REQ` response).
    pub fn encode_findbuddy_res(
        buddy_id: &KadIdBytes,
        client_id: &KadIdBytes,
        tcp_port: u16,
        connect_options: u8,
    ) -> Bytes {
        let mut buf = BytesMut::with_capacity(37);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA_FINDBUDDY_RES);
        buf.put_slice(buddy_id);
        buf.put_slice(client_id);
        buf.put_u16_le(tcp_port);
        buf.put_u8(connect_options);
        buf.freeze()
    }

    /// Encodes a KADEMLIA_CALLBACK_REQ (0x52) relay request:
    /// [buddy_id:16][file_id:16][tcp_port:2] — sent unencrypted to the
    /// buddy's Kad UDP port (eMule `TryToConnect` CCS_KADCALLBACK).
    pub fn encode_callback_req(
        buddy_id: &KadIdBytes,
        file_id: &KadIdBytes,
        tcp_port: u16,
    ) -> Bytes {
        let mut buf = BytesMut::with_capacity(36);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA_CALLBACK_REQ);
        buf.put_slice(buddy_id);
        buf.put_slice(file_id);
        buf.put_u16_le(tcp_port);
        buf.freeze()
    }

    /// Encodes a Kad2 Hello request packet.
    /// Shared encoder for `KADEMLIA2_HELLO_REQ`/`RES` (eMule `SendMyDetails`).
    fn encode_hello(
        opcode: u8,
        kad_id: &KadIdBytes,
        udp_port: u16,
        version: u8,
        tcp_port: u16,
        misc_options: Option<u8>,
    ) -> Bytes {
        let tag_count = 1 + u8::from(misc_options.is_some());
        let mut buf = BytesMut::with_capacity(22 + tag_count as usize * 6);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(opcode);
        buf.put_slice(kad_id);
        buf.put_u16_le(tcp_port);
        buf.put_u8(version);
        buf.put_u8(tag_count);

        // TAG_SOURCEUPORT (0xFC) -> TAGTYPE_UINT16 (0x08), old tag format:
        // [type][u16 name_len=1][name byte][value]
        buf.put_u8(0x08);
        buf.put_u16_le(1);
        buf.put_u8(TAG_SOURCEUPORT);
        buf.put_u16_le(udp_port);

        if let Some(misc) = misc_options {
            // TAG_KADMISCOPTIONS (0xF2) -> TAGTYPE_UINT8 (0x09)
            buf.put_u8(0x09);
            buf.put_u16_le(1);
            buf.put_u8(TAG_KADMISCOPTIONS);
            buf.put_u8(misc);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Hello request packet.
    ///
    /// eMule layout (`SendMyDetails`): `[kadID 16][tcp_port u16][version u8]
    /// [tag_count u8][tags]` — the UDP port travels inside the
    /// `TAG_SOURCEUPORT` (0xFC, u16) tag, `TAG_KADMISCOPTIONS` (0xF2, u8)
    /// carries firewall/ack-request bits for Kad >= 8.
    pub fn encode_hello_req(
        kad_id: &KadIdBytes,
        udp_port: u16,
        version: u8,
        tcp_port: u16,
        misc_options: Option<u8>,
    ) -> Bytes {
        Self::encode_hello(
            KADEMLIA2_HELLO_REQ,
            kad_id,
            udp_port,
            version,
            tcp_port,
            misc_options,
        )
    }

    /// Encodes a Kad2 Hello response packet (same layout as the request).
    pub fn encode_hello_res(
        kad_id: &KadIdBytes,
        udp_port: u16,
        version: u8,
        tcp_port: u16,
        misc_options: Option<u8>,
    ) -> Bytes {
        Self::encode_hello(
            KADEMLIA2_HELLO_RES,
            kad_id,
            udp_port,
            version,
            tcp_port,
            misc_options,
        )
    }

    /// Encodes a `KADEMLIA2_HELLO_RES_ACK` (0x22): `[kadID 16][tag_count u8]`.
    ///
    /// Sent after a HELLO_RES that requested it via `TAG_KADMISCOPTIONS` bit 2,
    /// proving our IP is not spoofed (eMule Process_KADEMLIA2_HELLO_RES).
    pub fn encode_hello_res_ack(kad_id: &KadIdBytes) -> Bytes {
        let mut buf = BytesMut::with_capacity(19);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_HELLO_RES_ACK);
        buf.put_slice(kad_id);
        buf.put_u8(0); // no tags
        buf.freeze()
    }

    /// Encodes a `KADEMLIA_FIREWALLED2_REQ` (0x53):
    /// `[tcp_port u16][user_hash 16][connect_options u8]` — 19 bytes.
    pub fn encode_firewalled2_req(
        tcp_port: u16,
        user_hash: &[u8; 16],
        connect_options: u8,
    ) -> Bytes {
        let mut buf = BytesMut::with_capacity(21);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA_FIREWALLED2_REQ);
        buf.put_u16_le(tcp_port);
        buf.put_slice(user_hash);
        buf.put_u8(connect_options);
        buf.freeze()
    }

    /// Encodes a `KADEMLIA_FIREWALLED_RES` (0x58): `[ip u32]` echoing the
    /// requester's public IPv4 address.
    pub fn encode_firewalled_res(ip: u32) -> Bytes {
        let mut buf = BytesMut::with_capacity(6);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA_FIREWALLED_RES);
        buf.put_u32_le(ip);
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
    ///
    /// eMule `Process_KADEMLIA2_BOOTSTRAP_REQ`: the header port field is the
    /// responder's **TCP** port (`thePrefs.GetPort()`), not the UDP port.
    pub fn encode_bootstrap_res(
        kad_id: &KadIdBytes,
        tcp_port: u16,
        version: u8,
        contacts: &[KadContactWire],
    ) -> Bytes {
        let count = contacts.len().min(MAX_KAD_RES_CONTACTS);
        let mut buf = BytesMut::with_capacity(2 + 16 + 2 + 1 + 2 + count * 25);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_BOOTSTRAP_RES);
        buf.put_slice(kad_id);
        buf.put_u16_le(tcp_port);
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
            buf.put_u8(if s.buddy_id.is_some() { 6 } else { 5 });

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

            // 6. TAG_BUDDYHASH (0xF8) -> TAGTYPE_STRING (0x02), 32-char hex
            //    (eMule CKadTagStr(TAG_BUDDYHASH, md4str(buddyID))) — only on
            //    LowID sources reachable through a Kad buddy.
            if let Some(buddy) = &s.buddy_id {
                let hex_str: String = buddy.iter().map(|b| format!("{b:02X}")).collect();
                buf.put_u8(0x02);
                buf.put_u16_le(1);
                buf.put_u8(0xF8);
                buf.put_u16_le(hex_str.len() as u16);
                buf.put_slice(hex_str.as_bytes());
            }
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

// ---------------------------------------------------------------------------
// Kad notes (file comments / ratings)
// ---------------------------------------------------------------------------

/// Encodes a `KADEMLIA2_SEARCH_NOTES_REQ` (0x35):
/// `[target_file_hash:16][file_size:u64]`.
pub fn encode_search_notes_req_payload(target_file_hash: &KadIdBytes, file_size: u64) -> Bytes {
    let mut buf = BytesMut::with_capacity(26);
    buf.put_u8(PR_KADEMLIA);
    buf.put_u8(KADEMLIA2_SEARCH_NOTES_REQ);
    buf.put_slice(target_file_hash);
    buf.put_u64_le(file_size);
    buf.freeze()
}

/// Encodes a notes search response on the unified `KADEMLIA2_SEARCH_RES`
/// (0x3B) form — the exact wire shape eMule `CIndexed::SendValidNoteResult`
/// emits: `[sender_id:16][target:16][count:u16]` then per entry
/// `[source_id:16][tag_count:u8][tags]`.
///
/// Returns `None` when any tag fails the bounded encoder (oversized strings
/// etc.) — the caller should simply not answer.
pub fn encode_search_notes_res(
    sender_id: &KadIdBytes,
    target_file_hash: &KadIdBytes,
    notes: &[KadNoteWire],
) -> Option<Bytes> {
    let limits = crate::ed2k::limits::Ed2kLimits::default();
    let count = notes.len().min(MAX_NOTE_SEARCH_RESULTS);
    let mut buf = BytesMut::with_capacity(2 + 34 + count * 96);
    buf.put_u8(PR_KADEMLIA);
    buf.put_u8(KADEMLIA2_SEARCH_RES);
    buf.put_slice(sender_id);
    buf.put_slice(target_file_hash);
    buf.put_u16_le(count as u16);
    for n in &notes[..count] {
        let tag_total = n.tags.len() + n.unknown_tags.len();
        if tag_total > MAX_NOTE_TAG_COUNT {
            return None;
        }
        buf.put_slice(&n.source_id);
        buf.put_u8(tag_total as u8);
        let mut tmp = Vec::new();
        for t in &n.tags {
            crate::ed2k::encoder::encode_tag(&limits, KADEMLIA2_SEARCH_RES, t, &mut tmp).ok()?;
        }
        for t in &n.unknown_tags {
            crate::ed2k::encoder::encode_raw_tag(&limits, KADEMLIA2_SEARCH_RES, t, &mut tmp)
                .ok()?;
        }
        buf.put_slice(&tmp);
    }
    Some(buf.freeze())
}

/// Encodes a `KADEMLIA2_PUBLISH_NOTES_REQ` (0x45):
/// `[target_file_hash:16][source_id:16][tag_count:u8][tags]`.
pub fn encode_publish_notes_req(
    target_file_hash: &KadIdBytes,
    source_id: &KadIdBytes,
    tags: &[crate::ed2k::tags::Ed2kTag],
) -> Option<Bytes> {
    if tags.len() > MAX_NOTE_TAG_COUNT {
        return None;
    }
    let limits = crate::ed2k::limits::Ed2kLimits::default();
    let mut buf = BytesMut::with_capacity(2 + 33 + tags.len() * 32);
    buf.put_u8(PR_KADEMLIA);
    buf.put_u8(KADEMLIA2_PUBLISH_NOTES_REQ);
    buf.put_slice(target_file_hash);
    buf.put_slice(source_id);
    buf.put_u8(tags.len() as u8);
    let mut tmp = Vec::new();
    for t in tags {
        crate::ed2k::encoder::encode_tag(&limits, KADEMLIA2_PUBLISH_NOTES_REQ, t, &mut tmp).ok()?;
    }
    buf.put_slice(&tmp);
    Some(buf.freeze())
}

/// Encodes a note tag list as a standalone blob (`[tag_count:u8][tags]`) for
/// persistence; `decode_note_tags_blob` reverses it.
pub fn encode_note_tags_blob(
    tags: &[crate::ed2k::tags::Ed2kTag],
    unknown_tags: &[crate::ed2k::tags::RawTag],
) -> Option<Vec<u8>> {
    let total = tags.len() + unknown_tags.len();
    if total > MAX_NOTE_TAG_COUNT {
        return None;
    }
    let limits = crate::ed2k::limits::Ed2kLimits::default();
    let mut out = Vec::with_capacity(1 + total * 32);
    out.push(total as u8);
    for t in tags {
        crate::ed2k::encoder::encode_tag(&limits, KADEMLIA2_PUBLISH_NOTES_REQ, t, &mut out).ok()?;
    }
    for t in unknown_tags {
        crate::ed2k::encoder::encode_raw_tag(&limits, KADEMLIA2_PUBLISH_NOTES_REQ, t, &mut out)
            .ok()?;
    }
    Some(out)
}

/// Encodes a `KADEMLIA2_PUBLISH_RES` (0x4B) for notes:
/// `[target_file_hash:16][load:u8]`.
pub fn encode_publish_notes_res(target_file_hash: &KadIdBytes, load: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(19);
    buf.put_u8(PR_KADEMLIA);
    buf.put_u8(KADEMLIA2_PUBLISH_RES);
    buf.put_slice(target_file_hash);
    buf.put_u8(load);
    buf.freeze()
}
