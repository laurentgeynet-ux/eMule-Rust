//! Security identification payload decoders (PublicKey, Signature, SecIdentState).

use bytes::Bytes;

use super::super::error::{Ed2kErrorKind, Ed2kParseError};
use super::super::packet::{Ed2kPublicKey, Ed2kSecIdentState, Ed2kSignature};

/// Decodes an `OP_PUBLICKEY` (`0x85`) payload.
pub fn decode_public_key(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kPublicKey, Ed2kParseError> {
    if payload.is_empty() {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset,
            "public_key_len",
            Ed2kErrorKind::UnexpectedEof {
                needed: 1,
                available: 0,
            },
        ));
    }

    let key_len = payload[0] as usize;
    if payload.len() < 1 + key_len {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + 1,
            "public_key_payload",
            Ed2kErrorKind::UnexpectedEof {
                needed: 1 + key_len,
                available: payload.len(),
            },
        ));
    }

    let key_bytes = Bytes::copy_from_slice(&payload[1..1 + key_len]);
    Ok(Ed2kPublicKey { key_bytes })
}

/// Decodes an `OP_SIGNATURE` (`0x87`) payload.
pub fn decode_signature(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kSignature, Ed2kParseError> {
    if payload.is_empty() {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset,
            "signature_len",
            Ed2kErrorKind::UnexpectedEof {
                needed: 1,
                available: 0,
            },
        ));
    }

    let sig_len = payload[0] as usize;
    if payload.len() < 1 + sig_len {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset + 1,
            "signature_payload",
            Ed2kErrorKind::UnexpectedEof {
                needed: 1 + sig_len,
                available: payload.len(),
            },
        ));
    }

    let signature_bytes = Bytes::copy_from_slice(&payload[1..1 + sig_len]);
    Ok(Ed2kSignature { signature_bytes })
}

/// Decodes an `OP_SECIDENTSTATE` (`0x86`) payload.
pub fn decode_secident_state(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
    base_offset: usize,
) -> Result<Ed2kSecIdentState, Ed2kParseError> {
    if payload.len() < 5 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            base_offset,
            "secident_state_payload",
            Ed2kErrorKind::UnexpectedEof {
                needed: 5,
                available: payload.len(),
            },
        ));
    }

    let state = payload[0];
    let challenge = u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]]);

    Ok(Ed2kSecIdentState { state, challenge })
}
