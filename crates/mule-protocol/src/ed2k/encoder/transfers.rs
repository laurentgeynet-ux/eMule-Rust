//! Transfer message encoders (FileStatus, RequestParts, SendingPart, FileReqAnsNoFile).

use bytes::Bytes;

use super::super::error::{Ed2kEncodeError, Ed2kErrorKind};
use super::super::limits::Ed2kLimits;
use super::super::opcodes::{
    OP_FILEREQANSNOFILE, OP_FILESTATUS, OP_HASHSETANSWER, OP_HASHSETREQUEST, OP_SENDINGPART,
};
use super::super::packet::{
    expected_mask_bytes, Ed2kFileReqAnsNoFile, Ed2kFileStatus, Ed2kFileStatusReq,
    Ed2kHashSetAnswer, Ed2kHashSetReq, Ed2kRequestParts, Ed2kSendingPart,
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
