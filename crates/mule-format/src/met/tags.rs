use crate::ed2k::hash::Ed2kHash;
use std::fmt;

// Tag types
pub const TAGTYPE_HASH16: u8 = 0x01;
pub const TAGTYPE_STRING: u8 = 0x02;
pub const TAGTYPE_UINT32: u8 = 0x03;
pub const TAGTYPE_FLOAT32: u8 = 0x04;
pub const TAGTYPE_BOOL: u8 = 0x05;
pub const TAGTYPE_BOOLARRAY: u8 = 0x06;
pub const TAGTYPE_BLOB: u8 = 0x07;
pub const TAGTYPE_UINT16: u8 = 0x08;
pub const TAGTYPE_UINT8: u8 = 0x09;
pub const TAGTYPE_BSOB: u8 = 0x0A;
pub const TAGTYPE_UINT64: u8 = 0x0B;

pub const TAGTYPE_STR1: u8 = 0x11;
pub const TAGTYPE_STR16: u8 = 0x20;

// Known file tag IDs (FT_*)
pub const FT_FILENAME: u8 = 0x01;
pub const FT_FILESIZE: u8 = 0x02;
pub const FT_FILETYPE: u8 = 0x03;
pub const FT_FILEFORMAT: u8 = 0x04;
pub const FT_LASTSEENCOMPLETE: u8 = 0x05;
pub const FT_TRANSFERRED: u8 = 0x08;
pub const FT_GAPSTART: u8 = 0x09;
pub const FT_GAPEND: u8 = 0x0A;
pub const FT_PARTFILENAME: u8 = 0x12;
pub const FT_MAXSOURCES: u8 = 0x13;
pub const FT_STATUS: u8 = 0x14;
pub const FT_SOURCES: u8 = 0x15;
pub const FT_PERMISSIONS: u8 = 0x16;
pub const FT_DLPRIORITY: u8 = 0x18;
pub const FT_ULPRIORITY: u8 = 0x19;
pub const FT_CORRUPTEDPARTS: u8 = 0x24;
pub const FT_AICH_HASH: u8 = 0x27;
pub const FT_FILESIZE_HI: u8 = 0x3A;
pub const FT_CATEGORY: u8 = 0x53;

// Known server tag IDs (SN_*)
pub const SN_NAME: u8 = 0x01;
pub const SN_DESCRIPTION: u8 = 0x0B;
pub const SN_PING: u8 = 0x0C;
pub const SN_FAIL: u8 = 0x0D;
pub const SN_PREFERENCE: u8 = 0x0E;
pub const SN_PORT: u8 = 0x0F;
pub const SN_IP: u8 = 0x86;
pub const SN_DYNIP: u8 = 0x85;
pub const SN_VERSION: u8 = 0x88;
pub const SN_LASTPING: u8 = 0x87;
pub const SN_MAXUSERS: u8 = 0x90;
pub const SN_SOFTFILES: u8 = 0x91;
pub const SN_HARDFILES: u8 = 0x92;

/// Tag value payload.
#[derive(Debug, Clone, PartialEq)]
pub enum TagValue {
    String(String),
    Uint64(u64),
    Float32(f32),
    Hash16(Ed2kHash),
    Blob(Vec<u8>),
    Bsob(Vec<u8>),
    Bool(bool),
}

/// A parsed metadata tag from eMule binary formats (.met).
#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    pub name_id: u8,
    pub name: String,
    pub value: TagValue,
}

impl Tag {
    pub fn new_id(name_id: u8, value: TagValue) -> Self {
        Self {
            name_id,
            name: String::new(),
            value,
        }
    }

    pub fn new_named(name: impl Into<String>, value: TagValue) -> Self {
        Self {
            name_id: 0,
            name: name.into(),
            value,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match &self.value {
            TagValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self.value {
            TagValue::Uint64(v) => Some(v),
            _ => None,
        }
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.name_id != 0 {
            write!(f, "Tag(0x{:02x}, {:?})", self.name_id, self.value)
        } else {
            write!(f, "Tag('{}', {:?})", self.name, self.value)
        }
    }
}
