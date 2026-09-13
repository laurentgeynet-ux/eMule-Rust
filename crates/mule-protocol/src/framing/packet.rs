//! Raw packet representation before semantic parsing.

use bytes::Bytes;

use crate::error::FramingError;
use crate::limits::HEADER_SIZE;

/// A raw eD2K wire packet containing protocol byte, opcode byte, and unparsed payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPacket {
    /// Protocol magic byte (e.g. 0xE3, 0xC5, 0xD4).
    pub protocol: u8,
    /// Operation code byte.
    pub opcode: u8,
    /// Unparsed payload data.
    pub payload: Bytes,
}

impl RawPacket {
    /// Creates a new `RawPacket`.
    pub fn new(protocol: u8, opcode: u8, payload: Bytes) -> Self {
        Self {
            protocol,
            opcode,
            payload,
        }
    }

    /// Creates a new `RawPacket` from a vector of bytes.
    pub fn with_vec(protocol: u8, opcode: u8, vec: Vec<u8>) -> Self {
        Self {
            protocol,
            opcode,
            payload: Bytes::from(vec),
        }
    }

    /// Creates a packet with an empty payload.
    pub fn empty(protocol: u8, opcode: u8) -> Self {
        Self {
            protocol,
            opcode,
            payload: Bytes::new(),
        }
    }

    /// Returns the length of the payload in bytes.
    #[inline]
    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }

    /// Returns the eD2K frame length (1 byte for opcode + payload length).
    #[inline]
    pub fn frame_len(&self) -> usize {
        1 + self.payload.len()
    }

    /// Returns the total wire size (5 header bytes + opcode byte + payload length).
    #[inline]
    pub fn total_wire_len(&self) -> usize {
        HEADER_SIZE + self.frame_len()
    }

    /// Validates that the packet size does not violate frame limits.
    pub fn validate_size(&self, max_frame_bytes: usize) -> Result<(), FramingError> {
        let flen = self.frame_len();
        if flen > max_frame_bytes {
            Err(FramingError::FrameTooLarge {
                size: flen,
                limit: max_frame_bytes,
            })
        } else {
            Ok(())
        }
    }
}
