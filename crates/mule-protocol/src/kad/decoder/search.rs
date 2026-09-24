// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Decoders for Kad sources and search responses.

use crate::ed2k::limits::Ed2kLimits;
use crate::ed2k::tags::{
    Ed2kTag, TagName, TagReader, TagValue, TAG_SPECIAL_FILESIZE, TAG_SPECIAL_NAME, TAG_SPECIAL_PORT,
};
use crate::ed2k::{Ed2kErrorKind, Ed2kParseError};
use crate::kad::opcodes::*;
use crate::kad::types::*;
use bytes::Buf;

/// Node-computed tags extracted from a keyword entry tag list — never
/// part of the stored publisher tag list (`CEntry::m_listTag`).
#[derive(Default)]
struct KwExtractedTags {
    /// `TAG_KADAICHHASHPUB` (0x36) — publisher's AICH root (publish only).
    aich_pub: Option<[u8; AICH_ROOT_HASH_SIZE]>,
    /// `TAG_KADAICHHASHRESULT` (0x37) — AICH popularity histogram.
    aich_votes: Vec<AichHashVote>,
    /// `TAG_PUBLISHINFO` (0x33) — `<names u8><publishers u8><trust u16>`.
    publish_info: Option<u32>,
}

/// Parses a `TAG_KADAICHHASHRESULT` BSOB payload —
/// `<u8 count>{<u8 popularity><20B root>}` — with strict length validation
/// (eMule drops the whole vote array on malformed payloads,
/// `ProcessResultKeyword`). Zero-popularity entries are skipped.
fn parse_aich_result_bsob(data: &[u8]) -> Vec<AichHashVote> {
    let Some(&count) = data.first() else {
        return Vec::new();
    };
    let count = count as usize;
    if data.len() != 1 + count * (1 + AICH_ROOT_HASH_SIZE) {
        return Vec::new();
    }
    data[1..]
        .as_chunks::<{ 1 + AICH_ROOT_HASH_SIZE }>()
        .0
        .iter()
        .filter_map(|c| {
            let popularity = c[0];
            if popularity == 0 {
                return None;
            }
            let mut root = [0u8; AICH_ROOT_HASH_SIZE];
            root.copy_from_slice(&c[1..]);
            Some(AichHashVote { popularity, root })
        })
        .collect()
}

/// Partitions a decoded keyword-entry tag list: publisher metadata tags
/// stay in the returned list (`CEntry::m_listTag`); consumed field tags —
/// `TAG_FILENAME` (0x01) and `TAG_FILESIZE` (0x02), intercepted by eMule
/// `KademliaUDPListener` and re-emitted from entry fields at serve
/// time — plus node-computed tags `TAG_KADAICHHASHPUB` (0x36, first
/// valid BSOB-20 wins), `TAG_KADAICHHASHRESULT` (0x37) and
/// `TAG_PUBLISHINFO` (0x33) are extracted (`CEntry::AddTag` deletes the
/// two result-only tags on publish).
fn split_kw_wire_tags(tags: Vec<Ed2kTag>) -> (Vec<Ed2kTag>, KwExtractedTags) {
    let mut out = Vec::with_capacity(tags.len());
    let mut ext = KwExtractedTags::default();
    for tag in tags {
        match tag.name {
            // Filename and filesize are consumed into entry fields —
            // eMule never stores them in `m_listTag`.
            TagName::Id(TAG_SPECIAL_NAME) | TagName::Id(TAG_SPECIAL_FILESIZE) => {}
            TagName::Id(TAG_KADAICHHASH_PUB) => {
                if ext.aich_pub.is_none() {
                    if let TagValue::Blob(b) = &tag.value {
                        if b.len() == AICH_ROOT_HASH_SIZE {
                            let mut root = [0u8; AICH_ROOT_HASH_SIZE];
                            root.copy_from_slice(b);
                            ext.aich_pub = Some(root);
                        }
                    }
                }
            }
            TagName::Id(TAG_KADAICHHASH_RESULT) => {
                if ext.aich_votes.is_empty() {
                    if let TagValue::Blob(b) = &tag.value {
                        ext.aich_votes = parse_aich_result_bsob(b);
                    }
                }
            }
            TagName::Id(TAG_PUBLISHINFO) => {
                if ext.publish_info.is_none() {
                    if let TagValue::UInt32(v) = tag.value {
                        ext.publish_info = Some(v);
                    }
                }
            }
            _ => out.push(tag),
        }
    }
    (out, ext)
}

/// Decodes an `OP_KADEMLIA2_SEARCH_SOURCE_REQ` payload.
/// Official eMule format: [target_hash 16] [start_pos u16] [file_size u64] (26 bytes).
pub fn decode_search_source_req(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 18 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadSearchSourceReq",
            Ed2kErrorKind::UnexpectedEof {
                needed: 18,
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

/// Decodes an `OP_KADEMLIA2_SEARCH_RES` payload.
pub fn decode_search_res(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    let raw_bytes = payload;
    let limits = Ed2kLimits::default();

    let mut taglist_result: Option<KadMessage> = None;
    if raw_bytes.len() >= 34 {
        let mut _sender_id = [0u8; 16];
        _sender_id.copy_from_slice(&raw_bytes[0..16]);
        let mut target_hash = [0u8; 16];
        target_hash.copy_from_slice(&raw_bytes[16..32]);
        let count = u16::from_le_bytes([raw_bytes[32], raw_bytes[33]]) as usize;

        if count <= MAX_KAD_SEARCH_KEYWORDS {
            let mut cursor = 34;
            let mut keyword_entries = Vec::with_capacity(count);
            let mut source_entries = Vec::with_capacity(count);
            let mut is_source_res = false;
            let mut parse_ok = true;

            for _ in 0..count {
                if cursor + 17 > raw_bytes.len() {
                    parse_ok = false;
                    break;
                }
                let mut answer_hash = [0u8; 16];
                answer_hash.copy_from_slice(&raw_bytes[cursor..cursor + 16]);
                cursor += 16;
                let tag_count = raw_bytes[cursor];
                cursor += 1;

                let mut file_name = String::new();
                let mut file_size = 0u64;
                let mut source_count = 0u32;
                let mut ip = [0u8; 4];
                let mut udp_port = 0u16;
                let mut tcp_port = 0u16;
                let mut encryption = 0u8;
                let mut buddy_id: Option<KadIdBytes> = None;
                let mut entry_is_source = false;
                let mut parsed_tags: Vec<Ed2kTag> = Vec::new();

                let mut reader = TagReader::new(raw_bytes, cursor, protocol, Some(opcode));
                for _ in 0..tag_count {
                    match reader.read_tag(&limits) {
                        Ok((Some(tag), _)) => {
                            parsed_tags.push(tag.clone());
                            match &tag.name {
                                TagName::Id(TAG_SPECIAL_NAME) => {
                                    if let TagValue::String(s) = tag.value {
                                        file_name = s.chars().filter(|c| !c.is_control()).collect();
                                    }
                                }
                                TagName::Id(TAG_SPECIAL_FILESIZE) => match tag.value {
                                    TagValue::UInt32(v) => file_size = v as u64,
                                    TagValue::UInt64(v) => file_size = v,
                                    TagValue::UInt16(v) => file_size = v as u64,
                                    TagValue::UInt8(v) => file_size = v as u64,
                                    _ => {}
                                },
                                TagName::Id(0x15) => {
                                    // FT_SOURCES (0x15)
                                    match tag.value {
                                        TagValue::UInt32(v) => source_count = v,
                                        TagValue::UInt16(v) => source_count = v as u32,
                                        TagValue::UInt8(v) => source_count = v as u32,
                                        _ => {}
                                    }
                                }
                                TagName::Id(TAG_PUBLISHINFO) => {
                                    // FT_PUBLISHINFO (0x33): (uNames << 24) | (uPublishers << 16) | uTrust
                                    // publishers_known is (v >> 16) & 0xFF — extracted
                                    // out of the tag list by `split_kw_wire_tags` below.
                                    if let TagValue::UInt32(v) = tag.value {
                                        let pubs = (v >> 16) & 0xFF;
                                        if pubs > 0 && source_count == 0 {
                                            source_count = pubs;
                                        }
                                    }
                                }
                                TagName::Id(0xFF) => {
                                    // FT_SOURCETYPE (0xFF)
                                    entry_is_source = true;
                                }
                                TagName::Id(0xFD) | TagName::Id(TAG_SPECIAL_PORT) => {
                                    // FT_SOURCEPORT (0xFD)
                                    entry_is_source = true;
                                    match tag.value {
                                        TagValue::UInt16(p) => tcp_port = p,
                                        TagValue::UInt32(p) => tcp_port = p as u16,
                                        _ => {}
                                    }
                                }
                                TagName::Id(0xFE) => {
                                    // FT_SOURCEIP (0xFE): eMule writes uIP in host order via WriteUInt32 (LE).
                                    // Reading it back as u32 LE yields the host-order IP integer, so to_be_bytes()
                                    // gives the correct IPv4 octets [a, b, c, d].
                                    entry_is_source = true;
                                    if let TagValue::UInt32(ip_val) = tag.value {
                                        ip = ip_val.to_be_bytes();
                                    }
                                }
                                TagName::Id(0xFC) => {
                                    // FT_SOURCEUPORT (0xFC)
                                    entry_is_source = true;
                                    match tag.value {
                                        TagValue::UInt16(p) => udp_port = p,
                                        TagValue::UInt32(p) => udp_port = p as u16,
                                        _ => {}
                                    }
                                }
                                TagName::Id(0xF3) => {
                                    // FT_ENCRYPTION (0xF3)
                                    entry_is_source = true;
                                    match tag.value {
                                        TagValue::UInt8(e) => encryption = e,
                                        TagValue::UInt16(e) => encryption = e as u8,
                                        TagValue::UInt32(e) => encryption = e as u8,
                                        _ => {}
                                    }
                                }
                                TagName::Id(0xFA) => {
                                    // FT_SERVERPORT (0xFA)
                                    entry_is_source = true;
                                    if udp_port == 0 {
                                        match tag.value {
                                            TagValue::UInt16(p) => udp_port = p,
                                            TagValue::UInt32(p) => udp_port = p as u16,
                                            _ => {}
                                        }
                                    }
                                }
                                TagName::Id(0xFB) => {
                                    // FT_SERVERIP (0xFB)
                                    entry_is_source = true;
                                    if ip == [0, 0, 0, 0] {
                                        if let TagValue::UInt32(ip_val) = tag.value {
                                            ip = ip_val.to_be_bytes();
                                        }
                                    }
                                }
                                TagName::Id(0xF8) => {
                                    // FT_BUDDYHASH (0xF8) — Kad buddy ID of a
                                    // LowID source, as 32-char hex string
                                    // (eMule `CKadTagStr(TAG_BUDDYHASH, md4str)`).
                                    entry_is_source = true;
                                    if let TagValue::String(s) = &tag.value {
                                        let s = s.trim();
                                        if s.len() == 32 {
                                            let mut id = [0u8; 16];
                                            let mut ok = true;
                                            for i in 0..16 {
                                                match u8::from_str_radix(&s[i * 2..i * 2 + 2], 16) {
                                                    Ok(b) => id[i] = b,
                                                    Err(_) => {
                                                        ok = false;
                                                        break;
                                                    }
                                                }
                                            }
                                            if ok {
                                                buddy_id = Some(id);
                                            }
                                        }
                                    }
                                }
                                TagName::Text(name) => {
                                    let lower = name.to_lowercase();
                                    if lower == "name" || lower == "filename" {
                                        if let TagValue::String(s) = tag.value {
                                            file_name =
                                                s.chars().filter(|c| !c.is_control()).collect();
                                        }
                                    } else if lower == "size" || lower == "filesize" {
                                        match tag.value {
                                            TagValue::UInt32(v) => file_size = v as u64,
                                            TagValue::UInt64(v) => file_size = v,
                                            TagValue::UInt16(v) => file_size = v as u64,
                                            TagValue::UInt8(v) => file_size = v as u64,
                                            _ => {}
                                        }
                                    } else if lower == "sources" {
                                        match tag.value {
                                            TagValue::UInt32(v) => source_count = v,
                                            TagValue::UInt16(v) => source_count = v as u32,
                                            TagValue::UInt8(v) => source_count = v as u32,
                                            _ => {}
                                        }
                                    } else if lower == "port" || lower == "sourceport" {
                                        entry_is_source = true;
                                        match tag.value {
                                            TagValue::UInt16(p) => tcp_port = p,
                                            TagValue::UInt32(p) => tcp_port = p as u16,
                                            _ => {}
                                        }
                                    } else if lower == "ip" || lower == "sourceip" {
                                        entry_is_source = true;
                                        if let TagValue::UInt32(ip_val) = tag.value {
                                            ip = ip_val.to_be_bytes();
                                        }
                                    } else if lower == "sourceuport" || lower == "udpport" {
                                        entry_is_source = true;
                                        match tag.value {
                                            TagValue::UInt16(p) => udp_port = p,
                                            TagValue::UInt32(p) => udp_port = p as u16,
                                            _ => {}
                                        }
                                    } else if lower == "encryption" {
                                        entry_is_source = true;
                                        match tag.value {
                                            TagValue::UInt8(e) => encryption = e,
                                            TagValue::UInt16(e) => encryption = e as u8,
                                            TagValue::UInt32(e) => encryption = e as u8,
                                            _ => {}
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        Ok((None, _)) => {}
                        Err(_) => {
                            parse_ok = false;
                            break;
                        }
                    }
                }

                if !parse_ok {
                    break;
                }
                cursor = reader.pos();

                if entry_is_source {
                    is_source_res = true;
                    source_entries.push(KadSourceWire {
                        kad_id: answer_hash,
                        ip,
                        udp_port,
                        tcp_port,
                        encryption,
                        buddy_id,
                    });
                } else {
                    let (tags, ext) = split_kw_wire_tags(parsed_tags);
                    keyword_entries.push(KadKeywordEntryWire {
                        file_hash: answer_hash,
                        file_name,
                        file_size,
                        source_count,
                        tags,
                        aich_pub: None,
                        aich_votes: ext.aich_votes,
                        publish_info: ext.publish_info,
                    });
                }
            }

            if (parse_ok || !source_entries.is_empty() || !keyword_entries.is_empty())
                && cursor <= raw_bytes.len()
            {
                if is_source_res || !source_entries.is_empty() {
                    taglist_result = Some(KadMessage::SearchSourceRes(KadSearchSourceRes {
                        target_file_hash: target_hash,
                        sources: source_entries,
                    }));
                } else {
                    taglist_result = Some(KadMessage::SearchKeywordRes(KadSearchKeywordRes {
                        keyword_hash: target_hash,
                        entries: keyword_entries,
                    }));
                }
            }
        }
    }

    if let Some(msg) = taglist_result {
        return Ok(msg);
    }

    // For Kad2 packets or packets with 34+ byte headers, never fall back to Kad1 decoding
    // as bytes 16..18 contain arbitrary target hash bytes, not a count.
    if (raw_bytes.len() >= 34 || protocol == PR_KADEMLIA || protocol == PR_KADEMLIA_PACKED)
        && raw_bytes.len() >= 32
    {
        let mut target_hash = [0u8; 16];
        target_hash.copy_from_slice(&raw_bytes[16..32]);
        return Ok(KadMessage::SearchSourceRes(KadSearchSourceRes {
            target_file_hash: target_hash,
            sources: Vec::new(),
        }));
    }

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
        let mut raw_ip = [0u8; 4];
        payload.copy_to_slice(&mut raw_ip);
        let ip = [raw_ip[3], raw_ip[2], raw_ip[1], raw_ip[0]];
        let udp_port = payload.get_u16_le();
        let tcp_port = payload.get_u16_le();
        let encryption = payload.get_u8();
        sources.push(KadSourceWire {
            kad_id,
            ip,
            udp_port,
            tcp_port,
            encryption,
            buddy_id: None,
        });
    }

    Ok(KadMessage::SearchSourceRes(KadSearchSourceRes {
        target_file_hash,
        sources,
    }))
}

/// Decodes an `OP_KADEMLIA2_PUBLISH_SOURCE_REQ` payload.
pub fn decode_publish_source_req(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 33 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadPublishSourceReq",
            Ed2kErrorKind::UnexpectedEof {
                needed: 33,
                available: payload.len(),
            },
        ));
    }

    // Try official eMule format: [target_hash 16] [kad_id 16] [tag_count 1] [tags...]
    if payload.len() >= 33 && payload[32] <= 30 {
        let mut target_file_hash = [0u8; 16];
        target_file_hash.copy_from_slice(&payload[..16]);
        let mut kad_id = [0u8; 16];
        kad_id.copy_from_slice(&payload[16..32]);
        let tag_count = payload[32];

        let mut ip = [0u8; 4];
        let mut udp_port = 0u16;
        let mut tcp_port = 0u16;
        let mut file_size = 0u64;
        let limits = Ed2kLimits::default();
        let mut reader = TagReader::new(payload, 33, protocol, Some(opcode));
        let mut parse_ok = true;

        for _ in 0..tag_count {
            match reader.read_tag(&limits) {
                Ok((Some(tag), _)) => match &tag.name {
                    TagName::Id(0xFD) | TagName::Id(TAG_SPECIAL_PORT) => match tag.value {
                        TagValue::UInt16(p) => tcp_port = p,
                        TagValue::UInt32(p) => tcp_port = p as u16,
                        _ => {}
                    },
                    TagName::Id(0xFC) => match tag.value {
                        TagValue::UInt16(p) => udp_port = p,
                        TagValue::UInt32(p) => udp_port = p as u16,
                        _ => {}
                    },
                    TagName::Id(TAG_SPECIAL_FILESIZE) => match tag.value {
                        TagValue::UInt32(v) => file_size = v as u64,
                        TagValue::UInt64(v) => file_size = v,
                        TagValue::UInt16(v) => file_size = v as u64,
                        TagValue::UInt8(v) => file_size = v as u64,
                        _ => {}
                    },
                    TagName::Id(0xFE) => {
                        if let TagValue::UInt32(ip_val) = tag.value {
                            ip = ip_val.to_be_bytes();
                        }
                    }
                    TagName::Text(name) => {
                        let lower = name.to_lowercase();
                        if lower == "port" || lower == "sourceport" {
                            match tag.value {
                                TagValue::UInt16(p) => tcp_port = p,
                                TagValue::UInt32(p) => tcp_port = p as u16,
                                _ => {}
                            }
                        } else if lower == "sourceuport" || lower == "udpport" {
                            match tag.value {
                                TagValue::UInt16(p) => udp_port = p,
                                TagValue::UInt32(p) => udp_port = p as u16,
                                _ => {}
                            }
                        } else if lower == "size" || lower == "filesize" {
                            match tag.value {
                                TagValue::UInt32(v) => file_size = v as u64,
                                TagValue::UInt64(v) => file_size = v,
                                _ => {}
                            }
                        } else if lower == "ip" || lower == "sourceip" {
                            if let TagValue::UInt32(ip_val) = tag.value {
                                ip = ip_val.to_be_bytes();
                            }
                        }
                    }
                    _ => {}
                },
                Ok((None, _)) => {}
                Err(_) => {
                    parse_ok = false;
                    break;
                }
            }
        }

        if parse_ok {
            let contact = KadContactWire {
                kad_id,
                ip,
                udp_port,
                tcp_port,
                version: 2,
            };
            return Ok(KadMessage::PublishSourceReq(KadPublishSourceReq {
                target_file_hash,
                contact,
                file_size,
            }));
        }
    }

    // Fallback: flat binary format [target_hash 16] [kad_id 16] [ip 4] [udp_port 2] [tcp_port 2] [version 1] [file_size 8]
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
    let mut raw_ip = [0u8; 4];
    payload.copy_to_slice(&mut raw_ip);
    let ip = [raw_ip[3], raw_ip[2], raw_ip[1], raw_ip[0]];
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

/// Decodes an `OP_KADEMLIA2_PUBLISH_SOURCE_RES` payload.
pub fn decode_publish_source_res(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
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
    // eMule `Process_KADEMLIA2_PUBLISH_RES`: an optional trailing options
    // byte may follow the 17-byte body; bit 0 requests a
    // `KADEMLIA2_PUBLISH_RES_ACK` datagram back ("for future use").
    let request_ack = payload.first().is_some_and(|options| options & 0x01 != 0);
    Ok(KadMessage::PublishSourceRes(KadPublishSourceRes {
        target_file_hash,
        status,
        request_ack,
    }))
}

/// Decodes an `OP_KADEMLIA2_SEARCH_KEYWORD_REQ` payload.
pub fn decode_search_keyword_req(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
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
    // eMule `Process_KADEMLIA2_SEARCH_KEY_REQ` (`KademliaUDPListener.cpp:1057`):
    // bit 15 of start_pos flags a restrictive search-expression tree; the
    // remaining bits are the pagination offset. Any trailing bytes are the
    // serialized `SSearchTerm` expression (kept raw for the caller).
    let raw_start = payload.get_u16_le();
    let restrictive = (raw_start & 0x8000) != 0;
    let start_pos = raw_start & 0x7FFF;
    let search_expression = if restrictive && !payload.is_empty() {
        Some(payload.to_vec())
    } else {
        None
    };
    Ok(KadMessage::SearchKeywordReq(KadSearchKeywordReq {
        keyword_hash,
        restrictive,
        start_pos,
        search_expression,
    }))
}

/// Decodes an `OP_KADEMLIA2_SEARCH_KEYWORD_RES_LEGACY` payload.
pub fn decode_search_keyword_res_legacy(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 18 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadSearchKeywordResLegacy",
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
            tags: Vec::new(),
            aich_pub: None,
            aich_votes: Vec::new(),
            publish_info: None,
        });
    }

    Ok(KadMessage::SearchKeywordRes(KadSearchKeywordRes {
        keyword_hash,
        entries,
    }))
}

/// Decodes an `OP_KADEMLIA2_PUBLISH_KEYWORD_REQ` payload.
pub fn decode_publish_keyword_req(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
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
    keyword_hash.copy_from_slice(&payload[..16]);
    let count =
        (u16::from_le_bytes([payload[16], payload[17]]) as usize).min(MAX_KAD_SEARCH_KEYWORDS);
    let mut cursor = 18;

    let mut entries = Vec::with_capacity(count);
    let limits = Ed2kLimits::default();

    for _ in 0..count {
        if cursor + 16 > payload.len() {
            break;
        }
        let mut file_hash = [0u8; 16];
        file_hash.copy_from_slice(&payload[cursor..cursor + 16]);
        cursor += 16;

        if cursor >= payload.len() {
            break;
        }

        // Check if next byte is tag_count (eMule tag list format)
        let tag_count = payload[cursor];
        if tag_count <= 30 && cursor + 1 + (tag_count as usize) <= payload.len() {
            cursor += 1;
            let mut file_name = String::new();
            let mut file_size = 0u64;
            let mut source_count = 1u32;
            let mut parsed_tags: Vec<Ed2kTag> = Vec::new();
            let mut reader = TagReader::new(payload, cursor, protocol, Some(opcode));
            let mut parse_ok = true;

            for _ in 0..tag_count {
                match reader.read_tag(&limits) {
                    Ok((Some(tag), _)) => {
                        parsed_tags.push(tag.clone());
                        match &tag.name {
                        TagName::Id(TAG_SPECIAL_NAME) => {
                            if let TagValue::String(s) = tag.value {
                                file_name = s.chars().filter(|c| !c.is_control()).collect();
                            }
                        }
                        TagName::Id(TAG_SPECIAL_FILESIZE) => match tag.value {
                            TagValue::UInt32(v) => file_size = v as u64,
                            TagValue::UInt64(v) => file_size = v,
                            TagValue::UInt16(v) => file_size = v as u64,
                            TagValue::UInt8(v) => file_size = v as u64,
                            _ => {}
                        },
                        TagName::Id(0x15) => match tag.value {
                            TagValue::UInt32(v) => source_count = v,
                            TagValue::UInt16(v) => source_count = v as u32,
                            TagValue::UInt8(v) => source_count = v as u32,
                            _ => {}
                        },
                        TagName::Text(name) => {
                            let lower = name.to_lowercase();
                            if lower == "name" || lower == "filename" {
                                if let TagValue::String(s) = tag.value {
                                    file_name = s.chars().filter(|c| !c.is_control()).collect();
                                }
                            } else if lower == "size" || lower == "filesize" {
                                match tag.value {
                                    TagValue::UInt32(v) => file_size = v as u64,
                                    TagValue::UInt64(v) => file_size = v,
                                    _ => {}
                                }
                            } else if lower == "sources" {
                                if let TagValue::UInt32(v) = tag.value {
                                    source_count = v;
                                }
                            }
                        }
                        _ => {}
                        }
                    }
                    Ok((None, _)) => {}
                    Err(_) => {
                        parse_ok = false;
                        break;
                    }
                }
            }

            if parse_ok {
                cursor = reader.pos();
                let (tags, ext) = split_kw_wire_tags(parsed_tags);
                entries.push(KadKeywordEntryWire {
                    file_hash,
                    file_name,
                    file_size,
                    source_count,
                    tags,
                    aich_pub: ext.aich_pub,
                    aich_votes: Vec::new(),
                    publish_info: None,
                });
                continue;
            }
        }

        // Fallback: flat binary format [name_len u16] [name_bytes] [file_size u64] [source_count u32]
        if cursor + 2 > payload.len() {
            break;
        }
        let name_len = u16::from_le_bytes([payload[cursor], payload[cursor + 1]]) as usize;
        cursor += 2;
        if cursor + name_len + 8 + 4 > payload.len() {
            break;
        }
        let name_bytes = &payload[cursor..cursor + name_len];
        cursor += name_len;
        let capped_len = name_bytes.len().min(255);
        let file_name: String = String::from_utf8_lossy(&name_bytes[..capped_len])
            .chars()
            .filter(|c| !c.is_control())
            .collect();
        let mut fs_bytes = [0u8; 8];
        fs_bytes.copy_from_slice(&payload[cursor..cursor + 8]);
        let file_size = u64::from_le_bytes(fs_bytes);
        cursor += 8;
        let mut sc_bytes = [0u8; 4];
        sc_bytes.copy_from_slice(&payload[cursor..cursor + 4]);
        let source_count = u32::from_le_bytes(sc_bytes);
        cursor += 4;

        entries.push(KadKeywordEntryWire {
            file_hash,
            file_name,
            file_size,
            source_count,
            tags: Vec::new(),
            aich_pub: None,
            aich_votes: Vec::new(),
            publish_info: None,
        });
    }

    Ok(KadMessage::PublishKeywordReq(KadPublishKeywordReq {
        keyword_hash,
        entries,
    }))
}

/// Decodes an `OP_KADEMLIA2_PUBLISH_KEYWORD_RES` payload.
pub fn decode_publish_keyword_res(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
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

// ---------------------------------------------------------------------------
// Kad notes (file comments / ratings)
// ---------------------------------------------------------------------------

/// Reads a bounded notes tag list (`tag_count:u8` then tags), preserving both
/// typed tags and unknown raw tags — note data is untrusted, display-only
/// (eMule `CDataIO::ReadTagList` with `bOptACP` semantics).
fn read_note_taglist(
    raw: &[u8],
    mut cursor: usize,
    protocol: u8,
    opcode: u8,
) -> Result<
    (
        Vec<crate::ed2k::tags::Ed2kTag>,
        Vec<crate::ed2k::tags::RawTag>,
        usize,
    ),
    Ed2kParseError,
> {
    if cursor >= raw.len() {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            cursor,
            "note_tag_count",
            Ed2kErrorKind::UnexpectedEof {
                needed: cursor + 1,
                available: raw.len(),
            },
        ));
    }
    let tag_count = raw[cursor] as usize;
    cursor += 1;
    if tag_count > MAX_NOTE_TAG_COUNT {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            cursor - 1,
            "note_tag_count",
            Ed2kErrorKind::MalformedField {
                message: format!("note tag count {tag_count} exceeds {MAX_NOTE_TAG_COUNT}"),
            },
        ));
    }
    let limits = Ed2kLimits::default();
    let mut reader = TagReader::new(raw, cursor, protocol, Some(opcode));
    let mut tags = Vec::with_capacity(tag_count);
    let mut unknown_tags = Vec::new();
    for _ in 0..tag_count {
        match reader.read_tag(&limits) {
            Ok((Some(tag), _)) => tags.push(tag),
            Ok((None, Some(raw_tag))) => unknown_tags.push(raw_tag),
            Ok((None, None)) => {}
            Err(e) => return Err(e),
        }
    }
    Ok((tags, unknown_tags, reader.pos()))
}

/// Decodes a persisted note tag-list blob (`[tag_count:u8][tags]`), the exact
/// fragment `read_note_taglist` consumes. Returns `None` on malformed data —
/// used to rehydrate the notes index from storage.
pub fn decode_note_tags_blob(
    blob: &[u8],
) -> Option<(
    Vec<crate::ed2k::tags::Ed2kTag>,
    Vec<crate::ed2k::tags::RawTag>,
)> {
    if blob.is_empty() || blob.len() > 64 * 1024 {
        return None;
    }
    read_note_taglist(blob, 0, PR_KADEMLIA, KADEMLIA2_PUBLISH_NOTES_REQ)
        .ok()
        .map(|(tags, unknown, _)| (tags, unknown))
}

/// Decodes a `KADEMLIA2_SEARCH_NOTES_REQ` (0x35) payload:
/// `[target_file_hash:16][file_size:u64]`
/// (eMule `Process_KADEMLIA2_SEARCH_NOTES_REQ`).
pub fn decode_search_notes_req(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 24 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadSearchNotesReq",
            Ed2kErrorKind::UnexpectedEof {
                needed: 24,
                available: payload.len(),
            },
        ));
    }
    let mut target_file_hash = [0u8; 16];
    payload.copy_to_slice(&mut target_file_hash);
    let file_size = payload.get_u64_le();
    Ok(KadMessage::SearchNotesReq(KadSearchNotesReq {
        target_file_hash,
        file_size,
    }))
}

/// Decodes a `KADEMLIA2_PUBLISH_NOTES_REQ` (0x45) payload:
/// `[target_file_hash:16][source_id:16][tag_count:u8][tags]`
/// (eMule `Process_KADEMLIA2_PUBLISH_NOTES_REQ`).
pub fn decode_publish_notes_req(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 33 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadPublishNotesReq",
            Ed2kErrorKind::UnexpectedEof {
                needed: 33,
                available: payload.len(),
            },
        ));
    }
    let mut target_file_hash = [0u8; 16];
    target_file_hash.copy_from_slice(&payload[..16]);
    payload.advance(16);
    let mut source_id = [0u8; 16];
    source_id.copy_from_slice(&payload[..16]);

    let (tags, unknown_tags, _) = read_note_taglist(payload, 16, protocol, opcode)?;
    Ok(KadMessage::PublishNotesReq(KadPublishNotesReq {
        target_file_hash,
        source_id,
        tags,
        unknown_tags,
    }))
}

/// Decodes a notes search response payload. Two wire forms exist:
///
/// - `KADEMLIA2_SEARCH_RES` (0x3B): `[sender_id:16][target:16][count:u16]`
///   then `count` × `[source_id:16][tag_count:u8][tags]` — sent by eMule
///   `CIndexed::SendValidNoteResult` even for Kad2 note searches.
/// - `KADEMLIA_SEARCH_NOTES_RES` (0x3A, legacy Kad1): `[target:16][count:u16]`
///   then the same entry list (no sender id).
pub fn decode_search_notes_res(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    // Header size: 0x3B carries sender_id (34 bytes), 0x3A doesn't (18 bytes).
    let (sender_id, hdr) = if opcode == KADEMLIA2_SEARCH_RES {
        if payload.len() < 34 {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                2,
                "KadSearchNotesRes",
                Ed2kErrorKind::UnexpectedEof {
                    needed: 34,
                    available: payload.len(),
                },
            ));
        }
        let mut s = [0u8; 16];
        s.copy_from_slice(&payload[..16]);
        (s, 34usize)
    } else {
        if payload.len() < 18 {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                2,
                "KadSearchNotesRes",
                Ed2kErrorKind::UnexpectedEof {
                    needed: 18,
                    available: payload.len(),
                },
            ));
        }
        ([0u8; 16], 18usize)
    };

    let mut target_file_hash = [0u8; 16];
    target_file_hash.copy_from_slice(&payload[hdr - 18..hdr - 2]);
    let count = u16::from_le_bytes([payload[hdr - 2], payload[hdr - 1]]) as usize;
    if count > MAX_NOTE_SEARCH_RESULTS {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            hdr,
            "KadSearchNotesRes::count",
            Ed2kErrorKind::MalformedField {
                message: format!("notes result count {count} exceeds {MAX_NOTE_SEARCH_RESULTS}"),
            },
        ));
    }

    let mut cursor = hdr;
    let mut notes = Vec::with_capacity(count.min(16));
    for _ in 0..count {
        if cursor + 17 > payload.len() {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                cursor,
                "KadSearchNotesRes::entry",
                Ed2kErrorKind::UnexpectedEof {
                    needed: cursor + 17,
                    available: payload.len(),
                },
            ));
        }
        let mut source_id = [0u8; 16];
        source_id.copy_from_slice(&payload[cursor..cursor + 16]);
        cursor += 16;
        let (tags, unknown_tags, next) = read_note_taglist(payload, cursor, protocol, opcode)?;
        cursor = next;
        notes.push(KadNoteWire {
            source_id,
            tags,
            unknown_tags,
        });
    }

    Ok(KadMessage::SearchNotesRes(KadSearchNotesRes {
        sender_id,
        target_file_hash,
        notes,
    }))
}
