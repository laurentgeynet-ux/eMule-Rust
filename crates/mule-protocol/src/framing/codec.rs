// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Zero-copy streaming encoder and decoder for eD2K wire packets.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use super::packet::RawPacket;
use crate::error::FramingError;
use crate::limits::{HEADER_SIZE, MAX_BUFFERED_BYTES, MAX_FRAME_BYTES};
use crate::types::is_valid_protocol;

/// Streaming codec for eD2K framing.
#[derive(Debug, Clone)]
pub struct PacketCodec {
    /// Maximum allowed packet frame size (opcode + payload).
    pub max_frame_bytes: usize,
    /// Maximum allowable bytes in accumulator buffer before overflow disconnect.
    pub max_buffered_bytes: usize,
    /// If true, reject unknown protocol magic bytes.
    pub strict_protocol: bool,
}

impl Default for PacketCodec {
    fn default() -> Self {
        Self {
            max_frame_bytes: MAX_FRAME_BYTES,
            max_buffered_bytes: MAX_BUFFERED_BYTES,
            strict_protocol: true,
        }
    }
}

impl PacketCodec {
    /// Creates a codec with default production limits.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a codec with customized frame and buffer limits.
    pub fn with_limits(max_frame_bytes: usize, max_buffered_bytes: usize) -> Self {
        Self {
            max_frame_bytes,
            max_buffered_bytes,
            strict_protocol: true,
        }
    }

    /// Configures whether strict protocol byte validation is enforced.
    pub fn strict_protocol(mut self, strict: bool) -> Self {
        self.strict_protocol = strict;
        self
    }

    /// Attempts to decode a single `RawPacket` from the accumulator buffer `src`.
    ///
    /// - Returns `Ok(Some(packet))` when a full frame is decoded and consumed from `src`.
    /// - Returns `Ok(None)` when `src` does not yet contain a complete frame (need more bytes).
    /// - Returns `Err(FramingError)` if protocol limits, invalid sizes, or buffer overflow occur.
    pub fn decode(&self, src: &mut BytesMut) -> Result<Option<RawPacket>, FramingError> {
        // Enforce maximum buffer capacity to prevent unconstrained memory accumulation.
        if src.len() > self.max_buffered_bytes {
            return Err(FramingError::BufferOverflow {
                buffered: src.len(),
                limit: self.max_buffered_bytes,
            });
        }

        // Need at least 5 header bytes (1 byte protocol + 4 bytes LE length).
        if src.len() < HEADER_SIZE {
            return Ok(None);
        }

        let protocol = src[0];
        if self.strict_protocol && !is_valid_protocol(protocol) {
            return Err(FramingError::UnsupportedProtocol { protocol });
        }

        // Read 4-byte little-endian frame length without consuming yet.
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&src[1..5]);
        let frame_len = u32::from_le_bytes(len_bytes) as usize;

        // In eD2K, frame_len must be at least 1 byte (for the opcode).
        if frame_len < 1 {
            return Err(FramingError::FrameTooSmall { size: frame_len });
        }

        if frame_len > self.max_frame_bytes {
            return Err(FramingError::FrameTooLarge {
                size: frame_len,
                limit: self.max_frame_bytes,
            });
        }

        let total_required = HEADER_SIZE + frame_len;
        if src.len() < total_required {
            // Partial packet: wait for remaining bytes.
            return Ok(None);
        }

        // Advance past header (5 bytes).
        src.advance(HEADER_SIZE);

        // First byte of frame is opcode.
        let opcode = src.split_to(1)[0];

        // Remaining bytes of frame are payload.
        let payload_len = frame_len - 1;
        let payload = src.split_to(payload_len).freeze();

        Ok(Some(RawPacket {
            protocol,
            opcode,
            payload,
        }))
    }

    /// Encodes a `RawPacket` into `dst`.
    pub fn encode(&self, packet: &RawPacket, dst: &mut BytesMut) -> Result<(), FramingError> {
        let frame_len = packet.frame_len();
        if frame_len > self.max_frame_bytes {
            return Err(FramingError::FrameTooLarge {
                size: frame_len,
                limit: self.max_frame_bytes,
            });
        }

        dst.reserve(HEADER_SIZE + frame_len);
        dst.put_u8(packet.protocol);
        dst.put_u32_le(frame_len as u32);
        dst.put_u8(packet.opcode);
        dst.put_slice(&packet.payload);

        Ok(())
    }

    /// Encodes a `RawPacket` directly into a new immutable `Bytes` buffer.
    pub fn encode_to_bytes(&self, packet: &RawPacket) -> Result<Bytes, FramingError> {
        let total = packet.total_wire_len();
        let mut buf = BytesMut::with_capacity(total);
        self.encode(packet, &mut buf)?;
        Ok(buf.freeze())
    }
}
