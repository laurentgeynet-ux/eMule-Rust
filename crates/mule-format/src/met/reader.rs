use std::io::{self, Read};

use crate::ed2k::hash::Ed2kHash;
use crate::met::error::MetError;
use crate::met::tags::*;

pub const MAX_TAG_STRING_LEN: usize = 65_536;
pub const MAX_BLOB_LEN: usize = 16 * 1024 * 1024; // 16 MiB

/// A reader wrapper that tracks the exact byte offset into the stream.
pub struct CountingReader<R> {
    inner: R,
    offset: u64,
}

impl<R: Read> CountingReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner, offset: 0 }
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn read_exact_bytes(&mut self, len: usize, context: &'static str) -> Result<Vec<u8>, MetError> {
        let start_offset = self.offset;
        let mut buf = vec![0u8; len];
        match self.inner.read_exact(&mut buf) {
            Ok(()) => {
                self.offset += len as u64;
                Ok(buf)
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Err(MetError::UnexpectedEof {
                offset: start_offset,
                context,
            }),
            Err(e) => Err(MetError::Io {
                offset: start_offset,
                message: e.to_string(),
            }),
        }
    }

    pub fn read_u8(&mut self, context: &'static str) -> Result<u8, MetError> {
        let bytes = self.read_exact_bytes(1, context)?;
        Ok(bytes[0])
    }

    pub fn read_u16_le(&mut self, context: &'static str) -> Result<u16, MetError> {
        let bytes = self.read_exact_bytes(2, context)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_u32_le(&mut self, context: &'static str) -> Result<u32, MetError> {
        let bytes = self.read_exact_bytes(4, context)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_u64_le(&mut self, context: &'static str) -> Result<u64, MetError> {
        let bytes = self.read_exact_bytes(8, context)?;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes);
        Ok(u64::from_le_bytes(buf))
    }

    pub fn read_f32_le(&mut self, context: &'static str) -> Result<f32, MetError> {
        let bytes = self.read_exact_bytes(4, context)?;
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&bytes);
        Ok(f32::from_le_bytes(buf))
    }

    pub fn read_string(&mut self, len: usize, context: &'static str) -> Result<String, MetError> {
        let start_offset = self.offset;
        if len > MAX_TAG_STRING_LEN {
            return Err(MetError::InvalidLength {
                offset: start_offset,
                declared: len as u64,
                maximum: MAX_TAG_STRING_LEN as u64,
                context,
            });
        }
        let bytes = self.read_exact_bytes(len, context)?;
        // Parse UTF-8, or fall back to Latin-1 lossy representation
        Ok(String::from_utf8(bytes.clone())
            .unwrap_or_else(|_| bytes.into_iter().map(|b| b as char).collect()))
    }

    /// Reads a single metadata Tag from the stream with exact offset tracking.
    pub fn read_tag(&mut self) -> Result<Tag, MetError> {
        let tag_offset = self.offset;
        let raw_type = self.read_u8("tag type")?;

        let (tag_type, name_id, name) = if (raw_type & 0x80) != 0 {
            let tag_type = raw_type & 0x7F;
            let name_id = self.read_u8("tag name id")?;
            (tag_type, name_id, String::new())
        } else {
            let tag_type = raw_type;
            let name_len = self.read_u16_le("tag name length")? as usize;
            if name_len == 1 {
                let name_id = self.read_u8("tag name id")?;
                (tag_type, name_id, String::new())
            } else {
                let name = self.read_string(name_len, "tag name string")?;
                (tag_type, 0, name)
            }
        };

        let value = match tag_type {
            TAGTYPE_STRING => {
                let len = self.read_u16_le("string tag length")? as usize;
                TagValue::String(self.read_string(len, "string tag value")?)
            }
            TAGTYPE_UINT32 => TagValue::Uint64(self.read_u32_le("uint32 tag value")? as u64),
            TAGTYPE_UINT64 => TagValue::Uint64(self.read_u64_le("uint64 tag value")?),
            TAGTYPE_UINT16 => TagValue::Uint64(self.read_u16_le("uint16 tag value")? as u64),
            TAGTYPE_UINT8 => TagValue::Uint64(self.read_u8("uint8 tag value")? as u64),
            TAGTYPE_FLOAT32 => TagValue::Float32(self.read_f32_le("float32 tag value")?),
            TAGTYPE_BOOL => TagValue::Bool(self.read_u8("bool tag value")? != 0),
            TAGTYPE_HASH16 => {
                let bytes = self.read_exact_bytes(16, "hash16 tag value")?;
                let mut h = [0u8; 16];
                h.copy_from_slice(&bytes);
                TagValue::Hash16(Ed2kHash(h))
            }
            TAGTYPE_BLOB => {
                let len_offset = self.offset;
                let len = self.read_u32_le("blob tag length")? as usize;
                if len > MAX_BLOB_LEN {
                    return Err(MetError::InvalidLength {
                        offset: len_offset,
                        declared: len as u64,
                        maximum: MAX_BLOB_LEN as u64,
                        context: "blob payload",
                    });
                }
                TagValue::Blob(self.read_exact_bytes(len, "blob tag payload")?)
            }
            TAGTYPE_BSOB => {
                let len = self.read_u8("bsob tag length")? as usize;
                TagValue::Bsob(self.read_exact_bytes(len, "bsob tag payload")?)
            }
            TAGTYPE_BOOLARRAY => {
                let len = self.read_u16_le("boolarray length")? as usize;
                let bytes_to_skip = (len / 8) + 1;
                let _ = self.read_exact_bytes(bytes_to_skip, "boolarray payload")?;
                TagValue::Blob(Vec::new())
            }
            _ if (TAGTYPE_STR1..=TAGTYPE_STR16).contains(&tag_type) => {
                let len = (tag_type - TAGTYPE_STR1 + 1) as usize;
                TagValue::String(self.read_string(len, "compressed string tag value")?)
            }
            _ => {
                return Err(MetError::InvalidTagType {
                    offset: tag_offset,
                    tag_type,
                })
            }
        };

        Ok(Tag {
            name_id,
            name,
            value,
        })
    }
}

/// Helper function to read a Tag from any Read implementation.
pub fn read_tag<R: Read>(reader: &mut R) -> Result<Tag, MetError> {
    let mut cr = CountingReader::new(reader);
    cr.read_tag()
}
