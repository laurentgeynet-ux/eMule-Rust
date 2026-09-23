// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! AICH message encoders (`OP_AICHFILEHASHREQ`/`ANS`, `OP_AICHREQUEST`/`ANSWER`).

use bytes::Bytes;

use super::super::error::{Ed2kEncodeError, Ed2kErrorKind};
use super::super::limits::Ed2kLimits;
use super::super::opcodes::{
    OP_AICHFILEHASHREQ, OP_AICHFILEHASHRES, OP_AICH_ANSWER, OP_AICH_REQUEST,
};
use super::super::packet::{
    Ed2kAichAnswer, Ed2kAichFileHashReq, Ed2kAichFileHashRes, Ed2kAichRequest,
};
use super::super::protocol::PR_EMULE;
use crate::framing::RawPacket;

fn check_frame(limits: &Ed2kLimits, opcode: u8, payload_len: usize) -> Result<(), Ed2kEncodeError> {
    let frame_len = 1 + payload_len;
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
    Ok(())
}

/// Encodes an `OP_AICHFILEHASHREQ` (`0x9E`): bare 16-byte file hash.
pub fn encode_aich_file_hash_req(
    limits: &Ed2kLimits,
    req: &Ed2kAichFileHashReq,
) -> Result<RawPacket, Ed2kEncodeError> {
    check_frame(limits, OP_AICHFILEHASHREQ, 16)?;
    Ok(RawPacket::new(
        PR_EMULE,
        OP_AICHFILEHASHREQ,
        Bytes::copy_from_slice(&req.file_hash),
    ))
}

/// Encodes an `OP_AICHFILEHASHANS` (`0x9D`): `file_hash` + 20-byte AICH root.
pub fn encode_aich_file_hash_res(
    limits: &Ed2kLimits,
    res: &Ed2kAichFileHashRes,
) -> Result<RawPacket, Ed2kEncodeError> {
    check_frame(limits, OP_AICHFILEHASHRES, 36)?;
    let mut payload = Vec::with_capacity(36);
    payload.extend_from_slice(&res.file_hash);
    payload.extend_from_slice(&res.aich_hash);
    Ok(RawPacket::new(
        PR_EMULE,
        OP_AICHFILEHASHRES,
        Bytes::from(payload),
    ))
}

/// Encodes an `OP_AICHREQUEST` (`0x9B`): `file_hash` + `u16 part` + `master_hash`.
pub fn encode_aich_request(
    limits: &Ed2kLimits,
    req: &Ed2kAichRequest,
) -> Result<RawPacket, Ed2kEncodeError> {
    check_frame(limits, OP_AICH_REQUEST, 38)?;
    let mut payload = Vec::with_capacity(38);
    payload.extend_from_slice(&req.file_hash);
    payload.extend_from_slice(&req.part.to_le_bytes());
    payload.extend_from_slice(&req.master_hash);
    Ok(RawPacket::new(
        PR_EMULE,
        OP_AICH_REQUEST,
        Bytes::from(payload),
    ))
}

/// Encodes an `OP_AICHANSWER` (`0x9C`).
///
/// With recovery data: `file_hash` + `u16 part` + `master_hash` +
/// `u16 count16` + entries + `u16 count32` + entries.
/// Failed answer: bare 16-byte `file_hash` (eMule fallback packet).
pub fn encode_aich_answer(
    limits: &Ed2kLimits,
    ans: &Ed2kAichAnswer,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_AICH_ANSWER;
    let payload = match &ans.recovery {
        None => ans.file_hash.to_vec(),
        Some(rec) => {
            if rec.entries16.len() > u16::MAX as usize || rec.entries32.len() > u16::MAX as usize {
                return Err(Ed2kEncodeError::new(
                    opcode,
                    "aich_entry_count",
                    Ed2kErrorKind::MalformedField {
                        message: "AICH recovery entry count exceeds u16::MAX".to_string(),
                    },
                ));
            }
            let mut payload =
                Vec::with_capacity(42 + rec.entries16.len() * 22 + rec.entries32.len() * 24);
            payload.extend_from_slice(&ans.file_hash);
            payload.extend_from_slice(&rec.part.to_le_bytes());
            payload.extend_from_slice(&rec.master_hash);
            payload.extend_from_slice(&(rec.entries16.len() as u16).to_le_bytes());
            for (ident, hash) in &rec.entries16 {
                payload.extend_from_slice(&ident.to_le_bytes());
                payload.extend_from_slice(hash);
            }
            payload.extend_from_slice(&(rec.entries32.len() as u16).to_le_bytes());
            for (ident, hash) in &rec.entries32 {
                payload.extend_from_slice(&ident.to_le_bytes());
                payload.extend_from_slice(hash);
            }
            payload
        }
    };
    check_frame(limits, opcode, payload.len())?;
    Ok(RawPacket::new(PR_EMULE, opcode, Bytes::from(payload)))
}
