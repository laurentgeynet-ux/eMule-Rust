// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Transfer and block payload decoders (RequestParts, SendingPart).

use bytes::Bytes;

use crate::ed2k::error::{Ed2kErrorKind, Ed2kParseError};
use crate::ed2k::opcodes::{OP_REQUESTPARTS, OP_REQUESTPARTS_EMULE};
use crate::ed2k::packet::{
    Ed2kBlockRange, Ed2kBlockRange64, Ed2kCompressedPartI64, Ed2kHashSetAnswer, Ed2kHashSetReq,
    Ed2kRequestParts, Ed2kRequestPartsI64, Ed2kSendingPart, Ed2kSendingPartI64,
};
use crate::ed2k::profile::Ed2kCompatibilityProfile;

/// Decodes an `OP_HASHSETREQUEST` (`0x51`) payload: a bare 16-byte file hash.
pub fn decode_hashset_request(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kHashSetReq, Ed2kParseError> {
    if payload.len() < 16 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + payload.len(),
            "hashset_req_hash",
            Ed2kErrorKind::UnexpectedEof {
                needed: 16,
                available: payload.len(),
            },
        ));
    }

    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[0..16]);

    if file_hash == [0u8; 16] {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset,
            "hashset_req_file_hash",
            Ed2kErrorKind::MalformedField {
                message: "zero file hash is not permitted in hashset request".to_string(),
            },
        ));
    }

    Ok(Ed2kHashSetReq { file_hash })
}

/// Decodes an `OP_HASHSETANSWER` (`0x52`) payload: `file_hash || u16 count || count*16 hashes`.
pub fn decode_hashset_answer(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kHashSetAnswer, Ed2kParseError> {
    if payload.len() < 18 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + payload.len(),
            "hashset_answer_header",
            Ed2kErrorKind::UnexpectedEof {
                needed: 18,
                available: payload.len(),
            },
        ));
    }

    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[0..16]);
    let part_count = u16::from_le_bytes([payload[16], payload[17]]) as usize;

    let expected_len = 18 + part_count * 16;
    if payload.len() != expected_len {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + 18,
            "hashset_answer_payload",
            Ed2kErrorKind::MalformedField {
                message: format!(
                    "hashset payload length {} does not match declared {} part hashes (expected {})",
                    payload.len(),
                    part_count,
                    expected_len
                ),
            },
        ));
    }

    let mut part_hashes = Vec::with_capacity(part_count);
    for i in 0..part_count {
        let mut h = [0u8; 16];
        h.copy_from_slice(&payload[18 + i * 16..18 + (i + 1) * 16]);
        part_hashes.push(h);
    }

    Ok(Ed2kHashSetAnswer {
        file_hash,
        part_hashes,
    })
}

/// Decodes a request parts payload according to an explicit `Ed2kCompatibilityProfile`.
pub fn decode_request_parts_with_profile(
    profile: Ed2kCompatibilityProfile,
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kRequestParts, Ed2kParseError> {
    match profile {
        Ed2kCompatibilityProfile::Emule072a | Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
            if opcode == OP_REQUESTPARTS {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset,
                    "request_parts_opcode",
                    Ed2kErrorKind::AmbiguousOpcodeForProfile {
                        opcode,
                        profile,
                        reason:
                            "opcode 0x4B is OP_ASKSHAREDFILESANSWER in eMule/aMule, not OP_REQUESTPARTS",
                    },
                ));
            }
            if opcode != OP_REQUESTPARTS_EMULE {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset,
                    "request_parts_opcode",
                    Ed2kErrorKind::AmbiguousOpcodeForProfile {
                        opcode,
                        profile,
                        reason: "expected OP_REQUESTPARTS_EMULE (0x47)",
                    },
                ));
            }
            if payload.len() == 24 {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset + 16,
                    "request_parts_layout",
                    Ed2kErrorKind::MalformedField {
                        message:
                            "24-byte single-slot layout is not permitted under eMule/aMule profile (expected 40-byte 3-slot layout)"
                                .to_string(),
                    },
                ));
            }
        }
        Ed2kCompatibilityProfile::LegacyEdonkey => {
            if opcode == OP_REQUESTPARTS_EMULE {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset,
                    "request_parts_opcode",
                    Ed2kErrorKind::AmbiguousOpcodeForProfile {
                        opcode,
                        profile,
                        reason:
                            "opcode 0x47 was OP_SETREQFILEID in legacy eDonkey, not OP_REQUESTPARTS",
                    },
                ));
            }
            if opcode != OP_REQUESTPARTS {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset,
                    "request_parts_opcode",
                    Ed2kErrorKind::AmbiguousOpcodeForProfile {
                        opcode,
                        profile,
                        reason: "expected OP_REQUESTPARTS (0x4B)",
                    },
                ));
            }
        }
        Ed2kCompatibilityProfile::UnknownStrict => {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset,
                "request_parts_profile",
                Ed2kErrorKind::AmbiguousOpcodeForProfile {
                    opcode,
                    profile,
                    reason:
                        "ambiguous request parts opcode cannot be determined under UnknownStrict profile",
                },
            ));
        }
    }

    if payload.len() < 24 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + payload.len(),
            "request_parts_header",
            Ed2kErrorKind::UnexpectedEof {
                needed: 24,
                available: payload.len(),
            },
        ));
    }

    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[0..16]);

    if file_hash == [0u8; 16] {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset,
            "request_parts_file_hash",
            Ed2kErrorKind::MalformedField {
                message: "zero file hash is not permitted in request parts".to_string(),
            },
        ));
    }

    if payload.len() == 24 {
        let start = u32::from_le_bytes([payload[16], payload[17], payload[18], payload[19]]);
        let end = u32::from_le_bytes([payload[20], payload[21], payload[22], payload[23]]);

        if start >= end {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + 16,
                "request_parts_range",
                Ed2kErrorKind::InvalidBlockOffsets {
                    start: start as u64,
                    end: end as u64,
                },
            ));
        }

        return Ok(Ed2kRequestParts::single(file_hash, start, end));
    }

    if payload.len() != 40 {
        if payload.len() < 40 {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + payload.len(),
                "request_parts_payload",
                Ed2kErrorKind::UnexpectedEof {
                    needed: 40,
                    available: payload.len(),
                },
            ));
        } else {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + 40,
                "request_parts_payload",
                Ed2kErrorKind::MalformedField {
                    message: format!(
                        "unexpected payload length {} for request parts (expected 24 or 40)",
                        payload.len()
                    ),
                },
            ));
        }
    }

    let mut ranges = Vec::with_capacity(3);
    for i in 0..3 {
        let start = u32::from_le_bytes([
            payload[16 + i * 4],
            payload[17 + i * 4],
            payload[18 + i * 4],
            payload[19 + i * 4],
        ]);
        let end = u32::from_le_bytes([
            payload[28 + i * 4],
            payload[29 + i * 4],
            payload[30 + i * 4],
            payload[31 + i * 4],
        ]);

        if (start == 0 && end == 0) || (start == u32::MAX && end == u32::MAX) {
            continue;
        }

        if start >= end {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + 16 + i * 4,
                "request_parts_range",
                Ed2kErrorKind::InvalidBlockOffsets {
                    start: start as u64,
                    end: end as u64,
                },
            ));
        }

        ranges.push(Ed2kBlockRange::new(start, end));
    }

    if ranges.is_empty() {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + 16,
            "request_parts_ranges",
            Ed2kErrorKind::MalformedField {
                message: "no valid non-zero block ranges in request parts".to_string(),
            },
        ));
    }

    Ok(Ed2kRequestParts::multi(file_hash, ranges))
}

/// Decodes an `OP_REQUESTPARTS_I64` (`0xA3`) payload:
/// `<file_hash 16><start[3] u64 LE><end[3] u64 LE>` — 64 bytes.
pub fn decode_request_parts_i64(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kRequestPartsI64, Ed2kParseError> {
    if payload.len() < 64 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + payload.len(),
            "request_parts_i64_payload",
            Ed2kErrorKind::UnexpectedEof {
                needed: 64,
                available: payload.len(),
            },
        ));
    }

    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[0..16]);

    if file_hash == [0u8; 16] {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset,
            "request_parts_i64_file_hash",
            Ed2kErrorKind::MalformedField {
                message: "zero file hash is not permitted in request parts I64".to_string(),
            },
        ));
    }

    let mut ranges = Vec::with_capacity(3);
    for i in 0..3 {
        let mut start_bytes = [0u8; 8];
        start_bytes.copy_from_slice(&payload[16 + i * 8..24 + i * 8]);
        let mut end_bytes = [0u8; 8];
        end_bytes.copy_from_slice(&payload[40 + i * 8..48 + i * 8]);
        let start = u64::from_le_bytes(start_bytes);
        let end = u64::from_le_bytes(end_bytes);

        if start == 0 && end == 0 {
            continue;
        }
        if start >= end {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + 16 + i * 8,
                "request_parts_i64_range",
                Ed2kErrorKind::InvalidBlockOffsets { start, end },
            ));
        }
        ranges.push(Ed2kBlockRange64::new(start, end));
    }

    if ranges.is_empty() {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + 16,
            "request_parts_i64_ranges",
            Ed2kErrorKind::MalformedField {
                message: "no valid non-zero block ranges in request parts I64".to_string(),
            },
        ));
    }

    Ok(Ed2kRequestPartsI64 { file_hash, ranges })
}

/// Decodes an `OP_SENDINGPART_I64` (`0xA2`) payload:
/// `<file_hash 16><start u64 LE><end u64 LE><data>`.
pub fn decode_sending_part_i64(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kSendingPartI64, Ed2kParseError> {
    if payload.len() < 32 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + payload.len(),
            "sending_part_i64_header",
            Ed2kErrorKind::UnexpectedEof {
                needed: 32,
                available: payload.len(),
            },
        ));
    }

    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[0..16]);

    let mut start_bytes = [0u8; 8];
    start_bytes.copy_from_slice(&payload[16..24]);
    let mut end_bytes = [0u8; 8];
    end_bytes.copy_from_slice(&payload[24..32]);
    let start_offset = u64::from_le_bytes(start_bytes);
    let end_offset = u64::from_le_bytes(end_bytes);

    if start_offset >= end_offset {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + 16,
            "sending_part_i64_offsets",
            Ed2kErrorKind::InvalidBlockOffsets {
                start: start_offset,
                end: end_offset,
            },
        ));
    }

    let expected_len = (end_offset - start_offset) as usize;
    let actual_len = payload.len() - 32;
    if actual_len != expected_len {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + 32,
            "sending_part_i64_payload",
            Ed2kErrorKind::BlockPayloadMismatch {
                expected: expected_len,
                actual: actual_len,
            },
        ));
    }

    Ok(Ed2kSendingPartI64 {
        file_hash,
        start_offset,
        end_offset,
        data: Bytes::copy_from_slice(&payload[32..]),
    })
}

/// Decodes an `OP_COMPRESSEDPART_I64` (`0xA1`) payload:
/// `<file_hash 16><start u64 LE><compressed_size u32 LE><zlib data>`.
pub fn decode_compressed_part_i64(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kCompressedPartI64, Ed2kParseError> {
    if payload.len() < 28 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + payload.len(),
            "compressed_part_i64_header",
            Ed2kErrorKind::UnexpectedEof {
                needed: 28,
                available: payload.len(),
            },
        ));
    }

    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[0..16]);

    let mut start_bytes = [0u8; 8];
    start_bytes.copy_from_slice(&payload[16..24]);
    let mut size_bytes = [0u8; 4];
    size_bytes.copy_from_slice(&payload[24..28]);
    let start_offset = u64::from_le_bytes(start_bytes);
    let compressed_size = u32::from_le_bytes(size_bytes);

    let actual_len = payload.len() - 28;
    if compressed_size as usize != actual_len {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + 28,
            "compressed_part_i64_payload",
            Ed2kErrorKind::BlockPayloadMismatch {
                expected: compressed_size as usize,
                actual: actual_len,
            },
        ));
    }

    Ok(Ed2kCompressedPartI64 {
        file_hash,
        start_offset,
        compressed_size,
        data: Bytes::copy_from_slice(&payload[28..]),
    })
}

/// Decodes an `OP_SENDINGPART` (`0x46`) payload.
pub fn decode_sending_part(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kSendingPart, Ed2kParseError> {
    if payload.len() < 24 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + payload.len(),
            "sending_part_header",
            Ed2kErrorKind::UnexpectedEof {
                needed: 24,
                available: payload.len(),
            },
        ));
    }

    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[0..16]);

    let start_offset = u32::from_le_bytes([payload[16], payload[17], payload[18], payload[19]]);
    let end_offset = u32::from_le_bytes([payload[20], payload[21], payload[22], payload[23]]);

    if start_offset >= end_offset {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + 16,
            "sending_part_offsets",
            Ed2kErrorKind::InvalidBlockOffsets {
                start: start_offset as u64,
                end: end_offset as u64,
            },
        ));
    }

    let expected_len = (end_offset - start_offset) as usize;
    let actual_len = payload.len() - 24;

    if actual_len != expected_len {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + 24,
            "sending_part_payload",
            Ed2kErrorKind::BlockPayloadMismatch {
                expected: expected_len,
                actual: actual_len,
            },
        ));
    }

    let data = Bytes::copy_from_slice(&payload[24..]);

    Ok(Ed2kSendingPart::new(
        file_hash,
        start_offset,
        end_offset,
        data,
    ))
}
