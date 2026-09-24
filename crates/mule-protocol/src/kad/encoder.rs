// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Kad UDP wire packet encoder.

use bytes::{BufMut, Bytes, BytesMut};

use crate::ed2k::encoder::tags::encode_tag;
use crate::ed2k::limits::Ed2kLimits;

use super::opcodes::*;
use super::types::*;

/// Bounded wire packet encoder for Kad UDP packets.
pub struct KadEncoder;

/// Allocates a buffer pre-filled with the `[PR_KADEMLIA][opcode]` header —
/// shared frame of every Kad UDP encoder below.
fn kad_buf(opcode: u8, capacity: usize) -> BytesMut {
    let mut buf = BytesMut::with_capacity(capacity);
    buf.put_u8(PR_KADEMLIA);
    buf.put_u8(opcode);
    buf
}

impl KadEncoder {
    /// Encodes a KadPing request packet.
    pub fn encode_ping(kad_id: &KadIdBytes, udp_port: u16) -> Bytes {
        let mut buf = kad_buf(KADEMLIA2_PING, 20);
        buf.put_slice(kad_id);
        buf.put_u16_le(udp_port);
        buf.freeze()
    }

    /// Encodes a KadPong response packet.
    pub fn encode_pong(client_port: u16) -> Bytes {
        let mut buf = kad_buf(KADEMLIA2_PONG, 4);
        buf.put_u16_le(client_port);
        buf.freeze()
    }

    /// Encodes a KADEMLIA2_FIREWALLUDP (0x62) test packet:
    /// [u8 error_code][u16 tested_udp_port] — official eMule format
    /// (`ProcessFirewallCheckUDPRequest` in BaseClient.cpp).
    pub fn encode_firewall_udp(error_code: u8, udp_port: u16) -> Bytes {
        let mut buf = kad_buf(KADEMLIA2_FIREWALLUDP, 5);
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
        let mut buf = kad_buf(KADEMLIA_FINDBUDDY_RES, 37);
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
        let mut buf = kad_buf(KADEMLIA_CALLBACK_REQ, 36);
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
        let mut buf = kad_buf(opcode, 22 + tag_count as usize * 6);
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
        let mut buf = kad_buf(KADEMLIA2_HELLO_RES_ACK, 19);
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
        let mut buf = kad_buf(KADEMLIA_FIREWALLED2_REQ, 21);
        buf.put_u16_le(tcp_port);
        buf.put_slice(user_hash);
        buf.put_u8(connect_options);
        buf.freeze()
    }

    /// Encodes a `KADEMLIA_FIREWALLED_RES` (0x58): `[ip u32]` echoing the
    /// requester's public IPv4 address.
    pub fn encode_firewalled_res(ip: u32) -> Bytes {
        let mut buf = kad_buf(KADEMLIA_FIREWALLED_RES, 6);
        buf.put_u32_le(ip);
        buf.freeze()
    }

    /// Encodes a Kad2 Bootstrap request packet (0x01).
    pub fn encode_bootstrap_req(_kad_id: &KadIdBytes, _udp_port: u16) -> Bytes {
        kad_buf(KADEMLIA2_BOOTSTRAP_REQ, 2).freeze()
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
        let mut buf = kad_buf(KADEMLIA2_BOOTSTRAP_RES, 2 + 16 + 2 + 1 + 2 + count * 25);
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
        let mut buf = kad_buf(KADEMLIA2_RES, 2 + 16 + 1 + count * 25);
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
        let mut buf = kad_buf(KADEMLIA2_REQ, 35);
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
        let mut buf = kad_buf(KADEMLIA2_SEARCH_SOURCE_REQ, 28);
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
        let mut buf = kad_buf(KADEMLIA2_SEARCH_RES, 2 + 34 + count * 64);
        buf.put_slice(&[0u8; 16]); // sender Kad ID
        buf.put_slice(target_file_hash);
        buf.put_u16_le(count as u16);
        for s in &sources[..count] {
            write_source_entry(&mut buf, s);
        }
        buf.freeze()
    }

    /// eMule `CIndexed::SendValidSourceResult` (`Indexed.cpp:688`) : packs up
    /// to `MAX_KAD_SEARCH_SOURCES` entries greedily into consecutive
    /// `KADEMLIA2_SEARCH_RES` datagrams, each staying under
    /// `UDP_KAD_MAXFRAGMENT` — IP fragmentation would drop Kad answers.
    /// Returns at least one packet (possibly with 0 entries when `sources`
    /// is empty).
    pub fn encode_search_source_res_fragmented(
        target_file_hash: &KadIdBytes,
        sources: &[KadSourceWire],
    ) -> Vec<Bytes> {
        const HEADER: usize = 2 + 16 + 16 + 2;
        let mut packets = Vec::new();
        let mut group: Vec<&KadSourceWire> = Vec::new();
        let mut used = HEADER;
        for s in sources.iter().take(MAX_KAD_SEARCH_SOURCES) {
            let entry_len = source_entry_wire_len(s);
            if used + entry_len > UDP_KAD_MAXFRAGMENT && !group.is_empty() {
                packets.push(encode_search_source_res_refs(target_file_hash, &group));
                group.clear();
                used = HEADER;
            }
            group.push(s);
            used += entry_len;
        }
        if !group.is_empty() || packets.is_empty() {
            packets.push(encode_search_source_res_refs(target_file_hash, &group));
        }
        packets
    }

    /// Encodes a Kad2 Publish Source request packet (0x44).
    pub fn encode_publish_source_req(
        target_file_hash: &KadIdBytes,
        contact: &KadContactWire,
        file_size: u64,
    ) -> Bytes {
        let mut buf = kad_buf(KADEMLIA2_PUBLISH_SOURCE_REQ, 2 + 32 + 1 + 5 * 10);
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
        let mut buf = kad_buf(KADEMLIA2_PUBLISH_SOURCE_RES, 19);
        buf.put_slice(target_file_hash);
        buf.put_u8(status);
        buf.freeze()
    }

    /// Encodes a Kad2 Search Keyword request packet (0x33).
    /// Official format: `[keyword_hash 16][start_pos u16]` — `start_pos`
    /// carries the `0x8000` restrictive-search flag when set; an expression
    /// tree may follow (not produced by this client, matching eMule's
    /// requester which only sets the flag).
    pub fn encode_search_keyword_req(keyword_hash: &KadIdBytes, start_pos: u16) -> Bytes {
        let mut buf = kad_buf(KADEMLIA2_SEARCH_KEYWORD_REQ, 20);
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
        let mut buf = kad_buf(KADEMLIA2_SEARCH_RES, 2 + 34 + count * 64);
        buf.put_slice(&[0u8; 16]);
        buf.put_slice(keyword_hash);
        buf.put_u16_le(count as u16);
        for entry in &entries[..count] {
            write_kw_entry(&mut buf, entry);
        }
        buf.freeze()
    }

    /// eMule `CIndexed::SendValidKeywordResult` (`Indexed.cpp:601`) : same
    /// `UDP_KAD_MAXFRAGMENT` greedy packing as
    /// [`encode_search_source_res_fragmented`], over at most
    /// `MAX_KAD_SEARCH_KEYWORDS` entries.
    ///
    /// [`encode_search_source_res_fragmented`]: Self::encode_search_source_res_fragmented
    pub fn encode_search_keyword_res_fragmented(
        keyword_hash: &KadIdBytes,
        entries: &[KadKeywordEntryWire],
    ) -> Vec<Bytes> {
        const HEADER: usize = 2 + 16 + 16 + 2;
        let mut packets = Vec::new();
        let mut group: Vec<&KadKeywordEntryWire> = Vec::new();
        let mut used = HEADER;
        for e in entries.iter().take(MAX_KAD_SEARCH_KEYWORDS) {
            let entry_len = kw_entry_wire_len(e);
            if used + entry_len > UDP_KAD_MAXFRAGMENT && !group.is_empty() {
                packets.push(encode_search_keyword_res_refs(keyword_hash, &group));
                group.clear();
                used = HEADER;
            }
            group.push(e);
            used += entry_len;
        }
        if !group.is_empty() || packets.is_empty() {
            packets.push(encode_search_keyword_res_refs(keyword_hash, &group));
        }
        packets
    }

    /// Encodes a Kad2 Publish Keyword request packet (0x43).
    pub fn encode_publish_keyword_req(
        keyword_hash: &KadIdBytes,
        entries: &[KadKeywordEntryWire],
    ) -> Bytes {
        let count = entries.len().min(MAX_KAD_SEARCH_KEYWORDS);
        let mut buf = kad_buf(KADEMLIA2_PUBLISH_KEYWORD_REQ, 2 + 16 + 2 + count * 64);
        buf.put_slice(keyword_hash);
        buf.put_u16_le(count as u16);
        for entry in &entries[..count] {
            write_kw_entry(&mut buf, entry);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Publish Keyword response packet (0x4B, unified publish response).
    pub fn encode_publish_keyword_res(keyword_hash: &KadIdBytes, status: u8) -> Bytes {
        let mut buf = kad_buf(KADEMLIA2_PUBLISH_KEYWORD_RES, 19);
        buf.put_slice(keyword_hash);
        buf.put_u8(status);
        buf.freeze()
    }
}

// --- Search-result entry writers shared by single-packet and fragmented encoders ---

/// Exact wire size of one `KADEMLIA2_SEARCH_RES` source entry (kad_id +
/// sourcetype + tag list + optional buddy hash).
fn source_entry_wire_len(s: &KadSourceWire) -> usize {
    16 + 1 + 5 + 8 + 6 + 6 + 5 + usize::from(s.buddy_id.is_some()) * (4 + 32)
}

/// Writes one source entry of a `KADEMLIA2_SEARCH_RES` (0x3B) packet.
fn write_source_entry(buf: &mut BytesMut, s: &KadSourceWire) {
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

/// Encodes a `KADEMLIA2_SEARCH_RES` source response over borrowed entries.
fn encode_search_source_res_refs(target_file_hash: &KadIdBytes, sources: &[&KadSourceWire]) -> Bytes {
    let count = sources.len().min(MAX_KAD_SEARCH_SOURCES);
    let mut buf = kad_buf(KADEMLIA2_SEARCH_RES, 2 + 34 + count * 64);
    buf.put_slice(&[0u8; 16]); // sender Kad ID
    buf.put_slice(target_file_hash);
    buf.put_u16_le(count as u16);
    for s in &sources[..count] {
        write_source_entry(&mut buf, s);
    }
    buf.freeze()
}

/// Exact wire size of one keyword entry — serializes the tag list into a
/// scratch buffer (stored tags + node-computed AICH/publish tags) instead
/// of estimating, so the fragmented packet packer stays exact.
fn kw_entry_wire_len(e: &KadKeywordEntryWire) -> usize {
    let mut tmp = BytesMut::new();
    write_kw_entry(&mut tmp, e);
    tmp.len()
}

/// Writes a `TAGTYPE_BSOB` Kad tag: `[0x0A][u16 name_len][name][u8 len][data]`
/// (eMule `CDataIO::WriteTag`/`WriteBsob`).
fn write_bsob_tag(dst: &mut Vec<u8>, name: u8, data: &[u8]) {
    debug_assert!(data.len() <= u8::MAX as usize);
    dst.push(crate::ed2k::tags::TAGTYPE_BSOB);
    dst.extend_from_slice(&1u16.to_le_bytes());
    dst.push(name);
    dst.push(data.len() as u8);
    dst.extend_from_slice(data);
}

/// Writes `TAG_FILENAME` (0x01) and `TAG_FILESIZE` (0x02) from the entry
/// fields — eMule `WriteTagListInc` emits them before `m_listTag`
/// (filesize is skipped when zero).
fn write_kw_field_tags(tmp: &mut Vec<u8>, entry: &KadKeywordEntryWire) -> usize {
    let mut n = 0;
    let name_bytes = entry.file_name.as_bytes();
    let name_len = name_bytes.len().min(255) as u16;
    if name_len > 0 {
        tmp.push(0x02); // TAGTYPE_STRING
        tmp.extend_from_slice(&1u16.to_le_bytes());
        tmp.push(0x01);
        tmp.extend_from_slice(&name_len.to_le_bytes());
        tmp.extend_from_slice(&name_bytes[..name_len as usize]);
        n += 1;
    }
    if entry.file_size > 0 {
        tmp.push(0x0B); // TAGTYPE_UINT64
        tmp.extend_from_slice(&1u16.to_le_bytes());
        tmp.push(0x02);
        tmp.extend_from_slice(&entry.file_size.to_le_bytes());
        n += 1;
    }
    n
}

/// Writes the synthesized minimal tag list (filename/filesize/sources)
/// used for flat-format entries and as fallback when the stored tag list
/// fails to re-encode.
fn write_kw_synth_tags(tmp: &mut Vec<u8>, entry: &KadKeywordEntryWire) {
    write_kw_field_tags(tmp, entry);
    // TAG_SOURCES (0x15) -> TAGTYPE_UINT32 (0x03)
    tmp.push(0x03);
    tmp.extend_from_slice(&1u16.to_le_bytes());
    tmp.push(0x15);
    tmp.extend_from_slice(&entry.source_count.to_le_bytes());
}

/// Writes one keyword entry of a `KADEMLIA2_SEARCH_RES` (0x3B) or
/// `KADEMLIA2_PUBLISH_KEYWORD_REQ` (0x43) packet.
///
/// Tag list layout (eMule `CEntry::WriteTagListInc` /
/// `CKeyEntry::WriteTagListWithPublishInfo`):
/// 1. `TAG_FILENAME` + `TAG_FILESIZE` from the entry fields (the stored
///    tag list — `m_listTag` — never carries them),
/// 2. the stored publisher tag list verbatim (`TAG_SOURCES` synthesized
///    for flat-format entries),
/// 3. `TAG_KADAICHHASHPUB` (0x36) when publishing our own AICH root,
/// 4. `TAG_PUBLISHINFO` (0x33) then `TAG_KADAICHHASHRESULT` (0x37) when
///    serving stored entries — node-computed, never part of `entry.tags`.
fn write_kw_entry(buf: &mut BytesMut, entry: &KadKeywordEntryWire) {
    buf.put_slice(&entry.file_hash);
    let limits = Ed2kLimits::default();
    let mut tmp = Vec::with_capacity(128);
    let mut n = write_kw_field_tags(&mut tmp, entry);
    // Leave headroom for the node-computed tags below (u8 count field).
    let stored = entry.tags.len().min(u8::MAX as usize - 3);
    if stored > 0 {
        for tag in &entry.tags[..stored] {
            if encode_tag(&limits, KADEMLIA2_SEARCH_RES, tag, &mut tmp).is_err() {
                tmp.clear();
                n = 0;
                break;
            }
            n += 1;
        }
    }
    if n == 0 {
        tmp.clear();
        write_kw_synth_tags(&mut tmp, entry);
        n = 3;
    } else if stored == 0 {
        // Flat-format entry: synthesize the TAG_SOURCES tag so the
        // historical wire shape is preserved.
        tmp.push(0x03); // TAGTYPE_UINT32
        tmp.extend_from_slice(&1u16.to_le_bytes());
        tmp.push(0x15);
        tmp.extend_from_slice(&entry.source_count.to_le_bytes());
        n += 1;
    }
    // `TAG_KADAICHHASHPUB` (0x36) — publish path only.
    if let Some(root) = &entry.aich_pub {
        write_bsob_tag(&mut tmp, TAG_KADAICHHASH_PUB, root);
        n += 1;
    }
    // `TAG_PUBLISHINFO` (0x33) — u32, eMule `CKadTagUInt` always lands on
    // TAGTYPE_UINT32 here since the names byte is always non-zero.
    if let Some(info) = entry.publish_info {
        tmp.push(crate::ed2k::tags::TAGTYPE_UINT32);
        tmp.extend_from_slice(&1u16.to_le_bytes());
        tmp.push(TAG_PUBLISHINFO);
        tmp.extend_from_slice(&info.to_le_bytes());
        n += 1;
    }
    // `TAG_KADAICHHASHRESULT` (0x37) — BSOB `<u8 count>{<u8 pop><20B root>}`,
    // capped to `MAX_AICH_RESULT_VOTES` (eMule 250-byte payload bound).
    if !entry.aich_votes.is_empty() {
        let votes: Vec<&AichHashVote> = entry
            .aich_votes
            .iter()
            .filter(|v| v.popularity > 0)
            .take(MAX_AICH_RESULT_VOTES)
            .collect();
        if !votes.is_empty() {
            let mut bsob = Vec::with_capacity(1 + 21 * votes.len());
            bsob.push(votes.len() as u8);
            for v in votes {
                bsob.push(v.popularity);
                bsob.extend_from_slice(&v.root);
            }
            write_bsob_tag(&mut tmp, TAG_KADAICHHASH_RESULT, &bsob);
            n += 1;
        }
    }
    buf.put_u8(n as u8);
    buf.put_slice(&tmp);
}

/// Encodes a `KADEMLIA2_SEARCH_RES` keyword response over borrowed entries.
fn encode_search_keyword_res_refs(
    keyword_hash: &KadIdBytes,
    entries: &[&KadKeywordEntryWire],
) -> Bytes {
    let count = entries.len().min(MAX_KAD_SEARCH_KEYWORDS);
    let mut buf = kad_buf(KADEMLIA2_SEARCH_RES, 2 + 34 + count * 64);
    buf.put_slice(&[0u8; 16]);
    buf.put_slice(keyword_hash);
    buf.put_u16_le(count as u16);
    for entry in &entries[..count] {
        write_kw_entry(&mut buf, entry);
    }
    buf.freeze()
}

// ---------------------------------------------------------------------------
// Kad notes (file comments / ratings)
// ---------------------------------------------------------------------------

/// Encodes a `KADEMLIA2_SEARCH_NOTES_REQ` (0x35):
/// `[target_file_hash:16][file_size:u64]`.
pub fn encode_search_notes_req_payload(target_file_hash: &KadIdBytes, file_size: u64) -> Bytes {
    let mut buf = kad_buf(KADEMLIA2_SEARCH_NOTES_REQ, 26);
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
    let mut buf = kad_buf(KADEMLIA2_SEARCH_RES, 2 + 34 + count * 96);
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
    let mut buf = kad_buf(KADEMLIA2_PUBLISH_NOTES_REQ, 2 + 33 + tags.len() * 32);
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
    let mut buf = kad_buf(KADEMLIA2_PUBLISH_RES, 19);
    buf.put_slice(target_file_hash);
    buf.put_u8(load);
    buf.freeze()
}
