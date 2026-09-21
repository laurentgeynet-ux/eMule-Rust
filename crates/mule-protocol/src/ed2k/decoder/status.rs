//! Decoders for file status, availability, and parts queries.

use bytes::Bytes;

use crate::ed2k::error::{Ed2kErrorKind, Ed2kParseError};
use crate::ed2k::limits::{Ed2kLimits, ED2K_MAX_PARTS};
use crate::ed2k::opcodes::{OP_FILESTATUSREQ, OP_SETREQFILEID_EMULE};
use crate::ed2k::packet::{
    expected_mask_bytes, Ed2kFileReqAnsNoFile, Ed2kFileStatus, Ed2kFileStatusReq,
    PartAvailabilityMask,
};
use crate::ed2k::profile::Ed2kCompatibilityProfile;
use crate::ed2k::tags::TagReader;

/// Decodes a file status request payload with an explicit compatibility profile.
pub fn decode_file_status_req_with_profile(
    profile: Ed2kCompatibilityProfile,
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kFileStatusReq, Ed2kParseError> {
    match profile {
        Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
            if opcode == OP_FILESTATUSREQ {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset,
                    "file_status_req_opcode",
                    Ed2kErrorKind::AmbiguousOpcodeForProfile {
                        opcode,
                        profile,
                        reason:
                            "opcode 0x49 is OP_END_OF_DOWNLOAD in eMule/aMule, not OP_FILESTATUSREQ",
                    },
                ));
            }
            if opcode != OP_SETREQFILEID_EMULE {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset,
                    "file_status_req_opcode",
                    Ed2kErrorKind::AmbiguousOpcodeForProfile {
                        opcode,
                        profile,
                        reason: "expected OP_SETREQFILEID_EMULE (0x4F)",
                    },
                ));
            }
        }
        Ed2kCompatibilityProfile::LegacyEdonkey => {
            if opcode == OP_SETREQFILEID_EMULE {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset,
                    "file_status_req_opcode",
                    Ed2kErrorKind::AmbiguousOpcodeForProfile {
                        opcode,
                        profile,
                        reason: "opcode 0x4F is not recognized in legacy eDonkey profile",
                    },
                ));
            }
            if opcode != OP_FILESTATUSREQ {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset,
                    "file_status_req_opcode",
                    Ed2kErrorKind::AmbiguousOpcodeForProfile {
                        opcode,
                        profile,
                        reason: "expected OP_FILESTATUSREQ (0x49)",
                    },
                ));
            }
        }
        Ed2kCompatibilityProfile::UnknownStrict => {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset,
                "file_status_req_profile",
                Ed2kErrorKind::AmbiguousOpcodeForProfile {
                    opcode,
                    profile,
                    reason:
                        "ambiguous file status request opcode cannot be determined under UnknownStrict profile",
                },
            ));
        }
    }

    let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));
    let file_hash = reader.read_hash("filestatusreq_file_hash")?;
    if file_hash == [0u8; 16] {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset,
            "filestatusreq_file_hash",
            Ed2kErrorKind::MalformedField {
                message: "zero file hash is not permitted in file status request".to_string(),
            },
        ));
    }
    if reader.remaining() > 0 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + reader.pos(),
            "filestatusreq_trailing_bytes",
            Ed2kErrorKind::MalformedField {
                message: format!(
                    "unexpected {} trailing bytes after file hash in file status request",
                    reader.remaining()
                ),
            },
        ));
    }
    Ok(Ed2kFileStatusReq { file_hash })
}

/// Decodes an `OP_FILESTATUS` payload.
pub fn decode_file_status(
    limits: &Ed2kLimits,
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kFileStatus, Ed2kParseError> {
    let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));

    let file_hash = reader.read_hash("filestatus_file_hash")?;
    let part_count = reader.read_u16_le("filestatus_part_count")?;

    if (part_count as usize) > limits.max_parts || (part_count as usize) > ED2K_MAX_PARTS {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + 16,
            "filestatus_part_count",
            Ed2kErrorKind::MalformedField {
                message: format!("part_count {part_count} exceeds limit {}", limits.max_parts),
            },
        ));
    }

    let bitfield_bytes_needed = if part_count == 0 {
        0
    } else {
        expected_mask_bytes(part_count)
    };

    let bitfield_bytes = if bitfield_bytes_needed > 0 {
        reader.read_bytes(bitfield_bytes_needed, "filestatus_bitfield")?
    } else {
        &[]
    };

    // eMule ignores bytes trailing the bitfield (CUpDownClient::
    // ProcessFileStatus): complete files are announced with part_count = 0
    // and some clients still append padding/flag bytes. Tolerate them up to
    // a bounded slack to avoid dropping real peers.
    if reader.remaining() > 1024 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + reader.pos(),
            "filestatus_bitfield_trailing_bytes",
            Ed2kErrorKind::MalformedField {
                message: format!(
                    "unexpected {} trailing bytes in file_status bitfield",
                    reader.remaining()
                ),
            },
        ));
    }

    let availability_mask =
        PartAvailabilityMask::from_bytes(Bytes::copy_from_slice(bitfield_bytes), part_count)
            .map_err(|k| {
                Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset + 18,
                    "filestatus_bitfield",
                    k,
                )
            })?;

    Ok(Ed2kFileStatus {
        file_hash,
        part_count,
        availability_mask,
    })
}

/// Decodes an `OP_FILEREQANSNOFILE` (`0x48`) payload (16 bytes file hash).
pub fn decode_file_req_ans_no_file(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kFileReqAnsNoFile, Ed2kParseError> {
    let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));
    let file_hash = reader.read_hash("filereqansnofile_file_hash")?;
    if reader.remaining() > 0 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + reader.pos(),
            "filereqansnofile_trailing_bytes",
            Ed2kErrorKind::MalformedField {
                message: format!(
                    "unexpected {} trailing bytes in filereqansnofile",
                    reader.remaining()
                ),
            },
        ));
    }
    Ok(Ed2kFileReqAnsNoFile { file_hash })
}
