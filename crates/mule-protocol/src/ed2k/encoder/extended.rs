// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Encoders for eMule 0.72a extended messages: `FileIdentifier`-based hashsets
//! (`OP_HASHSETREQUEST2`/`OP_HASHSETANSWER2`), grouped `MULTIPACKET*` requests
//! and answers, queue reports, chat message, file description, port test and
//! CAPTCHA.
//!
//! References: `srchybrid/opcodes.h:219-289`, `DownloadClient.cpp:259-345`,
//! `ListenSocket.cpp:834-1101`, `UploadClient.cpp` (`SendHashsetPacket`).

use bytes::Bytes;

use super::super::decoder::extended::{HASHSET2_OPT_AICH, HASHSET2_OPT_MD4};
use super::super::error::{Ed2kEncodeError, Ed2kErrorKind};
use super::super::limits::Ed2kLimits;
use super::super::opcodes::{
    OP_AICHFILEHASHREQ, OP_AICHFILEHASHRES, OP_CHANGE_SLOT, OP_CHATCAPTCHAREQ, OP_CHATCAPTCHARES,
    OP_FILEDESC, OP_FILESTATUS, OP_HASHSETANSWER2, OP_HASHSETREQUEST2, OP_MESSAGE, OP_MULTIPACKET,
    OP_MULTIPACKETANSWER, OP_MULTIPACKETANSWER_EXT2, OP_MULTIPACKET_EXT, OP_MULTIPACKET_EXT2,
    OP_PORTTEST, OP_QUEUERANK, OP_QUEUERANKING, OP_REQFILENAME_ANSWER, OP_REQFILENAME,
    OP_REQUESTSOURCES, OP_REQUESTSOURCES2, OP_SETREQFILEID_EMULE,
};
use super::super::packet::{
    expected_mask_bytes, Ed2kCaptchaRequest, Ed2kCaptchaResponse, Ed2kChangeSlot, Ed2kFileDesc,
    Ed2kChatMessage, Ed2kHashSetAnswer2, Ed2kHashSetReq2, Ed2kMultiPacketAnswer,
    Ed2kMultiPacketFileRef, Ed2kMultiPacketRequest, Ed2kMultiPacketSubAnswer,
    Ed2kMultiPacketSubRequest, Ed2kPortTest, Ed2kQueueRank, Ed2kQueueRanking, Ed2kRequestExtInfo,
};
use super::super::protocol::{PR_ED2K, PR_EMULE};
use super::tags::{encode_raw_tag, encode_tag};
use crate::framing::RawPacket;

fn finish(
    limits: &Ed2kLimits,
    protocol: u8,
    opcode: u8,
    payload: Vec<u8>,
) -> Result<RawPacket, Ed2kEncodeError> {
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
    Ok(RawPacket::new(protocol, opcode, Bytes::from(payload)))
}

fn encode_string_field(
    limits: &Ed2kLimits,
    opcode: u8,
    context: &'static str,
    s: &str,
    out: &mut Vec<u8>,
) -> Result<(), Ed2kEncodeError> {
    let bytes = s.as_bytes();
    if bytes.len() > limits.max_string_bytes || bytes.len() > u16::MAX as usize {
        return Err(Ed2kEncodeError::new(
            opcode,
            context,
            Ed2kErrorKind::OversizedString {
                size: bytes.len(),
                limit: limits.max_string_bytes.min(u16::MAX as usize),
            },
        ));
    }
    out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

/// Encodes an `Ed2kMessage` (`OP_MESSAGE`, `0x4E`, eDonkey protocol):
/// `<len u16><message bytes>`.
pub fn encode_message(
    limits: &Ed2kLimits,
    msg: &Ed2kChatMessage,
) -> Result<RawPacket, Ed2kEncodeError> {
    let mut payload = Vec::new();
    encode_string_field(limits, OP_MESSAGE, "message_text", &msg.text, &mut payload)?;
    finish(limits, PR_ED2K, OP_MESSAGE, payload)
}

/// Encodes an `Ed2kQueueRank` (`OP_QUEUERANK`, `0x5C`, eDonkey protocol):
/// `<rank u32>`.
pub fn encode_queue_rank(
    limits: &Ed2kLimits,
    qr: &Ed2kQueueRank,
) -> Result<RawPacket, Ed2kEncodeError> {
    let mut payload = Vec::with_capacity(4);
    payload.extend_from_slice(&qr.rank.to_le_bytes());
    finish(limits, PR_ED2K, OP_QUEUERANK, payload)
}

/// Encodes an `Ed2kQueueRanking` (`OP_QUEUERANKING`, `0x60`, eMule protocol):
/// exactly 12 bytes, `<rank u16><10 reserved>`.
pub fn encode_queue_ranking(
    limits: &Ed2kLimits,
    qr: &Ed2kQueueRanking,
) -> Result<RawPacket, Ed2kEncodeError> {
    let mut payload = Vec::with_capacity(12);
    payload.extend_from_slice(&qr.rank.to_le_bytes());
    payload.extend_from_slice(&qr.reserved);
    finish(limits, PR_EMULE, OP_QUEUERANKING, payload)
}

/// Encodes an `Ed2kChangeSlot` (`OP_CHANGE_SLOT`, `0x5B`, eDonkey protocol):
/// `<HASH 16>`.
pub fn encode_change_slot(
    limits: &Ed2kLimits,
    cs: &Ed2kChangeSlot,
) -> Result<RawPacket, Ed2kEncodeError> {
    let mut payload = Vec::with_capacity(16);
    payload.extend_from_slice(&cs.file_hash);
    finish(limits, PR_ED2K, OP_CHANGE_SLOT, payload)
}

/// Encodes an `Ed2kFileDesc` (`OP_FILEDESC`, `0x61`, eMule protocol):
/// `<rating u8><len u16><description bytes>`.
pub fn encode_file_desc(
    limits: &Ed2kLimits,
    fd: &Ed2kFileDesc,
) -> Result<RawPacket, Ed2kEncodeError> {
    let mut payload = Vec::new();
    payload.push(fd.rating);
    encode_string_field(limits, OP_FILEDESC, "filedesc_text", &fd.description, &mut payload)?;
    finish(limits, PR_EMULE, OP_FILEDESC, payload)
}

/// Encodes an `Ed2kPortTest` (`OP_PORTTEST`, `0xFE`, eDonkey protocol):
/// single status byte.
pub fn encode_port_test(
    limits: &Ed2kLimits,
    pt: &Ed2kPortTest,
) -> Result<RawPacket, Ed2kEncodeError> {
    finish(limits, PR_ED2K, OP_PORTTEST, vec![pt.value])
}

/// Encodes an `Ed2kCaptchaRequest` (`OP_CHATCAPTCHAREQ`, `0xA5`, eMule
/// protocol): `<tag_count u8><tags><bitmap bytes>`.
pub fn encode_captcha_request(
    limits: &Ed2kLimits,
    req: &Ed2kCaptchaRequest,
) -> Result<RawPacket, Ed2kEncodeError> {
    let tag_total = req.tags.len() + req.unknown_tags.len();
    if tag_total > limits.max_tags || tag_total > u8::MAX as usize {
        return Err(Ed2kEncodeError::new(
            OP_CHATCAPTCHAREQ,
            "captcha_tag_count",
            Ed2kErrorKind::TooManyTags {
                count: tag_total,
                limit: limits.max_tags.min(u8::MAX as usize),
            },
        ));
    }
    let mut payload = Vec::new();
    payload.push(tag_total as u8);
    for t in &req.tags {
        encode_tag(limits, OP_CHATCAPTCHAREQ, t, &mut payload)?;
    }
    for t in &req.unknown_tags {
        encode_raw_tag(limits, OP_CHATCAPTCHAREQ, t, &mut payload)?;
    }
    payload.extend_from_slice(&req.bitmap);
    finish(limits, PR_EMULE, OP_CHATCAPTCHAREQ, payload)
}

/// Encodes an `Ed2kCaptchaResponse` (`OP_CHATCAPTCHARES`, `0xA6`): `<status u8>`.
pub fn encode_captcha_response(
    limits: &Ed2kLimits,
    res: &Ed2kCaptchaResponse,
) -> Result<RawPacket, Ed2kEncodeError> {
    finish(limits, PR_EMULE, OP_CHATCAPTCHARES, vec![res.status])
}

/// Encodes an `Ed2kHashSetReq2` (`OP_HASHSETREQUEST2`, `0xB1`, eMule protocol):
/// `<FileIdentifier><options u8>`. At least one of `want_md4`/`want_aich`
/// must be set (eMule rejects empty option masks).
pub fn encode_hashset_request2(
    limits: &Ed2kLimits,
    req: &Ed2kHashSetReq2,
) -> Result<RawPacket, Ed2kEncodeError> {
    if !req.want_md4 && !req.want_aich {
        return Err(Ed2kEncodeError::new(
            OP_HASHSETREQUEST2,
            "hashset2_options",
            Ed2kErrorKind::MalformedField {
                message: "hashset2 request needs at least one of want_md4/want_aich".to_string(),
            },
        ));
    }
    let mut payload = Vec::new();
    req.identifier.write_to(&mut payload);
    let mut req_opts = 0u8;
    if req.want_md4 {
        req_opts |= HASHSET2_OPT_MD4;
    }
    if req.want_aich {
        req_opts |= HASHSET2_OPT_AICH;
    }
    payload.push(req_opts);
    finish(limits, PR_EMULE, OP_HASHSETREQUEST2, payload)
}

/// Encodes an `Ed2kHashSetAnswer2` (`OP_HASHSETANSWER2`, `0xB2`):
/// `<FileIdentifier><options u8>[<md4 set>][<aich set>]`.
pub fn encode_hashset_answer2(
    limits: &Ed2kLimits,
    ans: &Ed2kHashSetAnswer2,
) -> Result<RawPacket, Ed2kEncodeError> {
    if ans.md4_set.is_none() && ans.aich_set.is_none() {
        return Err(Ed2kEncodeError::new(
            OP_HASHSETANSWER2,
            "hashset2_answer_options",
            Ed2kErrorKind::MalformedField {
                message: "hashset2 answer carries no hashset".to_string(),
            },
        ));
    }
    let mut payload = Vec::new();
    ans.identifier.write_to(&mut payload);
    let mut opts = 0u8;
    if ans.md4_set.is_some() {
        opts |= HASHSET2_OPT_MD4;
    }
    if ans.aich_set.is_some() {
        opts |= HASHSET2_OPT_AICH;
    }
    payload.push(opts);
    if let Some(set) = &ans.md4_set {
        payload.extend_from_slice(&set.file_hash);
        payload.extend_from_slice(&(set.part_hashes.len() as u16).to_le_bytes());
        for h in &set.part_hashes {
            payload.extend_from_slice(h);
        }
    }
    if let Some(set) = &ans.aich_set {
        payload.extend_from_slice(&set.master_hash);
        payload.extend_from_slice(&(set.part_hashes.len() as u16).to_le_bytes());
        for h in &set.part_hashes {
            payload.extend_from_slice(h);
        }
    }
    finish(limits, PR_EMULE, OP_HASHSETANSWER2, payload)
}

/// Serializes one multipacket sub-request into `out`.
fn write_sub_request(
    sub: &Ed2kMultiPacketSubRequest,
    out: &mut Vec<u8>,
) -> Result<(), Ed2kEncodeError> {
    match sub {
        Ed2kMultiPacketSubRequest::FileName { ext_info } => {
            out.push(OP_REQFILENAME);
            if let Some(ext) = ext_info {
                write_ext_info(ext, out)?;
            }
        }
        Ed2kMultiPacketSubRequest::SetReqFileId => out.push(OP_SETREQFILEID_EMULE),
        Ed2kMultiPacketSubRequest::RequestSources => out.push(OP_REQUESTSOURCES),
        Ed2kMultiPacketSubRequest::RequestSources2 { version, options } => {
            out.push(OP_REQUESTSOURCES2);
            out.push(*version);
            out.extend_from_slice(&options.to_le_bytes());
        }
        Ed2kMultiPacketSubRequest::AichFileHash => out.push(OP_AICHFILEHASHREQ),
    }
    Ok(())
}

fn write_ext_info(
    ext: &Ed2kRequestExtInfo,
    out: &mut Vec<u8>,
) -> Result<(), Ed2kEncodeError> {
    if ext.part_status.len() != expected_mask_bytes(ext.part_count) {
        return Err(Ed2kEncodeError::new(
            OP_MULTIPACKET,
            "multipacket_ext_mask",
            Ed2kErrorKind::MalformedField {
                message: format!(
                    "ext-info bitfield {} bytes does not match part_count {}",
                    ext.part_status.len(),
                    ext.part_count
                ),
            },
        ));
    }
    out.extend_from_slice(&ext.part_count.to_le_bytes());
    out.extend_from_slice(&ext.part_status);
    if let Some(cs) = ext.complete_sources {
        out.extend_from_slice(&cs.to_le_bytes());
    }
    Ok(())
}

/// Encodes an `Ed2kMultiPacketRequest`. The opcode is derived from the file
/// reference: `Hash` → `0x92`, `HashAndSize` → `0xA4`, `Identifier` → `0xA9`.
pub fn encode_multipacket_request(
    limits: &Ed2kLimits,
    req: &Ed2kMultiPacketRequest,
) -> Result<RawPacket, Ed2kEncodeError> {
    let mut payload = Vec::new();
    let opcode = match &req.file {
        Ed2kMultiPacketFileRef::Hash(h) => {
            payload.extend_from_slice(h);
            OP_MULTIPACKET
        }
        Ed2kMultiPacketFileRef::HashAndSize { file_hash, file_size } => {
            payload.extend_from_slice(file_hash);
            payload.extend_from_slice(&file_size.to_le_bytes());
            OP_MULTIPACKET_EXT
        }
        Ed2kMultiPacketFileRef::Identifier(id) => {
            id.write_to(&mut payload);
            OP_MULTIPACKET_EXT2
        }
    };
    for sub in &req.sub_requests {
        write_sub_request(sub, &mut payload)?;
    }
    finish(limits, PR_EMULE, opcode, payload)
}

/// Encodes an `Ed2kMultiPacketAnswer`. The opcode is derived from the file
/// reference: `Hash`/`HashAndSize` → `0x93`, `Identifier` → `0xB0`.
pub fn encode_multipacket_answer(
    limits: &Ed2kLimits,
    ans: &Ed2kMultiPacketAnswer,
) -> Result<RawPacket, Ed2kEncodeError> {
    let mut payload = Vec::new();
    let opcode = match &ans.file {
        Ed2kMultiPacketFileRef::Hash(h) => {
            payload.extend_from_slice(h);
            OP_MULTIPACKETANSWER
        }
        Ed2kMultiPacketFileRef::HashAndSize { file_hash, .. } => {
            // eMule emits only hash or FileIdentifier on answers; a hash+size
            // reference is normalized to the legacy hash-only answer.
            payload.extend_from_slice(file_hash);
            OP_MULTIPACKETANSWER
        }
        Ed2kMultiPacketFileRef::Identifier(id) => {
            id.write_to(&mut payload);
            OP_MULTIPACKETANSWER_EXT2
        }
    };
    for sub in &ans.sub_answers {
        match sub {
            Ed2kMultiPacketSubAnswer::FileName(name) => {
                payload.push(OP_REQFILENAME_ANSWER);
                encode_string_field(limits, opcode, "mp_answer_filename", name, &mut payload)?;
            }
            Ed2kMultiPacketSubAnswer::FileStatus {
                part_count,
                part_status,
            } => {
                if part_status.len() != expected_mask_bytes(*part_count) {
                    return Err(Ed2kEncodeError::new(
                        opcode,
                        "mp_answer_filestatus",
                        Ed2kErrorKind::MalformedField {
                            message: format!(
                                "file status bitfield {} bytes does not match part_count {}",
                                part_status.len(),
                                part_count
                            ),
                        },
                    ));
                }
                payload.push(OP_FILESTATUS);
                payload.extend_from_slice(&part_count.to_le_bytes());
                payload.extend_from_slice(part_status);
            }
            Ed2kMultiPacketSubAnswer::AichFileHash(h) => {
                payload.push(OP_AICHFILEHASHRES);
                payload.extend_from_slice(h);
            }
        }
    }
    finish(limits, PR_EMULE, opcode, payload)
}
