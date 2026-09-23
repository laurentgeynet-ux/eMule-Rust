//! Transfer message encoders (FileStatus, RequestParts, SendingPart, FileReqAnsNoFile).

use bytes::Bytes;

use super::super::error::{Ed2kEncodeError, Ed2kErrorKind};
use super::super::limits::Ed2kLimits;
use super::super::opcodes::{
    OP_COMPRESSEDPART_I64, OP_FILEREQANSNOFILE, OP_FILESTATUS, OP_HASHSETANSWER, OP_HASHSETREQUEST,
    OP_REQFILENAME, OP_REQUESTPARTS_I64, OP_SENDINGPART, OP_SENDINGPART_I64,
};
use super::super::packet::{
    expected_mask_bytes, Ed2kCompressedPartI64, Ed2kFileReqAnsNoFile, Ed2kFileStatus,
    Ed2kFileStatusReq, Ed2kHashSetAnswer, Ed2kHashSetReq, Ed2kRequestParts, Ed2kRequestPartsI64,
    Ed2kSendingPart, Ed2kSendingPartI64, PartAvailabilityMask,
};
use super::super::profile::Ed2kCompatibilityProfile;
use super::super::protocol::PR_ED2K;
use crate::framing::RawPacket;

/// Encodes an `Ed2kFileStatusReq` message using the encoder's active profile opcode.
pub fn encode_file_status_req(
    limits: &Ed2kLimits,
    profile: Ed2kCompatibilityProfile,
    req: &Ed2kFileStatusReq,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = profile.file_status_req_opcode().ok_or_else(|| {
        Ed2kEncodeError::new(
            0,
            "file_status_req_profile",
            Ed2kErrorKind::AmbiguousOpcodeForProfile {
                opcode: 0,
                profile,
                reason: "cannot encode file status request under UnknownStrict profile",
            },
        )
    })?;
    encode_file_status_req_with_opcode(limits, req, opcode)
}

/// Encodes an `Ed2kFileStatusReq` message into a `RawPacket` with a specified opcode.
pub fn encode_file_status_req_with_opcode(
    limits: &Ed2kLimits,
    req: &Ed2kFileStatusReq,
    opcode: u8,
) -> Result<RawPacket, Ed2kEncodeError> {
    let mut payload = Vec::with_capacity(16);
    payload.extend_from_slice(&req.file_hash);

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

/// Encodes an `OP_REQFILENAME` (`0x58`) request with the negotiated extended
/// info appended, matching eMule 0.72a `SendFileRequest`:
/// `hash16` + (`ext_version >= 1`: `u16 part_count` + part-status bitfield
/// LSB-first; `ext_version >= 2`: + `u16 complete_sources`).
///
/// Mandatory when the peer announced `ExtendedRequestsVersion > 0`: eMule reads
/// the ext-info unconditionally and throws on a truncated payload, dropping the
/// connection (`UploadClient.cpp` `ProcessExtendedInfo`). With `ext_version ==
/// 0`, no ext-info is appended (bare hash16).
pub fn encode_req_filename(
    limits: &Ed2kLimits,
    file_hash: &[u8; 16],
    part_status: &PartAvailabilityMask,
    complete_sources: u16,
    ext_version: u8,
) -> Result<RawPacket, Ed2kEncodeError> {
    let mut payload = Vec::with_capacity(24 + part_status.raw_bytes().len());
    payload.extend_from_slice(file_hash);
    if ext_version >= 1 {
        payload.extend_from_slice(&part_status.part_count().to_le_bytes());
        payload.extend_from_slice(part_status.raw_bytes());
    }
    if ext_version >= 2 {
        payload.extend_from_slice(&complete_sources.to_le_bytes());
    }

    let frame_len = 1 + payload.len();
    let max_frame = limits.max_frame_for_opcode(OP_REQFILENAME);
    if frame_len > max_frame {
        return Err(Ed2kEncodeError::new(
            OP_REQFILENAME,
            "frame_len",
            Ed2kErrorKind::FrameTooLarge {
                size: frame_len,
                limit: max_frame,
            },
        ));
    }

    Ok(RawPacket::new(PR_ED2K, OP_REQFILENAME, Bytes::from(payload)))
}

/// Encodes an `Ed2kFileStatus` message into a `RawPacket`.
pub fn encode_file_status(
    limits: &Ed2kLimits,
    status: &Ed2kFileStatus,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_FILESTATUS;
    if (status.part_count as usize) > limits.max_parts {
        return Err(Ed2kEncodeError::new(
            opcode,
            "filestatus_part_count",
            Ed2kErrorKind::MalformedField {
                message: format!(
                    "part_count {} exceeds limit {}",
                    status.part_count, limits.max_parts
                ),
            },
        ));
    }

    let raw_bytes = status.availability_mask.raw_bytes();
    let expected_bitfield_bytes = if status.part_count == 0 {
        0
    } else {
        expected_mask_bytes(status.part_count)
    };

    if raw_bytes.len() != expected_bitfield_bytes {
        return Err(Ed2kEncodeError::new(
            opcode,
            "filestatus_bitfield",
            Ed2kErrorKind::MalformedField {
                message: format!(
                    "bitfield length {} does not match required {} for part_count {}",
                    raw_bytes.len(),
                    expected_bitfield_bytes,
                    status.part_count
                ),
            },
        ));
    }

    let mut payload = Vec::with_capacity(18 + expected_bitfield_bytes);
    payload.extend_from_slice(&status.file_hash);
    payload.extend_from_slice(&status.part_count.to_le_bytes());
    payload.extend_from_slice(raw_bytes);

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

/// Encodes an `Ed2kFileReqAnsNoFile` (`0x48`) message into a `RawPacket`.
pub fn encode_file_req_ans_no_file(
    limits: &Ed2kLimits,
    ans: &Ed2kFileReqAnsNoFile,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_FILEREQANSNOFILE;
    let mut payload = Vec::with_capacity(16);
    payload.extend_from_slice(&ans.file_hash);

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

/// Encodes an `Ed2kHashSetReq` message (`0x51`): 16-byte file hash payload.
pub fn encode_hashset_request(
    limits: &Ed2kLimits,
    req: &Ed2kHashSetReq,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_HASHSETREQUEST;
    let payload = req.file_hash.to_vec();

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

/// Encodes an `Ed2kHashSetAnswer` message (`0x52`): `file_hash || u16 count || count*16 hashes`.
pub fn encode_hashset_answer(
    limits: &Ed2kLimits,
    ans: &Ed2kHashSetAnswer,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_HASHSETANSWER;
    if ans.part_hashes.len() > limits.max_parts {
        return Err(Ed2kEncodeError::new(
            opcode,
            "hashset_part_count",
            Ed2kErrorKind::MalformedField {
                message: format!(
                    "part hash count {} exceeds limit {}",
                    ans.part_hashes.len(),
                    limits.max_parts
                ),
            },
        ));
    }

    let mut payload = Vec::with_capacity(18 + ans.part_hashes.len() * 16);
    payload.extend_from_slice(&ans.file_hash);
    payload.extend_from_slice(&(ans.part_hashes.len() as u16).to_le_bytes());
    for h in &ans.part_hashes {
        payload.extend_from_slice(h);
    }

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

/// Encodes an `Ed2kRequestParts` message using the encoder's active profile opcode.
pub fn encode_request_parts(
    limits: &Ed2kLimits,
    profile: Ed2kCompatibilityProfile,
    req: &Ed2kRequestParts,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = profile.request_parts_opcode().ok_or_else(|| {
        Ed2kEncodeError::new(
            0,
            "request_parts_profile",
            Ed2kErrorKind::AmbiguousOpcodeForProfile {
                opcode: 0,
                profile,
                reason: "cannot encode request parts under UnknownStrict profile",
            },
        )
    })?;
    encode_request_parts_with_opcode(limits, req, opcode)
}

/// Encodes an `Ed2kRequestParts` message with a specified opcode.
pub fn encode_request_parts_with_opcode(
    limits: &Ed2kLimits,
    req: &Ed2kRequestParts,
    opcode: u8,
) -> Result<RawPacket, Ed2kEncodeError> {
    if req.ranges.len() > 3 {
        return Err(Ed2kEncodeError::new(
            opcode,
            "request_parts_ranges",
            Ed2kErrorKind::MalformedField {
                message: format!(
                    "number of requested ranges {} exceeds standard limit of 3",
                    req.ranges.len()
                ),
            },
        ));
    }

    for range in &req.ranges {
        if range.start >= range.end {
            return Err(Ed2kEncodeError::new(
                opcode,
                "request_parts_range",
                Ed2kErrorKind::InvalidBlockOffsets {
                    start: range.start as u64,
                    end: range.end as u64,
                },
            ));
        }
    }

    let mut payload = Vec::with_capacity(40);
    payload.extend_from_slice(&req.file_hash);

    for i in 0..3 {
        if let Some(range) = req.ranges.get(i) {
            payload.extend_from_slice(&range.start.to_le_bytes());
        } else {
            payload.extend_from_slice(&0u32.to_le_bytes());
        }
    }

    for i in 0..3 {
        if let Some(range) = req.ranges.get(i) {
            payload.extend_from_slice(&range.end.to_le_bytes());
        } else {
            payload.extend_from_slice(&0u32.to_le_bytes());
        }
    }

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

/// Encodes an `Ed2kRequestPartsI64` message (`OP_REQUESTPARTS_I64`, `0xA3`).
///
/// Wire layout: `<file_hash 16><start[3] u64 LE><end[3] u64 LE>` — 64 bytes,
/// same 3-slot structure as the 32-bit variant with widened offsets.
pub fn encode_request_parts_i64(
    limits: &Ed2kLimits,
    req: &Ed2kRequestPartsI64,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_REQUESTPARTS_I64;
    if req.ranges.is_empty() || req.ranges.len() > 3 {
        return Err(Ed2kEncodeError::new(
            opcode,
            "request_parts_i64_ranges",
            Ed2kErrorKind::MalformedField {
                message: format!(
                    "request parts I64 requires 1-3 ranges, got {}",
                    req.ranges.len()
                ),
            },
        ));
    }

    for range in &req.ranges {
        if range.start >= range.end {
            return Err(Ed2kEncodeError::new(
                opcode,
                "request_parts_i64_range",
                Ed2kErrorKind::InvalidBlockOffsets {
                    start: range.start,
                    end: range.end,
                },
            ));
        }
    }

    let mut payload = Vec::with_capacity(64);
    payload.extend_from_slice(&req.file_hash);

    for i in 0..3 {
        let start = req.ranges.get(i).map(|r| r.start).unwrap_or(0);
        payload.extend_from_slice(&start.to_le_bytes());
    }
    for i in 0..3 {
        let end = req.ranges.get(i).map(|r| r.end).unwrap_or(0);
        payload.extend_from_slice(&end.to_le_bytes());
    }

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

/// Encodes an `Ed2kSendingPartI64` message (`OP_SENDINGPART_I64`, `0xA2`).
///
/// Wire layout: `<file_hash 16><start u64 LE><end u64 LE><data>`.
pub fn encode_sending_part_i64(
    limits: &Ed2kLimits,
    part: &Ed2kSendingPartI64,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_SENDINGPART_I64;

    if part.start_offset >= part.end_offset {
        return Err(Ed2kEncodeError::new(
            opcode,
            "sending_part_i64_offsets",
            Ed2kErrorKind::InvalidBlockOffsets {
                start: part.start_offset,
                end: part.end_offset,
            },
        ));
    }

    let expected_len = (part.end_offset - part.start_offset) as usize;
    if part.data.len() != expected_len {
        return Err(Ed2kEncodeError::new(
            opcode,
            "sending_part_i64_payload",
            Ed2kErrorKind::BlockPayloadMismatch {
                expected: expected_len,
                actual: part.data.len(),
            },
        ));
    }

    let mut payload = Vec::with_capacity(32 + part.data.len());
    payload.extend_from_slice(&part.file_hash);
    payload.extend_from_slice(&part.start_offset.to_le_bytes());
    payload.extend_from_slice(&part.end_offset.to_le_bytes());
    payload.extend_from_slice(&part.data);

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

/// Encodes an `Ed2kCompressedPartI64` message (`OP_COMPRESSEDPART_I64`, `0xA1`).
///
/// Wire layout: `<file_hash 16><start u64 LE><compressed_size u32 LE><zlib data>`.
pub fn encode_compressed_part_i64(
    limits: &Ed2kLimits,
    part: &Ed2kCompressedPartI64,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_COMPRESSEDPART_I64;
    if part.compressed_size as usize != part.data.len() {
        return Err(Ed2kEncodeError::new(
            opcode,
            "compressed_part_i64_payload",
            Ed2kErrorKind::BlockPayloadMismatch {
                expected: part.compressed_size as usize,
                actual: part.data.len(),
            },
        ));
    }

    let mut payload = Vec::with_capacity(28 + part.data.len());
    payload.extend_from_slice(&part.file_hash);
    payload.extend_from_slice(&part.start_offset.to_le_bytes());
    payload.extend_from_slice(&part.compressed_size.to_le_bytes());
    payload.extend_from_slice(&part.data);

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

    // eMule sends compressed parts under the compressed protocol marker.
    Ok(RawPacket::new(
        super::super::protocol::PR_EMULE_COMPRESSED,
        opcode,
        Bytes::from(payload),
    ))
}

/// Encodes an `Ed2kSendingPart` message (`0x46`).
pub fn encode_sending_part(
    limits: &Ed2kLimits,
    part: &Ed2kSendingPart,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_SENDINGPART;

    if part.start_offset >= part.end_offset {
        return Err(Ed2kEncodeError::new(
            opcode,
            "sending_part_offsets",
            Ed2kErrorKind::InvalidBlockOffsets {
                start: part.start_offset as u64,
                end: part.end_offset as u64,
            },
        ));
    }

    let expected_len = (part.end_offset - part.start_offset) as usize;
    if part.data.len() != expected_len {
        return Err(Ed2kEncodeError::new(
            opcode,
            "sending_part_payload",
            Ed2kErrorKind::BlockPayloadMismatch {
                expected: expected_len,
                actual: part.data.len(),
            },
        ));
    }

    let mut payload = Vec::with_capacity(24 + part.data.len());
    payload.extend_from_slice(&part.file_hash);
    payload.extend_from_slice(&part.start_offset.to_le_bytes());
    payload.extend_from_slice(&part.end_offset.to_le_bytes());
    payload.extend_from_slice(&part.data);

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
