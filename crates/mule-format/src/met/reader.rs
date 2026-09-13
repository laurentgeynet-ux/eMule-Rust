use std::io::Read;

use crate::ed2k::hash::Ed2kHash;
use crate::error::FormatError;
use crate::met::tags::*;

const MAX_TAG_STRING_LEN: usize = 65_536;
const MAX_BLOB_LEN: usize = 16 * 1024 * 1024; // 16 MiB

pub fn read_u8<R: Read>(reader: &mut R) -> Result<u8, FormatError> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf)?;
    Ok(buf[0])
}

pub fn read_u16_le<R: Read>(reader: &mut R) -> Result<u16, FormatError> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

pub fn read_u32_le<R: Read>(reader: &mut R) -> Result<u32, FormatError> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn read_u64_le<R: Read>(reader: &mut R) -> Result<u64, FormatError> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

pub fn read_f32_le<R: Read>(reader: &mut R) -> Result<f32, FormatError> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(f32::from_le_bytes(buf))
}

pub fn read_exact_bytes<R: Read>(reader: &mut R, len: usize) -> Result<Vec<u8>, FormatError> {
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

pub fn read_string<R: Read>(reader: &mut R, len: usize) -> Result<String, FormatError> {
    if len > MAX_TAG_STRING_LEN {
        return Err(FormatError::InputTooLarge(len));
    }
    let bytes = read_exact_bytes(reader, len)?;
    // Try UTF-8 first, fallback to lossy if legacy Latin-1
    Ok(String::from_utf8(bytes.clone())
        .unwrap_or_else(|_| bytes.into_iter().map(|b| b as char).collect()))
}

/// Reads a single metadata Tag from an eMule binary stream.
pub fn read_tag<R: Read>(reader: &mut R) -> Result<Tag, FormatError> {
    let raw_type = read_u8(reader)?;

    let (tag_type, name_id, name) = if (raw_type & 0x80) != 0 {
        let tag_type = raw_type & 0x7F;
        let name_id = read_u8(reader)?;
        (tag_type, name_id, String::new())
    } else {
        let tag_type = raw_type;
        let name_len = read_u16_le(reader)? as usize;
        if name_len == 1 {
            let name_id = read_u8(reader)?;
            (tag_type, name_id, String::new())
        } else {
            let name = read_string(reader, name_len)?;
            (tag_type, 0, name)
        }
    };

    let value = match tag_type {
        TAGTYPE_STRING => {
            let len = read_u16_le(reader)? as usize;
            TagValue::String(read_string(reader, len)?)
        }
        TAGTYPE_UINT32 => TagValue::Uint64(read_u32_le(reader)? as u64),
        TAGTYPE_UINT64 => TagValue::Uint64(read_u64_le(reader)?),
        TAGTYPE_UINT16 => TagValue::Uint64(read_u16_le(reader)? as u64),
        TAGTYPE_UINT8 => TagValue::Uint64(read_u8(reader)? as u64),
        TAGTYPE_FLOAT32 => TagValue::Float32(read_f32_le(reader)?),
        TAGTYPE_BOOL => TagValue::Bool(read_u8(reader)? != 0),
        TAGTYPE_HASH16 => {
            let mut h = [0u8; 16];
            reader.read_exact(&mut h)?;
            TagValue::Hash16(Ed2kHash(h))
        }
        TAGTYPE_BLOB => {
            let len = read_u32_le(reader)? as usize;
            if len > MAX_BLOB_LEN {
                return Err(FormatError::InputTooLarge(len));
            }
            TagValue::Blob(read_exact_bytes(reader, len)?)
        }
        TAGTYPE_BSOB => {
            let len = read_u8(reader)? as usize;
            TagValue::Bsob(read_exact_bytes(reader, len)?)
        }
        TAGTYPE_BOOLARRAY => {
            let len = read_u16_le(reader)? as usize;
            let bytes_to_skip = (len / 8) + 1;
            let _ = read_exact_bytes(reader, bytes_to_skip)?;
            TagValue::Blob(Vec::new())
        }
        _ if (TAGTYPE_STR1..=TAGTYPE_STR16).contains(&tag_type) => {
            let len = (tag_type - TAGTYPE_STR1 + 1) as usize;
            TagValue::String(read_string(reader, len)?)
        }
        _ => {
            return Err(FormatError::InvalidMet(format!(
                "Unknown tag type 0x{tag_type:02x}"
            )))
        }
    };

    Ok(Tag {
        name_id,
        name,
        value,
    })
}
