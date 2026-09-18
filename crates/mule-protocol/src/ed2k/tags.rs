//! Safe, bounded parsing for historical eD2K Type-Length-Value (TLV) tags.

use super::error::{Ed2kErrorKind, Ed2kParseError};
use super::limits::Ed2kLimits;

/// Tag data types.
pub const TAGTYPE_HASH: u8 = 0x01;
/// String tag type (2-byte length + UTF-8/ASCII bytes).
pub const TAGTYPE_STRING: u8 = 0x02;
/// 32-bit unsigned integer tag type.
pub const TAGTYPE_UINT32: u8 = 0x03;
/// 32-bit floating point tag type.
pub const TAGTYPE_FLOAT32: u8 = 0x04;
/// Boolean tag type (1 byte, historical 0x05).
pub const TAGTYPE_BOOL: u8 = 0x05;
/// Boolean array tag type (1 byte, historical 0x06).
pub const TAGTYPE_BOOLARRAY: u8 = 0x06;
/// Raw byte blob tag type (4-byte length + bytes).
pub const TAGTYPE_BLOB: u8 = 0x07;
/// 16-bit unsigned integer tag type.
pub const TAGTYPE_UINT16: u8 = 0x08;
/// 8-bit unsigned integer tag type.
pub const TAGTYPE_UINT8: u8 = 0x09;
/// Byte-sized blob tag type (1-byte length + bytes).
pub const TAGTYPE_BSOB: u8 = 0x0A;
/// 64-bit unsigned integer tag type.
pub const TAGTYPE_UINT64: u8 = 0x0B;
/// Fixed-length string tag type base (TAGTYPE_STR1 = 0x11, len = tag_type - 0x10).
pub const TAGTYPE_STR1: u8 = 0x11;

/// Standard special tag IDs (when tag name length is 1).
pub const TAG_SPECIAL_NAME: u8 = 0x01;
/// Standard special file size tag ID.
pub const TAG_SPECIAL_FILESIZE: u8 = 0x02;
/// Standard special file type tag ID.
pub const TAG_SPECIAL_FILETYPE: u8 = 0x03;
/// Standard special port tag ID (`0x0F` = 15).
pub const TAG_SPECIAL_PORT: u8 = 0x0F;
/// Standard special version tag ID (`0x11` = 17).
pub const TAG_SPECIAL_VERSION: u8 = 0x11;
/// Canonical eD2K / eMule server flags tag ID (`0x20` = 32, CT_SERVER_FLAGS).
pub const TAG_SPECIAL_SERVER_FLAGS: u8 = 0x20;
/// Standard special eMule version tag ID (`0x5A` = 90, SO_EMULEVERSION).
pub const TAG_SPECIAL_EMULE_VERSION: u8 = 0x5A;
/// Standard special miscellaneous options 1 tag ID (historical 0x5B).
pub const TAG_SPECIAL_MISCOPTIONS1: u8 = 0x5B;
/// Standard special miscellaneous options 2 tag ID (historical 0x5C).
pub const TAG_SPECIAL_MISCOPTIONS2: u8 = 0x5C;
/// Canonical eMule UDP ports tag ID (`0xF9` = 249).
pub const TAG_SPECIAL_EMULE_UDPPORTS: u8 = 0xF9;
/// Canonical eMule miscellaneous options 1 tag ID (`0xFA` = 250).
pub const TAG_SPECIAL_EMULE_MISCOPTIONS1: u8 = 0xFA;
/// Canonical eMule version tag ID for server login (`0xFB` = 251, CT_EMULE_VERSION).
pub const TAG_SPECIAL_CT_EMULE_VERSION: u8 = 0xFB;
/// Canonical eMule UDP options / flags tag ID (`0xFB` = 251).
pub const TAG_SPECIAL_EMULE_UDPOPTIONS: u8 = 0xFB;
/// Standard special UDP options / port tag ID (alias to 0xFB).
pub const TAG_SPECIAL_UDPOPTIONS: u8 = 0xFB;
/// Canonical eMule miscellaneous options 2 tag ID (`0xFE` = 254).
pub const TAG_SPECIAL_EMULE_MISCOPTIONS2: u8 = 0xFE;
/// Canonical eMule extension compression tag ID (`0x20` = 32).
pub const TAG_SPECIAL_EMULE_EXT_COMPRESSION: u8 = 0x20;
/// Canonical eMule extension UDP version tag ID (`0x21` = 33).
pub const TAG_SPECIAL_EMULE_EXT_UDPVER: u8 = 0x21;
/// Canonical eMule extension UDP port tag ID (`0x22` = 34).
pub const TAG_SPECIAL_EMULE_EXT_UDPPORT: u8 = 0x22;
/// Canonical eMule extension source exchange tag ID (`0x23` = 35).
pub const TAG_SPECIAL_EMULE_EXT_SOURCEEXCHANGE: u8 = 0x23;
/// Canonical eMule extension comments tag ID (`0x24` = 36).
pub const TAG_SPECIAL_EMULE_EXT_COMMENTS: u8 = 0x24;
/// Canonical eMule extension extended requests tag ID (`0x25` = 37).
pub const TAG_SPECIAL_EMULE_EXT_EXTENDEDREQUEST: u8 = 0x25;
/// Canonical eMule extension compatible client tag ID (`0x27` = 39).
pub const TAG_SPECIAL_EMULE_EXT_COMPATIBLECLIENT: u8 = 0x27;

/// Server capability flag: Zlib TCP compression supported.
pub const SRVCAP_ZLIB: u32 = 0x0001;
/// Server capability flag: Client sends IP in login.
pub const SRVCAP_IP_IN_LOGIN: u32 = 0x0002;
/// Server capability flag: Auxiliary port support.
pub const SRVCAP_AUXPORT: u32 = 0x0004;
/// Server capability flag: Modern tag format supported.
pub const SRVCAP_NEWTAGS: u32 = 0x0008;
/// Server capability flag: Unicode character support.
pub const SRVCAP_UNICODE: u32 = 0x0010;
/// Server capability flag: Files larger than 4 GiB supported.
pub const SRVCAP_LARGEFILES: u32 = 0x0100;
/// Server capability flag: Cryptographic obfuscation supported.
pub const SRVCAP_SUPPORTCRYPT: u32 = 0x0200;
/// Server capability flag: Cryptographic obfuscation requested.
pub const SRVCAP_REQUESTCRYPT: u32 = 0x0400;
/// Server capability flag: Cryptographic obfuscation required.
pub const SRVCAP_REQUIRECRYPT: u32 = 0x0800;

/// Tag name identifier: either a 1-byte special ID or a textual string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TagName {
    /// Special 1-byte standardized tag identifier.
    Id(u8),
    /// Textual tag name string.
    Text(String),
}

impl TagName {
    /// Returns a human-readable display string for this tag name.
    pub fn display_name(&self) -> String {
        match self {
            Self::Id(id) => match *id {
                TAG_SPECIAL_NAME => "name".to_string(),
                TAG_SPECIAL_FILESIZE => "filesize".to_string(),
                TAG_SPECIAL_FILETYPE => "filetype".to_string(),
                TAG_SPECIAL_PORT => "port".to_string(),
                TAG_SPECIAL_VERSION => "version".to_string(),
                TAG_SPECIAL_EMULE_VERSION => "emule_version".to_string(),
                TAG_SPECIAL_EMULE_UDPPORTS => "emule_udp_ports".to_string(),
                TAG_SPECIAL_MISCOPTIONS1 | TAG_SPECIAL_EMULE_MISCOPTIONS1 => "misc_options_1".to_string(),
                TAG_SPECIAL_MISCOPTIONS2 | TAG_SPECIAL_EMULE_MISCOPTIONS2 => "misc_options_2".to_string(),
                TAG_SPECIAL_UDPOPTIONS => "udp_options".to_string(),
                TAG_SPECIAL_EMULE_EXT_COMPRESSION => "compression".to_string(),
                TAG_SPECIAL_EMULE_EXT_UDPVER => "emule_udp_ver".to_string(),
                TAG_SPECIAL_EMULE_EXT_UDPPORT => "emule_udp_port".to_string(),
                TAG_SPECIAL_EMULE_EXT_SOURCEEXCHANGE => "source_exchange".to_string(),
                TAG_SPECIAL_EMULE_EXT_COMMENTS => "comments".to_string(),
                TAG_SPECIAL_EMULE_EXT_EXTENDEDREQUEST => "extended_request".to_string(),
                TAG_SPECIAL_EMULE_EXT_COMPATIBLECLIENT => "compatible_client".to_string(),
                other => format!("special_0x{other:02X}"),
            },
            Self::Text(s) => s.clone(),
        }
    }
}

/// Parsed typed value of an eD2K tag.
#[derive(Debug, Clone, PartialEq)]
pub enum TagValue {
    /// 16-byte MD4 hash.
    Hash([u8; 16]),
    /// UTF-8 or decoded string.
    String(String),
    /// 32-bit unsigned integer.
    UInt32(u32),
    /// 32-bit floating point.
    Float32(f32),
    /// Boolean flag.
    Bool(bool),
    /// Byte buffer blob.
    Blob(Vec<u8>),
    /// 16-bit unsigned integer.
    UInt16(u16),
    /// 8-bit unsigned integer.
    UInt8(u8),
    /// 64-bit unsigned integer.
    UInt64(u64),
}

/// A fully typed eD2K tag.
#[derive(Debug, Clone, PartialEq)]
pub struct Ed2kTag {
    /// Tag identifier or name.
    pub name: TagName,
    /// Typed tag payload value.
    pub value: TagValue,
}

/// Raw representation of an unknown or unrecognized tag to ensure no data loss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTag {
    /// Raw type byte.
    pub tag_type: u8,
    /// Tag identifier or name.
    pub name: TagName,
    /// Unparsed raw payload bytes.
    pub raw_bytes: Vec<u8>,
}

/// Helper reader for safe tag extraction from a byte slice.
pub struct TagReader<'a> {
    buf: &'a [u8],
    pos: usize,
    protocol: u8,
    opcode: Option<u8>,
}

impl<'a> TagReader<'a> {
    /// Creates a new `TagReader` starting at `offset`.
    pub fn new(buf: &'a [u8], offset: usize, protocol: u8, opcode: Option<u8>) -> Self {
        Self {
            buf,
            pos: offset,
            protocol,
            opcode,
        }
    }

    /// Returns the current read position offset.
    #[inline]
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Returns the number of remaining bytes.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    /// Reads exactly `n` bytes from the buffer.
    pub fn read_bytes(
        &mut self,
        n: usize,
        context: &'static str,
    ) -> Result<&'a [u8], Ed2kParseError> {
        let available = self.remaining();
        if available < n {
            return Err(Ed2kParseError::new(
                self.protocol,
                self.opcode,
                self.pos,
                context,
                Ed2kErrorKind::UnexpectedEof {
                    needed: n,
                    available,
                },
            ));
        }
        let start = self.pos;
        self.pos += n;
        Ok(&self.buf[start..self.pos])
    }

    /// Peeks at the next byte without advancing the reader.
    pub fn peek_u8(&self) -> Option<u8> {
        self.buf.get(self.pos).copied()
    }

    /// Reads a single byte.
    pub fn read_u8(&mut self, context: &'static str) -> Result<u8, Ed2kParseError> {
        let bytes = self.read_bytes(1, context)?;
        Ok(bytes[0])
    }

    /// Reads a little-endian 16-bit unsigned integer.
    pub fn read_u16_le(&mut self, context: &'static str) -> Result<u16, Ed2kParseError> {
        let bytes = self.read_bytes(2, context)?;
        let mut arr = [0u8; 2];
        arr.copy_from_slice(bytes);
        Ok(u16::from_le_bytes(arr))
    }

    /// Reads a little-endian 32-bit unsigned integer.
    pub fn read_u32_le(&mut self, context: &'static str) -> Result<u32, Ed2kParseError> {
        let bytes = self.read_bytes(4, context)?;
        let mut arr = [0u8; 4];
        arr.copy_from_slice(bytes);
        Ok(u32::from_le_bytes(arr))
    }

    /// Reads a little-endian 64-bit unsigned integer.
    pub fn read_u64_le(&mut self, context: &'static str) -> Result<u64, Ed2kParseError> {
        let bytes = self.read_bytes(8, context)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(arr))
    }

    /// Reads a little-endian 32-bit float.
    pub fn read_f32_le(&mut self, context: &'static str) -> Result<f32, Ed2kParseError> {
        let bytes = self.read_bytes(4, context)?;
        let mut arr = [0u8; 4];
        arr.copy_from_slice(bytes);
        Ok(f32::from_le_bytes(arr))
    }

    /// Reads a 16-byte MD4 hash.
    pub fn read_hash(&mut self, context: &'static str) -> Result<[u8; 16], Ed2kParseError> {
        let bytes = self.read_bytes(16, context)?;
        let mut arr = [0u8; 16];
        arr.copy_from_slice(bytes);
        Ok(arr)
    }

    /// Reads a length-prefixed string (16-bit LE length + UTF-8/lossy bytes), enforcing `max_string_bytes`.
    pub fn read_string(
        &mut self,
        max_string_bytes: usize,
        context: &'static str,
    ) -> Result<String, Ed2kParseError> {
        let len = self.read_u16_le(context)? as usize;
        if len > max_string_bytes {
            return Err(Ed2kParseError::new(
                self.protocol,
                self.opcode,
                self.pos - 2,
                context,
                Ed2kErrorKind::OversizedString {
                    size: len,
                    limit: max_string_bytes,
                },
            ));
        }
        let raw = self.read_bytes(len, context)?;
        Ok(String::from_utf8_lossy(raw).into_owned())
    }

    /// Reads a tag name (length 1 = special ID, length > 1 = string).
    pub fn read_tag_name(&mut self, max_string_bytes: usize) -> Result<TagName, Ed2kParseError> {
        let name_len = self.read_u16_le("tag_name_len")? as usize;
        if name_len == 1 {
            let id = self.read_u8("tag_special_id")?;
            Ok(TagName::Id(id))
        } else {
            if name_len > max_string_bytes {
                return Err(Ed2kParseError::new(
                    self.protocol,
                    self.opcode,
                    self.pos - 2,
                    "tag_name",
                    Ed2kErrorKind::OversizedString {
                        size: name_len,
                        limit: max_string_bytes,
                    },
                ));
            }
            let bytes = self.read_bytes(name_len, "tag_name_bytes")?;
            let s = String::from_utf8_lossy(bytes).into_owned();
            Ok(TagName::Text(s))
        }
    }

    /// Parses a single tag from the input stream.
    ///
    /// If the tag type is unknown, it returns `Ok((None, Some(raw_tag)))`.
    /// If recognized, it returns `Ok((Some(ed2k_tag), None))`.
    pub fn read_tag(
        &mut self,
        limits: &Ed2kLimits,
    ) -> Result<(Option<Ed2kTag>, Option<RawTag>), Ed2kParseError> {
        let raw_type = self.read_u8("tag_type")?;
        let (tag_type, name) = if raw_type & 0x80 != 0 {
            let tag_type = raw_type & 0x7F;
            let id = self.read_u8("tag_special_id")?;
            (tag_type, TagName::Id(id))
        } else {
            let name = self.read_tag_name(limits.max_string_bytes)?;
            (raw_type, name)
        };

        match tag_type {
            TAGTYPE_HASH => {
                let hash = self.read_hash("tag_value_hash")?;
                Ok((
                    Some(Ed2kTag {
                        name,
                        value: TagValue::Hash(hash),
                    }),
                    None,
                ))
            }
            TAGTYPE_STRING => {
                let s = self.read_string(limits.max_string_bytes, "tag_value_string")?;
                Ok((
                    Some(Ed2kTag {
                        name,
                        value: TagValue::String(s),
                    }),
                    None,
                ))
            }
            TAGTYPE_UINT32 => {
                let v = self.read_u32_le("tag_value_u32")?;
                Ok((
                    Some(Ed2kTag {
                        name,
                        value: TagValue::UInt32(v),
                    }),
                    None,
                ))
            }
            TAGTYPE_FLOAT32 => {
                let v = self.read_f32_le("tag_value_f32")?;
                Ok((
                    Some(Ed2kTag {
                        name,
                        value: TagValue::Float32(v),
                    }),
                    None,
                ))
            }
            TAGTYPE_BOOL | TAGTYPE_BOOLARRAY => {
                let b = self.read_u8("tag_value_bool")? != 0;
                Ok((
                    Some(Ed2kTag {
                        name,
                        value: TagValue::Bool(b),
                    }),
                    None,
                ))
            }
            TAGTYPE_BLOB => {
                let blob_len = self.read_u32_le("tag_value_blob_len")? as usize;
                if blob_len > limits.max_blob_bytes {
                    return Err(Ed2kParseError::new(
                        self.protocol,
                        self.opcode,
                        self.pos - 4,
                        "tag_value_blob",
                        Ed2kErrorKind::FrameTooLarge {
                            size: blob_len,
                            limit: limits.max_blob_bytes,
                        },
                    ));
                }
                let blob_bytes = self.read_bytes(blob_len, "tag_value_blob_bytes")?;
                Ok((
                    Some(Ed2kTag {
                        name,
                        value: TagValue::Blob(blob_bytes.to_vec()),
                    }),
                    None,
                ))
            }
            TAGTYPE_BSOB => {
                let blob_len = self.read_u8("tag_value_bsob_len")? as usize;
                let blob_bytes = self.read_bytes(blob_len, "tag_value_bsob_bytes")?;
                Ok((
                    Some(Ed2kTag {
                        name,
                        value: TagValue::Blob(blob_bytes.to_vec()),
                    }),
                    None,
                ))
            }
            TAGTYPE_UINT16 => {
                let v = self.read_u16_le("tag_value_u16")?;
                Ok((
                    Some(Ed2kTag {
                        name,
                        value: TagValue::UInt16(v),
                    }),
                    None,
                ))
            }
            TAGTYPE_UINT8 => {
                let v = self.read_u8("tag_value_u8")?;
                Ok((
                    Some(Ed2kTag {
                        name,
                        value: TagValue::UInt8(v),
                    }),
                    None,
                ))
            }
            TAGTYPE_UINT64 => {
                let v = self.read_u64_le("tag_value_u64")?;
                Ok((
                    Some(Ed2kTag {
                        name,
                        value: TagValue::UInt64(v),
                    }),
                    None,
                ))
            }
            t if (0x11..=0x7F).contains(&t) => {
                let len = (t - 0x10) as usize;
                let bytes = self.read_bytes(len, "tag_value_str_fixed")?;
                let s = String::from_utf8_lossy(bytes).into_owned();
                Ok((
                    Some(Ed2kTag {
                        name,
                        value: TagValue::String(s),
                    }),
                    None,
                ))
            }
            unknown => {
                // Return as a RawTag if length cannot be resolved or if invalid
                Err(Ed2kParseError::new(
                    self.protocol,
                    self.opcode,
                    self.pos - 1,
                    "tag_type",
                    Ed2kErrorKind::InvalidTagType { tag_type: unknown },
                ))
            }
        }
    }
}
