//! Handshake message encoders (Hello, HelloAnswer, EmuleInfo).

use bytes::Bytes;

use super::super::error::{Ed2kEncodeError, Ed2kErrorKind};
use super::super::limits::Ed2kLimits;
use super::super::opcodes::{OP_EMULE_INFO, OP_HELLO, OP_HELLO_ANSWER};
use super::super::packet::{Ed2kEmuleInfo, Ed2kHello, Ed2kHelloAnswer};
use super::super::profile::Ed2kCompatibilityProfile;
use super::super::protocol::{PR_ED2K, PR_EMULE};
use super::tags::{encode_raw_tag, encode_tag};
use crate::framing::RawPacket;

/// Encodes an `Ed2kHello` message into a `RawPacket`.
pub fn encode_hello(
    limits: &Ed2kLimits,
    profile: Ed2kCompatibilityProfile,
    hello: &Ed2kHello,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_HELLO;
    let total_tags = hello.tags.len().saturating_add(hello.unknown_tags.len());
    if total_tags > limits.max_tags {
        return Err(Ed2kEncodeError::new(
            opcode,
            "hello_tag_count",
            Ed2kErrorKind::TooManyTags {
                count: total_tags,
                limit: limits.max_tags,
            },
        ));
    }

    let mut payload = Vec::with_capacity(64);
    if profile != Ed2kCompatibilityProfile::LegacyEdonkey {
        payload.push(0x10); // Standard eD2K OP_HELLO user hash length prefix (16 bytes)
    }
    payload.extend_from_slice(&hello.user_hash);
    payload.extend_from_slice(&hello.client_id.to_le_bytes());
    payload.extend_from_slice(&hello.tcp_port.to_le_bytes());
    payload.extend_from_slice(&(total_tags as u32).to_le_bytes());

    for tag in &hello.tags {
        encode_tag(limits, opcode, tag, &mut payload)?;
    }
    for raw in &hello.unknown_tags {
        encode_raw_tag(limits, opcode, raw, &mut payload)?;
    }

    // Standard eD2K handshake always includes server IP (4 bytes LE) and port (2 bytes LE)
    payload.extend_from_slice(&hello.server_ip.to_le_bytes());
    payload.extend_from_slice(&hello.server_port.to_le_bytes());

    let frame_len = 1 + payload.len();
    let max_frame = limits.max_frame_for_opcode(opcode);
    if frame_len > max_frame {
        return Err(Ed2kEncodeError::new(
            opcode,
            "frame_len",
            Ed2kErrorKind::FrameTooLarge {
                size: frame_len,
                limit: max_frame,
            },
        ));
    }

    Ok(RawPacket::new(PR_ED2K, opcode, Bytes::from(payload)))
}

/// Encodes an `Ed2kHelloAnswer` message into a `RawPacket`.
pub fn encode_hello_answer(
    limits: &Ed2kLimits,
    answer: &Ed2kHelloAnswer,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_HELLO_ANSWER;
    let total_tags = answer.tags.len().saturating_add(answer.unknown_tags.len());
    if total_tags > limits.max_tags {
        return Err(Ed2kEncodeError::new(
            opcode,
            "hello_answer_tag_count",
            Ed2kErrorKind::TooManyTags {
                count: total_tags,
                limit: limits.max_tags,
            },
        ));
    }

    let mut payload = Vec::with_capacity(64);
    payload.extend_from_slice(&answer.user_hash);
    payload.extend_from_slice(&answer.client_id.to_le_bytes());
    payload.extend_from_slice(&answer.tcp_port.to_le_bytes());
    payload.extend_from_slice(&(total_tags as u32).to_le_bytes());

    for tag in &answer.tags {
        encode_tag(limits, opcode, tag, &mut payload)?;
    }
    for raw in &answer.unknown_tags {
        encode_raw_tag(limits, opcode, raw, &mut payload)?;
    }

    payload.extend_from_slice(&answer.server_ip.to_le_bytes());
    payload.extend_from_slice(&answer.server_port.to_le_bytes());

    let frame_len = 1 + payload.len();
    let max_frame = limits.max_frame_for_opcode(opcode);
    if frame_len > max_frame {
        return Err(Ed2kEncodeError::new(
            opcode,
            "frame_len",
            Ed2kErrorKind::FrameTooLarge {
                size: frame_len,
                limit: max_frame,
            },
        ));
    }

    Ok(RawPacket::new(PR_ED2K, opcode, Bytes::from(payload)))
}

/// Encodes an `OP_EMULE_INFO` (`0xC5 0x01`) payload.
pub fn encode_emule_info(
    limits: &Ed2kLimits,
    info: &Ed2kEmuleInfo,
) -> Result<RawPacket, Ed2kEncodeError> {
    let total_tags = info.tags.len() + info.unknown_tags.len();
    if total_tags > limits.max_tags {
        return Err(Ed2kEncodeError::new(
            OP_EMULE_INFO,
            "emule_info_tag_count",
            Ed2kErrorKind::TooManyTags {
                count: total_tags,
                limit: limits.max_tags,
            },
        ));
    }

    let mut payload = Vec::with_capacity(64);
    payload.push(info.client_version);
    payload.push(info.protocol_version);
    payload.extend_from_slice(&(total_tags as u32).to_le_bytes());

    for tag in &info.tags {
        encode_tag(limits, OP_EMULE_INFO, tag, &mut payload)?;
    }
    for raw in &info.unknown_tags {
        encode_raw_tag(limits, OP_EMULE_INFO, raw, &mut payload)?;
    }

    let frame_len = 1 + payload.len();
    let max_frame = limits.max_frame_for_opcode(OP_EMULE_INFO);
    if frame_len > max_frame {
        return Err(Ed2kEncodeError::new(
            OP_EMULE_INFO,
            "emule_info_frame_len",
            Ed2kErrorKind::FrameTooLarge {
                size: frame_len,
                limit: max_frame,
            },
        ));
    }

    Ok(RawPacket::new(
        PR_EMULE,
        OP_EMULE_INFO,
        Bytes::from(payload),
    ))
}
