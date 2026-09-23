// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! AICH message decoders (`OP_AICHFILEHASHREQ`/`ANS`, `OP_AICHREQUEST`/`ANSWER`).

use crate::ed2k::error::{Ed2kErrorKind, Ed2kParseError};
use crate::ed2k::packet::{
    Ed2kAichAnswer, Ed2kAichFileHashReq, Ed2kAichFileHashRes, Ed2kAichRecovery, Ed2kAichRequest,
};

/// Maximum number of AICH recovery entries accepted in one answer.
/// A part has at most 53 blocks plus a handful of sibling hashes; the bound is
/// generous but keeps malformed counts from causing large allocations.
const MAX_AICH_RECOVERY_ENTRIES: usize = 4096;

fn eof(
    protocol: u8,
    opcode: u8,
    offset: usize,
    field: &'static str,
    needed: usize,
    available: usize,
) -> Ed2kParseError {
    Ed2kParseError::new(
        protocol,
        Some(opcode),
        offset,
        field,
        Ed2kErrorKind::UnexpectedEof { needed, available },
    )
}

fn malformed(
    protocol: u8,
    opcode: u8,
    offset: usize,
    field: &'static str,
    message: String,
) -> Ed2kParseError {
    Ed2kParseError::new(
        protocol,
        Some(opcode),
        offset,
        field,
        Ed2kErrorKind::MalformedField { message },
    )
}

/// Decodes an `OP_AICHFILEHASHREQ` (`0x9E`, eMule 0.72a) payload: bare 16-byte file hash.
pub fn decode_aich_file_hash_req(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kAichFileHashReq, Ed2kParseError> {
    if payload.len() < 16 {
        return Err(eof(
            protocol,
            opcode,
            base_offset + payload.len(),
            "aich_fhreq_hash",
            16,
            payload.len(),
        ));
    }
    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[..16]);
    if file_hash == [0u8; 16] {
        return Err(malformed(
            protocol,
            opcode,
            base_offset,
            "aich_fhreq_file_hash",
            "zero file hash is not permitted".to_string(),
        ));
    }
    Ok(Ed2kAichFileHashReq { file_hash })
}

/// Decodes an `OP_AICHFILEHASHANS` (`0x9D`, eMule 0.72a) payload: `file_hash` + 20-byte AICH root.
pub fn decode_aich_file_hash_res(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kAichFileHashRes, Ed2kParseError> {
    if payload.len() < 36 {
        return Err(eof(
            protocol,
            opcode,
            base_offset + payload.len(),
            "aich_fhres_payload",
            36,
            payload.len(),
        ));
    }
    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[..16]);
    let mut aich_hash = [0u8; 20];
    aich_hash.copy_from_slice(&payload[16..36]);
    if aich_hash == [0u8; 20] {
        return Err(malformed(
            protocol,
            opcode,
            base_offset + 16,
            "aich_fhres_root",
            "zero AICH root hash is not permitted".to_string(),
        ));
    }
    Ok(Ed2kAichFileHashRes {
        file_hash,
        aich_hash,
    })
}

/// Decodes an `OP_AICHREQUEST` (`0x9B`, eMule 0.72a) payload: `file_hash` + `u16 part` + `master_hash`.
pub fn decode_aich_request(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kAichRequest, Ed2kParseError> {
    // eMule ProcessAICHRequest requires exactly 16 + 2 + 20 bytes.
    if payload.len() != 38 {
        return Err(malformed(
            protocol,
            opcode,
            base_offset,
            "aich_request_size",
            format!(
                "AICH request payload must be exactly 38 bytes, got {}",
                payload.len()
            ),
        ));
    }
    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[..16]);
    let part = u16::from_le_bytes([payload[16], payload[17]]);
    let mut master_hash = [0u8; 20];
    master_hash.copy_from_slice(&payload[18..38]);
    if file_hash == [0u8; 16] {
        return Err(malformed(
            protocol,
            opcode,
            base_offset,
            "aich_request_file_hash",
            "zero file hash is not permitted".to_string(),
        ));
    }
    if master_hash == [0u8; 20] {
        return Err(malformed(
            protocol,
            opcode,
            base_offset + 18,
            "aich_request_master",
            "zero AICH master hash is not permitted".to_string(),
        ));
    }
    Ok(Ed2kAichRequest {
        file_hash,
        part,
        master_hash,
    })
}

/// Decodes an `OP_AICHANSWER` (`0x9C`, eMule 0.72a) payload.
///
/// Bare 16-byte payload = failed answer. Otherwise:
/// `file_hash` + `u16 part` + `master_hash` + `u16 count16` + entries +
/// `u16 count32` + entries.
pub fn decode_aich_answer(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kAichAnswer, Ed2kParseError> {
    if payload.len() == 16 {
        let mut file_hash = [0u8; 16];
        file_hash.copy_from_slice(&payload[..16]);
        return Ok(Ed2kAichAnswer {
            file_hash,
            recovery: None,
        });
    }
    if payload.len() < 42 {
        return Err(eof(
            protocol,
            opcode,
            base_offset + payload.len(),
            "aich_answer_header",
            42,
            payload.len(),
        ));
    }
    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[..16]);
    let part = u16::from_le_bytes([payload[16], payload[17]]);
    let mut master_hash = [0u8; 20];
    master_hash.copy_from_slice(&payload[18..38]);

    let mut pos = 38usize;
    let count16 = u16::from_le_bytes([payload[pos], payload[pos + 1]]) as usize;
    pos += 2;
    if count16 > MAX_AICH_RECOVERY_ENTRIES {
        return Err(malformed(
            protocol,
            opcode,
            base_offset + pos,
            "aich_answer_count16",
            format!("16-bit entry count {count16} exceeds limit"),
        ));
    }
    if payload.len() < pos + count16 * 22 + 2 {
        return Err(eof(
            protocol,
            opcode,
            base_offset + pos,
            "aich_answer_entries16",
            pos + count16 * 22 + 2,
            payload.len(),
        ));
    }
    let mut entries16 = Vec::with_capacity(count16);
    for _ in 0..count16 {
        let ident = u16::from_le_bytes([payload[pos], payload[pos + 1]]);
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&payload[pos + 2..pos + 22]);
        entries16.push((ident, hash));
        pos += 22;
    }

    let count32 = u16::from_le_bytes([payload[pos], payload[pos + 1]]) as usize;
    pos += 2;
    if count32 > MAX_AICH_RECOVERY_ENTRIES {
        return Err(malformed(
            protocol,
            opcode,
            base_offset + pos,
            "aich_answer_count32",
            format!("32-bit entry count {count32} exceeds limit"),
        ));
    }
    if payload.len() < pos + count32 * 24 {
        return Err(eof(
            protocol,
            opcode,
            base_offset + pos,
            "aich_answer_entries32",
            pos + count32 * 24,
            payload.len(),
        ));
    }
    let mut entries32 = Vec::with_capacity(count32);
    for _ in 0..count32 {
        let ident = u32::from_le_bytes([
            payload[pos],
            payload[pos + 1],
            payload[pos + 2],
            payload[pos + 3],
        ]);
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&payload[pos + 4..pos + 24]);
        entries32.push((ident, hash));
        pos += 24;
    }

    if payload.len() != pos {
        return Err(malformed(
            protocol,
            opcode,
            base_offset + pos,
            "aich_answer_trailing",
            format!(
                "{} unexpected trailing bytes in AICH answer",
                payload.len() - pos
            ),
        ));
    }
    if count16 == 0 && count32 == 0 {
        return Err(malformed(
            protocol,
            opcode,
            base_offset,
            "aich_answer_empty",
            "AICH answer contains no recovery hashes".to_string(),
        ));
    }

    Ok(Ed2kAichAnswer {
        file_hash,
        recovery: Some(Ed2kAichRecovery {
            part,
            master_hash,
            entries16,
            entries32,
        }),
    })
}
