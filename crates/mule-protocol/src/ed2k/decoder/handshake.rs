// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Handshake payload decoders (OP_HELLO, OP_HELLO_ANSWER, OP_EMULE_INFO).

use super::super::error::{Ed2kErrorKind, Ed2kParseError};
use super::super::limits::Ed2kLimits;
use super::super::packet::{Ed2kEmuleInfo, Ed2kHello, Ed2kHelloAnswer};
use super::super::tags::TagReader;

/// Decodes an `OP_HELLO` payload.
pub fn decode_hello(
    limits: &Ed2kLimits,
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kHello, Ed2kParseError> {
    let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));

    // In standard eD2K client-to-client handshake, OP_HELLO payload begins with hash length (0x10 = 16).
    // If present, consume it. If absent (e.g. legacy/synthetic fixtures), read hash directly.
    if reader.remaining() >= 17 && reader.peek_u8() == Some(0x10) {
        let _ = reader.read_u8("hello_user_hash_len")?;
    }

    let user_hash = reader.read_hash("hello_user_hash")?;
    let client_id = reader.read_u32_le("hello_client_id")?;
    let tcp_port = reader.read_u16_le("hello_tcp_port")?;
    let tag_count = reader.read_u32_le("hello_tag_count")? as usize;

    if tag_count > limits.max_tags {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + reader.pos() - 4,
            "hello_tag_count",
            Ed2kErrorKind::TooManyTags {
                count: tag_count,
                limit: limits.max_tags,
            },
        ));
    }

    let mut tags = Vec::with_capacity(tag_count.min(64));
    let mut unknown_tags = Vec::new();

    for _ in 0..tag_count {
        let (typed_opt, raw_opt) = reader.read_tag(limits)?;
        if let Some(tag) = typed_opt {
            tags.push(tag);
        }
        if let Some(raw) = raw_opt {
            unknown_tags.push(raw);
        }
    }

    let (server_ip, server_port) = if reader.remaining() >= 6 {
        let ip = reader.read_u32_le("hello_server_ip")?;
        let port = reader.read_u16_le("hello_server_port")?;
        (ip, port)
    } else {
        (0, 0)
    };

    Ok(Ed2kHello {
        user_hash,
        client_id,
        tcp_port,
        tags,
        unknown_tags,
        server_ip,
        server_port,
    })
}

/// Decodes an `OP_HELLO_ANSWER` payload.
pub fn decode_hello_answer(
    limits: &Ed2kLimits,
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kHelloAnswer, Ed2kParseError> {
    let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));

    let user_hash = reader.read_hash("hello_answer_user_hash")?;
    let client_id = reader.read_u32_le("hello_answer_client_id")?;
    let tcp_port = reader.read_u16_le("hello_answer_tcp_port")?;
    let tag_count = reader.read_u32_le("hello_answer_tag_count")? as usize;

    if tag_count > limits.max_tags {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + reader.pos() - 4,
            "hello_answer_tag_count",
            Ed2kErrorKind::TooManyTags {
                count: tag_count,
                limit: limits.max_tags,
            },
        ));
    }

    let mut tags = Vec::with_capacity(tag_count.min(64));
    let mut unknown_tags = Vec::new();

    for _ in 0..tag_count {
        let (typed_opt, raw_opt) = reader.read_tag(limits)?;
        if let Some(tag) = typed_opt {
            tags.push(tag);
        }
        if let Some(raw) = raw_opt {
            unknown_tags.push(raw);
        }
    }

    let (server_ip, server_port) = if reader.remaining() >= 6 {
        let ip = reader.read_u32_le("hello_answer_server_ip")?;
        let port = reader.read_u16_le("hello_answer_server_port")?;
        (ip, port)
    } else {
        (0, 0)
    };

    Ok(Ed2kHelloAnswer {
        user_hash,
        client_id,
        tcp_port,
        tags,
        unknown_tags,
        server_ip,
        server_port,
    })
}

/// Decodes an `OP_EMULE_INFO` (`0xC5 0x01`) payload.
pub fn decode_emule_info(
    limits: &Ed2kLimits,
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kEmuleInfo, Ed2kParseError> {
    let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));

    let client_version = reader.read_u8("emule_info_client_version")?;
    let protocol_version = reader.read_u8("emule_info_protocol_version")?;
    let tag_count = reader.read_u32_le("emule_info_tag_count")? as usize;

    if tag_count > limits.max_tags {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + reader.pos() - 4,
            "emule_info_tag_count",
            Ed2kErrorKind::TooManyTags {
                count: tag_count,
                limit: limits.max_tags,
            },
        ));
    }

    let mut tags = Vec::with_capacity(tag_count.min(64));
    let mut unknown_tags = Vec::new();

    for _ in 0..tag_count {
        let (typed_opt, raw_opt) = reader.read_tag(limits)?;
        if let Some(tag) = typed_opt {
            tags.push(tag);
        }
        if let Some(raw) = raw_opt {
            unknown_tags.push(raw);
        }
    }

    Ok(Ed2kEmuleInfo {
        client_version,
        protocol_version,
        tags,
        unknown_tags,
    })
}
