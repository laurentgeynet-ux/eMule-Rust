//! Decoders for Kad sources and search responses.

use bytes::Buf;
use crate::ed2k::limits::Ed2kLimits;
use crate::ed2k::tags::{
    TagName, TagReader, TagValue, TAG_SPECIAL_FILESIZE, TAG_SPECIAL_NAME, TAG_SPECIAL_PORT,
};
use crate::ed2k::{Ed2kErrorKind, Ed2kParseError};
use crate::kad::opcodes::*;
use crate::kad::types::*;

/// Decodes an `OP_KADEMLIA2_SEARCH_SOURCE_REQ` payload.
/// Official eMule format: [target_hash 16] [start_pos u16] [file_size u64] (26 bytes).
pub fn decode_search_source_req(protocol: u8, opcode: u8, mut payload: &[u8]) -> Result<KadMessage, Ed2kParseError> {
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
pub fn decode_search_res(protocol: u8, opcode: u8, mut payload: &[u8]) -> Result<KadMessage, Ed2kParseError> {
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
                let mut entry_is_source = false;

                let mut reader = TagReader::new(raw_bytes, cursor, protocol, Some(opcode));
                for _ in 0..tag_count {
                    match reader.read_tag(&limits) {
                        Ok((Some(tag), _)) => {
                            match &tag.name {
                                TagName::Id(TAG_SPECIAL_NAME) => {
                                    if let TagValue::String(s) = tag.value {
                                        file_name = s.chars().filter(|c| !c.is_control()).collect();
                                    }
                                }
                                TagName::Id(TAG_SPECIAL_FILESIZE) => {
                                    match tag.value {
                                        TagValue::UInt32(v) => file_size = v as u64,
                                        TagValue::UInt64(v) => file_size = v,
                                        TagValue::UInt16(v) => file_size = v as u64,
                                        TagValue::UInt8(v) => file_size = v as u64,
                                        _ => {}
                                    }
                                }
                                TagName::Id(0x15) => {
                                    // FT_SOURCES (0x15)
                                    match tag.value {
                                        TagValue::UInt32(v) => source_count = v,
                                        TagValue::UInt16(v) => source_count = v as u32,
                                        TagValue::UInt8(v) => source_count = v as u32,
                                        _ => {}
                                    }
                                }
                                TagName::Id(0x33) => {
                                    // FT_PUBLISHINFO (0x33): (uNames << 24) | (uPublishers << 16) | uTrust
                                    // publishers_known is (v >> 16) & 0xFF
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
                                    // FT_BUDDYHASH (0xF8)
                                    entry_is_source = true;
                                }
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
                    });
                } else {
                    keyword_entries.push(KadKeywordEntryWire {
                        file_hash: answer_hash,
                        file_name,
                        file_size,
                        source_count,
                    });
                }
            }

            if parse_ok && cursor <= raw_bytes.len() {
                if is_source_res {
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
        });
    }

    Ok(KadMessage::SearchSourceRes(KadSearchSourceRes {
        target_file_hash,
        sources,
    }))
}

/// Decodes an `OP_KADEMLIA2_PUBLISH_SOURCE_REQ` payload.
pub fn decode_publish_source_req(protocol: u8, opcode: u8, mut payload: &[u8]) -> Result<KadMessage, Ed2kParseError> {
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
                Ok((Some(tag), _)) => {
                    match &tag.name {
                        TagName::Id(0xFD) | TagName::Id(TAG_SPECIAL_PORT) => {
                            match tag.value {
                                TagValue::UInt16(p) => tcp_port = p,
                                TagValue::UInt32(p) => tcp_port = p as u16,
                                _ => {}
                            }
                        }
                        TagName::Id(0xFC) => {
                            match tag.value {
                                TagValue::UInt16(p) => udp_port = p,
                                TagValue::UInt32(p) => udp_port = p as u16,
                                _ => {}
                            }
                        }
                        TagName::Id(TAG_SPECIAL_FILESIZE) => {
                            match tag.value {
                                TagValue::UInt32(v) => file_size = v as u64,
                                TagValue::UInt64(v) => file_size = v,
                                TagValue::UInt16(v) => file_size = v as u64,
                                TagValue::UInt8(v) => file_size = v as u64,
                                _ => {}
                            }
                        }
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
pub fn decode_publish_source_res(protocol: u8, opcode: u8, mut payload: &[u8]) -> Result<KadMessage, Ed2kParseError> {
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

/// Decodes an `OP_KADEMLIA2_SEARCH_KEYWORD_REQ` payload.
pub fn decode_search_keyword_req(protocol: u8, opcode: u8, mut payload: &[u8]) -> Result<KadMessage, Ed2kParseError> {
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

/// Decodes an `OP_KADEMLIA2_SEARCH_KEYWORD_RES_LEGACY` payload.
pub fn decode_search_keyword_res_legacy(protocol: u8, opcode: u8, mut payload: &[u8]) -> Result<KadMessage, Ed2kParseError> {
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
        });
    }

    Ok(KadMessage::SearchKeywordRes(KadSearchKeywordRes {
        keyword_hash,
        entries,
    }))
}

/// Decodes an `OP_KADEMLIA2_PUBLISH_KEYWORD_REQ` payload.
pub fn decode_publish_keyword_req(protocol: u8, opcode: u8, payload: &[u8]) -> Result<KadMessage, Ed2kParseError> {
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
    let count = (u16::from_le_bytes([payload[16], payload[17]]) as usize).min(MAX_KAD_SEARCH_KEYWORDS);
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
            let mut reader = TagReader::new(payload, cursor, protocol, Some(opcode));
            let mut parse_ok = true;

            for _ in 0..tag_count {
                match reader.read_tag(&limits) {
                    Ok((Some(tag), _)) => {
                        match &tag.name {
                            TagName::Id(TAG_SPECIAL_NAME) => {
                                if let TagValue::String(s) = tag.value {
                                    file_name = s.chars().filter(|c| !c.is_control()).collect();
                                }
                            }
                            TagName::Id(TAG_SPECIAL_FILESIZE) => {
                                match tag.value {
                                    TagValue::UInt32(v) => file_size = v as u64,
                                    TagValue::UInt64(v) => file_size = v,
                                    TagValue::UInt16(v) => file_size = v as u64,
                                    TagValue::UInt8(v) => file_size = v as u64,
                                    _ => {}
                                }
                            }
                            TagName::Id(0x15) => {
                                match tag.value {
                                    TagValue::UInt32(v) => source_count = v,
                                    TagValue::UInt16(v) => source_count = v as u32,
                                    TagValue::UInt8(v) => source_count = v as u32,
                                    _ => {}
                                }
                            }
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
                                    match tag.value {
                                        TagValue::UInt32(v) => source_count = v,
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

            if parse_ok {
                cursor = reader.pos();
                entries.push(KadKeywordEntryWire {
                    file_hash,
                    file_name,
                    file_size,
                    source_count,
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
        });
    }

    Ok(KadMessage::PublishKeywordReq(KadPublishKeywordReq {
        keyword_hash,
        entries,
    }))
}

/// Decodes an `OP_KADEMLIA2_PUBLISH_KEYWORD_RES` payload.
pub fn decode_publish_keyword_res(protocol: u8, opcode: u8, mut payload: &[u8]) -> Result<KadMessage, Ed2kParseError> {
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
