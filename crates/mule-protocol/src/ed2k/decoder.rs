//! Safe, read-only eD2K wire decoders with strict bounds and context tracking.

use bytes::Bytes;

use super::error::{Ed2kErrorKind, Ed2kParseError};
use super::limits::{Ed2kLimits, ED2K_MAX_PARTS};
use super::opcodes::{OP_FILESTATUS, OP_HELLO, OP_HELLO_ANSWER};
use super::packet::{Ed2kFileStatus, Ed2kHello, Ed2kHelloAnswer, Ed2kMessage};
use super::protocol::{is_valid_ed2k_protocol, PR_ED2K, PR_EMULE_COMPRESSED};
use super::tags::TagReader;
use crate::framing::RawPacket;
use crate::limits::HEADER_SIZE;

/// Read-only wire decoder for eD2K protocol frames.
#[derive(Debug, Clone, Default)]
pub struct Ed2kDecoder {
    /// Safety limits and resource bounds.
    pub limits: Ed2kLimits,
}

impl Ed2kDecoder {
    /// Creates a new decoder with default limits.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new decoder with customized limits.
    pub fn with_limits(limits: Ed2kLimits) -> Self {
        Self { limits }
    }

    /// Decodes the 5-byte header of an eD2K packet without consuming payload.
    ///
    /// Returns `(protocol, packet_len, opcode)`.
    pub fn decode_header(&self, src: &[u8]) -> Result<(u8, u32, u8), Ed2kParseError> {
        if src.len() < HEADER_SIZE {
            return Err(Ed2kParseError::new(
                0,
                None,
                0,
                "header",
                Ed2kErrorKind::UnexpectedEof {
                    needed: HEADER_SIZE,
                    available: src.len(),
                },
            ));
        }

        let protocol = src[0];
        if protocol == PR_EMULE_COMPRESSED {
            return Err(Ed2kParseError::new(
                protocol,
                None,
                0,
                "protocol_byte",
                Ed2kErrorKind::CompressedProtocolUnsupported,
            ));
        }

        if !is_valid_ed2k_protocol(protocol) {
            return Err(Ed2kParseError::new(
                protocol,
                None,
                0,
                "protocol_byte",
                Ed2kErrorKind::UnsupportedProtocol { byte: protocol },
            ));
        }

        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&src[1..5]);
        let packet_len = u32::from_le_bytes(len_bytes);

        if packet_len < 1 {
            return Err(Ed2kParseError::new(
                protocol,
                None,
                1,
                "packet_length",
                Ed2kErrorKind::FrameTooSmall {
                    size: packet_len as usize,
                },
            ));
        }

        // Check if opcode byte is available
        if src.len() < HEADER_SIZE + 1 {
            return Err(Ed2kParseError::new(
                protocol,
                None,
                5,
                "opcode",
                Ed2kErrorKind::UnexpectedEof {
                    needed: HEADER_SIZE + 1,
                    available: src.len(),
                },
            ));
        }

        let opcode = src[5];
        Ok((protocol, packet_len, opcode))
    }

    /// Decodes a complete packet from a byte slice into an `Ed2kMessage`.
    pub fn decode_packet(&self, src: &[u8]) -> Result<Ed2kMessage, Ed2kParseError> {
        let (protocol, packet_len, opcode) = self.decode_header(src)?;
        let frame_len = packet_len as usize;

        let max_frame = self.limits.max_frame_for_opcode(opcode);
        if frame_len > max_frame {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                1,
                "frame_len",
                Ed2kErrorKind::FrameTooLarge {
                    size: frame_len,
                    limit: max_frame,
                },
            ));
        }

        let total_required = HEADER_SIZE + frame_len;
        if src.len() < total_required {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                src.len(),
                "frame_payload",
                Ed2kErrorKind::UnexpectedEof {
                    needed: total_required,
                    available: src.len(),
                },
            ));
        }

        // Payload slice excludes header (5 bytes) and opcode (1 byte)
        let payload = &src[HEADER_SIZE + 1..total_required];

        match (protocol, opcode) {
            (PR_ED2K, OP_HELLO) => {
                let hello = self.decode_hello(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::Hello(hello))
            }
            (PR_ED2K, OP_HELLO_ANSWER) => {
                let answer =
                    self.decode_hello_answer(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::HelloAnswer(answer))
            }
            (PR_ED2K, OP_FILESTATUS) => {
                let status = self.decode_file_status(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::FileStatus(status))
            }
            _ => {
                // Return as unparsed RawPacket for passive observation of other opcodes
                let raw_packet = RawPacket::new(protocol, opcode, Bytes::copy_from_slice(payload));
                Ok(Ed2kMessage::Raw(raw_packet))
            }
        }
    }

    /// Decodes a pre-framed `RawPacket` into an `Ed2kMessage`.
    pub fn decode_raw_packet(&self, packet: &RawPacket) -> Result<Ed2kMessage, Ed2kParseError> {
        if packet.protocol == PR_EMULE_COMPRESSED {
            return Err(Ed2kParseError::new(
                packet.protocol,
                Some(packet.opcode),
                0,
                "protocol_byte",
                Ed2kErrorKind::CompressedProtocolUnsupported,
            ));
        }
        if !is_valid_ed2k_protocol(packet.protocol) {
            return Err(Ed2kParseError::new(
                packet.protocol,
                Some(packet.opcode),
                0,
                "protocol_byte",
                Ed2kErrorKind::UnsupportedProtocol {
                    byte: packet.protocol,
                },
            ));
        }
        let frame_len = packet.frame_len();
        let max_frame = self.limits.max_frame_for_opcode(packet.opcode);
        if frame_len > max_frame {
            return Err(Ed2kParseError::new(
                packet.protocol,
                Some(packet.opcode),
                1,
                "frame_len",
                Ed2kErrorKind::FrameTooLarge {
                    size: frame_len,
                    limit: max_frame,
                },
            ));
        }

        match (packet.protocol, packet.opcode) {
            (PR_ED2K, OP_HELLO) => {
                let hello = self.decode_hello(packet.protocol, packet.opcode, &packet.payload, 0)?;
                Ok(Ed2kMessage::Hello(hello))
            }
            (PR_ED2K, OP_HELLO_ANSWER) => {
                let answer = self.decode_hello_answer(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::HelloAnswer(answer))
            }
            (PR_ED2K, OP_FILESTATUS) => {
                let status = self.decode_file_status(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::FileStatus(status))
            }
            _ => Ok(Ed2kMessage::Raw(packet.clone())),
        }
    }

    /// Decodes an `OP_HELLO` payload.
    pub fn decode_hello(

        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kHello, Ed2kParseError> {
        let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));

        let user_hash = reader.read_hash("hello_user_hash")?;
        let client_id = reader.read_u32_le("hello_client_id")?;
        let tcp_port = reader.read_u16_le("hello_tcp_port")?;
        let tag_count = reader.read_u32_le("hello_tag_count")? as usize;

        if tag_count > self.limits.max_tags {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + reader.pos() - 4,
                "hello_tag_count",
                Ed2kErrorKind::TooManyTags {
                    count: tag_count,
                    limit: self.limits.max_tags,
                },
            ));
        }

        let mut tags = Vec::with_capacity(tag_count.min(64));
        let mut unknown_tags = Vec::new();

        for _ in 0..tag_count {
            let (typed_opt, raw_opt) = reader.read_tag(&self.limits)?;
            if let Some(tag) = typed_opt {
                tags.push(tag);
            }
            if let Some(raw) = raw_opt {
                unknown_tags.push(raw);
            }
        }

        let (server_ip, server_port) = if reader.remaining() >= 6 {
            let ip = reader.read_u32_le("hello_server_ip")?;
            let port = reader.read_u16_le("hello_server_port")?;
            (ip, port)
        } else {
            (0, 0)
        };

        Ok(Ed2kHello {
            user_hash,
            client_id,
            tcp_port,
            tags,
            unknown_tags,
            server_ip,
            server_port,
        })
    }

    /// Decodes an `OP_HELLO_ANSWER` payload.
    pub fn decode_hello_answer(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kHelloAnswer, Ed2kParseError> {
        let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));

        let user_hash = reader.read_hash("hello_answer_user_hash")?;
        let client_id = reader.read_u32_le("hello_answer_client_id")?;
        let tcp_port = reader.read_u16_le("hello_answer_tcp_port")?;
        let tag_count = reader.read_u32_le("hello_answer_tag_count")? as usize;

        if tag_count > self.limits.max_tags {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + reader.pos() - 4,
                "hello_answer_tag_count",
                Ed2kErrorKind::TooManyTags {
                    count: tag_count,
                    limit: self.limits.max_tags,
                },
            ));
        }

        let mut tags = Vec::with_capacity(tag_count.min(64));
        let mut unknown_tags = Vec::new();

        for _ in 0..tag_count {
            let (typed_opt, raw_opt) = reader.read_tag(&self.limits)?;
            if let Some(tag) = typed_opt {
                tags.push(tag);
            }
            if let Some(raw) = raw_opt {
                unknown_tags.push(raw);
            }
        }

        let (server_ip, server_port) = if reader.remaining() >= 6 {
            let ip = reader.read_u32_le("hello_answer_server_ip")?;
            let port = reader.read_u16_le("hello_answer_server_port")?;
            (ip, port)
        } else {
            (0, 0)
        };

        Ok(Ed2kHelloAnswer {
            user_hash,
            client_id,
            tcp_port,
            tags,
            unknown_tags,
            server_ip,
            server_port,
        })
    }

    /// Decodes an `OP_FILESTATUS` payload.
    pub fn decode_file_status(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kFileStatus, Ed2kParseError> {
        let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));

        let file_hash = reader.read_hash("filestatus_file_hash")?;
        let part_count = reader.read_u16_le("filestatus_part_count")?;

        if (part_count as usize) > self.limits.max_parts || (part_count as usize) > ED2K_MAX_PARTS {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + 16,
                "filestatus_part_count",
                Ed2kErrorKind::MalformedField {
                    message: format!(
                        "part_count {part_count} exceeds limit {}",
                        self.limits.max_parts
                    ),
                },
            ));
        }

        let bitfield_bytes_needed = if part_count == 0 {
            0
        } else {
            (part_count as usize).div_ceil(8)
        };

        let bitfield_bytes = if bitfield_bytes_needed > 0 {
            reader
                .read_bytes(bitfield_bytes_needed, "filestatus_bitfield")?
                .to_vec()
        } else {
            Vec::new()
        };

        Ok(Ed2kFileStatus {
            file_hash,
            part_count,
            status_bitfield: bitfield_bytes,
        })
    }
}
