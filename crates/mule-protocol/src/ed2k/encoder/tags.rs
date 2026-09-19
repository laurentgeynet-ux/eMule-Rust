//! eD2K Tag and tag name encoders.

use super::super::error::{Ed2kEncodeError, Ed2kErrorKind};
use super::super::limits::Ed2kLimits;
use super::super::tags::{
    Ed2kTag, RawTag, TagName, TagValue, TAGTYPE_BLOB, TAGTYPE_BOOL, TAGTYPE_FLOAT32, TAGTYPE_HASH,
    TAGTYPE_STRING, TAGTYPE_UINT16, TAGTYPE_UINT32, TAGTYPE_UINT64, TAGTYPE_UINT8,
};

/// Encodes a `TagName` into the destination buffer.
pub fn encode_tag_name(
    limits: &Ed2kLimits,
    opcode: u8,
    name: &TagName,
    dst: &mut Vec<u8>,
) -> Result<(), Ed2kEncodeError> {
    match name {
        TagName::Id(id) => {
            dst.extend_from_slice(&1u16.to_le_bytes());
            dst.push(*id);
            Ok(())
        }
        TagName::Text(s) => {
            let bytes = s.as_bytes();
            if bytes.len() > limits.max_string_bytes {
                return Err(Ed2kEncodeError::new(
                    opcode,
                    "tag_name",
                    Ed2kErrorKind::OversizedString {
                        size: bytes.len(),
                        limit: limits.max_string_bytes,
                    },
                ));
            }
            if bytes.len() > u16::MAX as usize {
                return Err(Ed2kEncodeError::new(
                    opcode,
                    "tag_name",
                    Ed2kErrorKind::OversizedString {
                        size: bytes.len(),
                        limit: u16::MAX as usize,
                    },
                ));
            }
            dst.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            dst.extend_from_slice(bytes);
            Ok(())
        }
    }
}

/// Encodes a single recognized `Ed2kTag` into the destination buffer.
pub fn encode_tag(
    limits: &Ed2kLimits,
    opcode: u8,
    tag: &Ed2kTag,
    dst: &mut Vec<u8>,
) -> Result<(), Ed2kEncodeError> {
    match &tag.value {
        TagValue::Hash(h) => {
            dst.push(TAGTYPE_HASH);
            encode_tag_name(limits, opcode, &tag.name, dst)?;
            dst.extend_from_slice(h);
        }
        TagValue::String(s) => {
            let bytes = s.as_bytes();
            if bytes.len() > limits.max_string_bytes {
                return Err(Ed2kEncodeError::new(
                    opcode,
                    "tag_value_string",
                    Ed2kErrorKind::OversizedString {
                        size: bytes.len(),
                        limit: limits.max_string_bytes,
                    },
                ));
            }
            if bytes.len() > u16::MAX as usize {
                return Err(Ed2kEncodeError::new(
                    opcode,
                    "tag_value_string",
                    Ed2kErrorKind::OversizedString {
                        size: bytes.len(),
                        limit: u16::MAX as usize,
                    },
                ));
            }
            dst.push(TAGTYPE_STRING);
            encode_tag_name(limits, opcode, &tag.name, dst)?;
            dst.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            dst.extend_from_slice(bytes);
        }
        TagValue::UInt32(v) => {
            dst.push(TAGTYPE_UINT32);
            encode_tag_name(limits, opcode, &tag.name, dst)?;
            dst.extend_from_slice(&v.to_le_bytes());
        }
        TagValue::Float32(v) => {
            dst.push(TAGTYPE_FLOAT32);
            encode_tag_name(limits, opcode, &tag.name, dst)?;
            dst.extend_from_slice(&v.to_le_bytes());
        }
        TagValue::Bool(b) => {
            dst.push(TAGTYPE_BOOL);
            encode_tag_name(limits, opcode, &tag.name, dst)?;
            dst.push(if *b { 1 } else { 0 });
        }
        TagValue::Blob(b) => {
            if b.len() > limits.max_blob_bytes {
                return Err(Ed2kEncodeError::new(
                    opcode,
                    "tag_value_blob",
                    Ed2kErrorKind::FrameTooLarge {
                        size: b.len(),
                        limit: limits.max_blob_bytes,
                    },
                ));
            }
            if b.len() > u32::MAX as usize {
                return Err(Ed2kEncodeError::new(
                    opcode,
                    "tag_value_blob",
                    Ed2kErrorKind::FrameTooLarge {
                        size: b.len(),
                        limit: u32::MAX as usize,
                    },
                ));
            }
            dst.push(TAGTYPE_BLOB);
            encode_tag_name(limits, opcode, &tag.name, dst)?;
            dst.extend_from_slice(&(b.len() as u32).to_le_bytes());
            dst.extend_from_slice(b);
        }
        TagValue::UInt16(v) => {
            dst.push(TAGTYPE_UINT16);
            encode_tag_name(limits, opcode, &tag.name, dst)?;
            dst.extend_from_slice(&v.to_le_bytes());
        }
        TagValue::UInt8(v) => {
            dst.push(TAGTYPE_UINT8);
            encode_tag_name(limits, opcode, &tag.name, dst)?;
            dst.push(*v);
        }
        TagValue::UInt64(v) => {
            dst.push(TAGTYPE_UINT64);
            encode_tag_name(limits, opcode, &tag.name, dst)?;
            dst.extend_from_slice(&v.to_le_bytes());
        }
    }
    Ok(())
}

/// Encodes a raw preserved unknown tag into the destination buffer.
pub fn encode_raw_tag(
    limits: &Ed2kLimits,
    opcode: u8,
    tag: &RawTag,
    dst: &mut Vec<u8>,
) -> Result<(), Ed2kEncodeError> {
    if tag.raw_bytes.len() > limits.max_blob_bytes {
        return Err(Ed2kEncodeError::new(
            opcode,
            "raw_tag_bytes",
            Ed2kErrorKind::FrameTooLarge {
                size: tag.raw_bytes.len(),
                limit: limits.max_blob_bytes,
            },
        ));
    }
    dst.push(tag.tag_type);
    encode_tag_name(limits, opcode, &tag.name, dst)?;
    dst.extend_from_slice(&tag.raw_bytes);
    Ok(())
}
