//! Source querying and response structures for eD2K servers.

use crate::ed2k::error::{Ed2kEncodeError, Ed2kErrorKind};
use crate::ed2k::limits::Ed2kLimits;
use crate::ed2k::opcodes::{OP_GETSOURCES, OP_GETSOURCES_OBFU};
use crate::ed2k::protocol::PR_ED2K;
use crate::framing::RawPacket;

/// Request to find sources for a specific file (`OP_GETSOURCES` = `0x19` or `OP_GETSOURCES_OBFU` = `0x23`).
#[derive(Debug, Clone, PartialEq)]
pub struct GetSourcesRequest {
    /// 16-byte MD4 file hash.
    pub file_hash: [u8; 16],
    /// File size in bytes.
    pub file_size: u64,
    /// Whether to request sources with obfuscation parameters (`OP_GETSOURCES_OBFU` = `0x23`).
    pub obfuscation: bool,
}

impl GetSourcesRequest {
    /// Creates a new GetSourcesRequest (legacy plaintext OP_GETSOURCES = 0x19).
    pub fn new(file_hash: [u8; 16], file_size: u64) -> Self {
        Self {
            file_hash,
            file_size,
            obfuscation: false,
        }
    }

    /// Creates a new GetSourcesRequest with obfuscation toggle (0x23 if true, 0x19 if false).
    pub fn with_obfuscation(file_hash: [u8; 16], file_size: u64, obfuscation: bool) -> Self {
        Self {
            file_hash,
            file_size,
            obfuscation,
        }
    }

    /// Encodes the request into a `RawPacket`.
    pub fn encode(&self, limits: &Ed2kLimits) -> Result<RawPacket, Ed2kEncodeError> {
        let opcode = if self.obfuscation {
            OP_GETSOURCES_OBFU
        } else {
            OP_GETSOURCES
        };
        let mut buf = Vec::with_capacity(24);
        buf.extend_from_slice(&self.file_hash);

        if self.file_size <= u32::MAX as u64 {
            buf.extend_from_slice(&(self.file_size as u32).to_le_bytes());
        } else {
            buf.extend_from_slice(&0u32.to_le_bytes());
            buf.extend_from_slice(&self.file_size.to_le_bytes());
        }

        let payload_len = buf.len() + 1;
        if payload_len > limits.max_frame_bytes {
            return Err(Ed2kEncodeError::new(
                opcode,
                "get_sources",
                Ed2kErrorKind::FrameTooLarge {
                    size: payload_len,
                    limit: limits.max_frame_bytes,
                },
            ));
        }

        Ok(RawPacket::with_vec(PR_ED2K, opcode, buf))
    }
}

/// A peer source entry returned by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundSourcePeer {
    /// IPv4 address in network byte order or host integer.
    pub ip: u32,
    /// Port number.
    pub port: u16,
}

impl FoundSourcePeer {
    /// Converts the source address to "A.B.C.D:port" string format.
    pub fn to_address_string(&self) -> String {
        let bytes = self.ip.to_le_bytes();
        format!(
            "{}.{}.{}.{}:{}",
            bytes[0], bytes[1], bytes[2], bytes[3], self.port
        )
    }
}

/// Found sources response from server (`OP_FOUNDSOURCES` = `0x42`).
#[derive(Debug, Clone, PartialEq)]
pub struct FoundSources {
    /// 16-byte MD4 file hash.
    pub file_hash: [u8; 16],
    /// Sources list.
    pub sources: Vec<FoundSourcePeer>,
}

/// A peer source entry with obfuscation flags and optional UserHash (`OP_FOUNDSOURCES_OBFU` = `0x44`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundSourcePeerObfu {
    /// IPv4 address in network byte order / host integer.
    pub ip: u32,
    /// Port number.
    pub port: u16,
    /// Obfuscation settings byte.
    pub obf_settings: u8,
    /// Optional 16-byte user hash (present if obf_settings & 0x80 != 0).
    pub user_hash: Option<[u8; 16]>,
}

impl FoundSourcePeerObfu {
    /// Converts the source address to "A.B.C.D:port" string format.
    pub fn to_address_string(&self) -> String {
        let bytes = self.ip.to_le_bytes();
        format!(
            "{}.{}.{}.{}:{}",
            bytes[0], bytes[1], bytes[2], bytes[3], self.port
        )
    }

    /// Whether this peer supports protocol obfuscation (bit 0).
    #[inline]
    pub fn supports_crypt(&self) -> bool {
        (self.obf_settings & 0x01) != 0
    }

    /// Whether this peer requests protocol obfuscation (bit 1).
    #[inline]
    pub fn requests_crypt(&self) -> bool {
        (self.obf_settings & 0x02) != 0
    }

    /// Whether this peer strictly requires protocol obfuscation (bit 2).
    #[inline]
    pub fn requires_crypt(&self) -> bool {
        (self.obf_settings & 0x04) != 0
    }
}

/// Found sources response with obfuscation from server (`OP_FOUNDSOURCES_OBFU` = `0x44`).
#[derive(Debug, Clone, PartialEq)]
pub struct FoundSourcesObfu {
    /// 16-byte MD4 file hash.
    pub file_hash: [u8; 16],
    /// Sources list with obfuscation parameters.
    pub sources: Vec<FoundSourcePeerObfu>,
}
