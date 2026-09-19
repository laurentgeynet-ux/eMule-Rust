//! Security message encoders (PublicKey, Signature, SecIdentState).

use bytes::Bytes;

use super::super::error::{Ed2kEncodeError, Ed2kErrorKind};
use super::super::limits::Ed2kLimits;
use super::super::opcodes::{OP_PUBLICKEY, OP_SECIDENTSTATE, OP_SIGNATURE};
use super::super::packet::{Ed2kPublicKey, Ed2kSecIdentState, Ed2kSignature};
use super::super::protocol::PR_EMULE;
use crate::framing::RawPacket;

/// Encodes an `OP_PUBLICKEY` (`0x85`) message into a `RawPacket`.
pub fn encode_public_key(
    limits: &Ed2kLimits,
    key: &Ed2kPublicKey,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_PUBLICKEY;
    let mut payload = Vec::with_capacity(1 + key.key_bytes.len());
    payload.push(key.key_bytes.len() as u8);
    payload.extend_from_slice(&key.key_bytes);

    let frame_len = 1 + payload.len();
    let max_frame = limits.max_frame_for_opcode(opcode);
    if frame_len > max_frame {
        return Err(Ed2kEncodeError::new(
            opcode,
            "public_key_frame_len",
            Ed2kErrorKind::FrameTooLarge {
                size: frame_len,
                limit: max_frame,
            },
        ));
    }

    Ok(RawPacket::new(PR_EMULE, opcode, Bytes::from(payload)))
}

/// Encodes an `OP_SIGNATURE` (`0x86`) message into a `RawPacket`.
pub fn encode_signature(
    limits: &Ed2kLimits,
    sig: &Ed2kSignature,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_SIGNATURE;
    let mut payload = Vec::with_capacity(1 + sig.signature_bytes.len());
    payload.push(sig.signature_bytes.len() as u8);
    payload.extend_from_slice(&sig.signature_bytes);

    let frame_len = 1 + payload.len();
    let max_frame = limits.max_frame_for_opcode(opcode);
    if frame_len > max_frame {
        return Err(Ed2kEncodeError::new(
            opcode,
            "signature_frame_len",
            Ed2kErrorKind::FrameTooLarge {
                size: frame_len,
                limit: max_frame,
            },
        ));
    }

    Ok(RawPacket::new(PR_EMULE, opcode, Bytes::from(payload)))
}

/// Encodes an `OP_SECIDENTSTATE` (`0x87`) message into a `RawPacket`.
pub fn encode_secident_state(
    limits: &Ed2kLimits,
    state: &Ed2kSecIdentState,
) -> Result<RawPacket, Ed2kEncodeError> {
    let opcode = OP_SECIDENTSTATE;
    let mut payload = Vec::with_capacity(5);
    payload.push(state.state);
    payload.extend_from_slice(&state.challenge.to_le_bytes());

    let frame_len = 1 + payload.len();
    let max_frame = limits.max_frame_for_opcode(opcode);
    if frame_len > max_frame {
        return Err(Ed2kEncodeError::new(
            opcode,
            "secident_state_frame_len",
            Ed2kErrorKind::FrameTooLarge {
                size: frame_len,
                limit: max_frame,
            },
        ));
    }

    Ok(RawPacket::new(PR_EMULE, opcode, Bytes::from(payload)))
}
