//! Safe, deterministic eD2K wire encoders with strict resource bounds.

use bytes::Bytes;

use super::error::{Ed2kEncodeError, Ed2kErrorKind};
use super::limits::Ed2kLimits;
use super::opcodes::{OP_FILESTATUS, OP_HELLO, OP_HELLO_ANSWER};
use super::packet::{Ed2kFileStatus, Ed2kHello, Ed2kHelloAnswer, Ed2kMessage};
use super::protocol::PR_ED2K;
use super::tags::{
    Ed2kTag, RawTag, TagName, TagValue, TAGTYPE_BLOB, TAGTYPE_BOOL, TAGTYPE_FLOAT32, TAGTYPE_HASH,
    TAGTYPE_STRING, TAGTYPE_UINT16, TAGTYPE_UINT32, TAGTYPE_UINT64, TAGTYPE_UINT8,
};
use crate::framing::RawPacket;
use crate::limits::HEADER_SIZE;

/// Deterministic wire encoder for historical eD2K protocol messages.
#[derive(Debug, Clone, Default)]
pub struct Ed2kEncoder {
    /// Safety limits and resource boundaries.
    pub limits: Ed2kLimits,
}

impl Ed2kEncoder {
    /// Creates a new encoder with default production limits.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new encoder with customized limits.
    pub fn with_limits(limits: Ed2kLimits) -> Self {
        Self { limits }
    }

    /// Encodes a `TagName` into the destination buffer.
    pub fn encode_tag_name(
        &self,
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
                if bytes.len() > self.limits.max_string_bytes {
                    return Err(Ed2kEncodeError::new(
                        opcode,
                        "tag_name",
                        Ed2kErrorKind::OversizedString {
                            size: bytes.len(),
                            limit: self.limits.max_string_bytes,
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
        &self,
        opcode: u8,
        tag: &Ed2kTag,
        dst: &mut Vec<u8>,
    ) -> Result<(), Ed2kEncodeError> {
        match &tag.value {
            TagValue::Hash(h) => {
                dst.push(TAGTYPE_HASH);
                self.encode_tag_name(opcode, &tag.name, dst)?;
                dst.extend_from_slice(h);
            }
            TagValue::String(s) => {
                let bytes = s.as_bytes();
                if bytes.len() > self.limits.max_string_bytes {
                    return Err(Ed2kEncodeError::new(
                        opcode,
                        "tag_value_string",
                        Ed2kErrorKind::OversizedString {
                            size: bytes.len(),
                            limit: self.limits.max_string_bytes,
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
                self.encode_tag_name(opcode, &tag.name, dst)?;
                dst.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
                dst.extend_from_slice(bytes);
            }
            TagValue::UInt32(v) => {
                dst.push(TAGTYPE_UINT32);
                self.encode_tag_name(opcode, &tag.name, dst)?;
                dst.extend_from_slice(&v.to_le_bytes());
            }
            TagValue::Float32(v) => {
                dst.push(TAGTYPE_FLOAT32);
                self.encode_tag_name(opcode, &tag.name, dst)?;
                dst.extend_from_slice(&v.to_le_bytes());
            }
            TagValue::Bool(b) => {
                dst.push(TAGTYPE_BOOL);
                self.encode_tag_name(opcode, &tag.name, dst)?;
                dst.push(if *b { 1 } else { 0 });
            }
            TagValue::Blob(b) => {
                if b.len() > self.limits.max_blob_bytes {
                    return Err(Ed2kEncodeError::new(
                        opcode,
                        "tag_value_blob",
                        Ed2kErrorKind::FrameTooLarge {
                            size: b.len(),
                            limit: self.limits.max_blob_bytes,
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
                self.encode_tag_name(opcode, &tag.name, dst)?;
                dst.extend_from_slice(&(b.len() as u32).to_le_bytes());
                dst.extend_from_slice(b);
            }
            TagValue::UInt16(v) => {
                dst.push(TAGTYPE_UINT16);
                self.encode_tag_name(opcode, &tag.name, dst)?;
                dst.extend_from_slice(&v.to_le_bytes());
            }
            TagValue::UInt8(v) => {
                dst.push(TAGTYPE_UINT8);
                self.encode_tag_name(opcode, &tag.name, dst)?;
                dst.push(*v);
            }
            TagValue::UInt64(v) => {
                dst.push(TAGTYPE_UINT64);
                self.encode_tag_name(opcode, &tag.name, dst)?;
                dst.extend_from_slice(&v.to_le_bytes());
            }
        }
        Ok(())
    }

    /// Encodes a raw preserved unknown tag into the destination buffer.
    pub fn encode_raw_tag(
        &self,
        opcode: u8,
        tag: &RawTag,
        dst: &mut Vec<u8>,
    ) -> Result<(), Ed2kEncodeError> {
        if tag.raw_bytes.len() > self.limits.max_blob_bytes {
            return Err(Ed2kEncodeError::new(
                opcode,
                "raw_tag_bytes",
                Ed2kErrorKind::FrameTooLarge {
                    size: tag.raw_bytes.len(),
                    limit: self.limits.max_blob_bytes,
                },
            ));
        }
        dst.push(tag.tag_type);
        self.encode_tag_name(opcode, &tag.name, dst)?;
        dst.extend_from_slice(&tag.raw_bytes);
        Ok(())
    }

    /// Encodes an `Ed2kHello` message into a `RawPacket`.
    pub fn encode_hello(&self, hello: &Ed2kHello) -> Result<RawPacket, Ed2kEncodeError> {
        let opcode = OP_HELLO;
        let total_tags = hello.tags.len().saturating_add(hello.unknown_tags.len());
        if total_tags > self.limits.max_tags {
            return Err(Ed2kEncodeError::new(
                opcode,
                "hello_tag_count",
                Ed2kErrorKind::TooManyTags {
                    count: total_tags,
                    limit: self.limits.max_tags,
                },
            ));
        }

        let mut payload = Vec::with_capacity(64);
        payload.extend_from_slice(&hello.user_hash);
        payload.extend_from_slice(&hello.client_id.to_le_bytes());
        payload.extend_from_slice(&hello.tcp_port.to_le_bytes());
        payload.extend_from_slice(&(total_tags as u32).to_le_bytes());

        for tag in &hello.tags {
            self.encode_tag(opcode, tag, &mut payload)?;
        }
        for raw in &hello.unknown_tags {
            self.encode_raw_tag(opcode, raw, &mut payload)?;
        }

        // Standard eD2K handshake always includes server IP (4 bytes LE) and port (2 bytes LE)
        payload.extend_from_slice(&hello.server_ip.to_le_bytes());
        payload.extend_from_slice(&hello.server_port.to_le_bytes());

        let frame_len = 1 + payload.len();
        let max_frame = self.limits.max_frame_for_opcode(opcode);
        if frame_len > max_frame {
            return Err(Ed2kEncodeError::new(
                opcode,
                "frame_len",
                Ed2kErrorKind::FrameTooLarge {
                    size: frame_len,
                    limit: max_frame,
                },
            ));
        }

        Ok(RawPacket::new(PR_ED2K, opcode, Bytes::from(payload)))
    }

    /// Encodes an `Ed2kHelloAnswer` message into a `RawPacket`.
    pub fn encode_hello_answer(
        &self,
        answer: &Ed2kHelloAnswer,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        let opcode = OP_HELLO_ANSWER;
        let total_tags = answer.tags.len().saturating_add(answer.unknown_tags.len());
        if total_tags > self.limits.max_tags {
            return Err(Ed2kEncodeError::new(
                opcode,
                "hello_answer_tag_count",
                Ed2kErrorKind::TooManyTags {
                    count: total_tags,
                    limit: self.limits.max_tags,
                },
            ));
        }

        let mut payload = Vec::with_capacity(64);
        payload.extend_from_slice(&answer.user_hash);
        payload.extend_from_slice(&answer.client_id.to_le_bytes());
        payload.extend_from_slice(&answer.tcp_port.to_le_bytes());
        payload.extend_from_slice(&(total_tags as u32).to_le_bytes());

        for tag in &answer.tags {
            self.encode_tag(opcode, tag, &mut payload)?;
        }
        for raw in &answer.unknown_tags {
            self.encode_raw_tag(opcode, raw, &mut payload)?;
        }

        // Standard eD2K handshake always includes server IP (4 bytes LE) and port (2 bytes LE)
        payload.extend_from_slice(&answer.server_ip.to_le_bytes());
        payload.extend_from_slice(&answer.server_port.to_le_bytes());


        let frame_len = 1 + payload.len();
        let max_frame = self.limits.max_frame_for_opcode(opcode);
        if frame_len > max_frame {
            return Err(Ed2kEncodeError::new(
                opcode,
                "frame_len",
                Ed2kErrorKind::FrameTooLarge {
                    size: frame_len,
                    limit: max_frame,
                },
            ));
        }

        Ok(RawPacket::new(PR_ED2K, opcode, Bytes::from(payload)))
    }

    /// Encodes an `Ed2kFileStatus` message into a `RawPacket`.
    pub fn encode_file_status(
        &self,
        status: &Ed2kFileStatus,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        let opcode = OP_FILESTATUS;
        if (status.part_count as usize) > self.limits.max_parts {
            return Err(Ed2kEncodeError::new(
                opcode,
                "filestatus_part_count",
                Ed2kErrorKind::MalformedField {
                    message: format!(
                        "part_count {} exceeds limit {}",
                        status.part_count, self.limits.max_parts
                    ),
                },
            ));
        }

        let expected_bitfield_bytes = if status.part_count == 0 {
            0
        } else {
            (status.part_count as usize).div_ceil(8)
        };

        if status.status_bitfield.len() != expected_bitfield_bytes {
            return Err(Ed2kEncodeError::new(
                opcode,
                "filestatus_bitfield",
                Ed2kErrorKind::MalformedField {
                    message: format!(
                        "bitfield length {} does not match required {} for part_count {}",
                        status.status_bitfield.len(),
                        expected_bitfield_bytes,
                        status.part_count
                    ),
                },
            ));
        }

        let mut payload = Vec::with_capacity(18 + expected_bitfield_bytes);
        payload.extend_from_slice(&status.file_hash);
        payload.extend_from_slice(&status.part_count.to_le_bytes());
        payload.extend_from_slice(&status.status_bitfield);

        let frame_len = 1 + payload.len();
        let max_frame = self.limits.max_frame_for_opcode(opcode);
        if frame_len > max_frame {
            return Err(Ed2kEncodeError::new(
                opcode,
                "frame_len",
                Ed2kErrorKind::FrameTooLarge {
                    size: frame_len,
                    limit: max_frame,
                },
            ));
        }

        Ok(RawPacket::new(PR_ED2K, opcode, Bytes::from(payload)))
    }

    /// Encodes any `Ed2kMessage` into its `RawPacket` representation.
    pub fn encode_message(&self, msg: &Ed2kMessage) -> Result<RawPacket, Ed2kEncodeError> {
        match msg {
            Ed2kMessage::Hello(hello) => self.encode_hello(hello),
            Ed2kMessage::HelloAnswer(answer) => self.encode_hello_answer(answer),
            Ed2kMessage::FileStatus(status) => self.encode_file_status(status),
            Ed2kMessage::Raw(raw) => {
                let frame_len = raw.frame_len();
                let max_frame = self.limits.max_frame_for_opcode(raw.opcode);
                if frame_len > max_frame {
                    return Err(Ed2kEncodeError::new(
                        raw.opcode,
                        "raw_frame_len",
                        Ed2kErrorKind::FrameTooLarge {
                            size: frame_len,
                            limit: max_frame,
                        },
                    ));
                }
                Ok(raw.clone())
            }
        }
    }

    /// Encodes any `Ed2kMessage` directly into a wire-ready byte buffer (5-byte header + frame).
    pub fn encode_to_bytes(&self, msg: &Ed2kMessage) -> Result<Bytes, Ed2kEncodeError> {
        let packet = self.encode_message(msg)?;
        let frame_len = packet.frame_len();
        let total = HEADER_SIZE + frame_len;
        let mut buf = Vec::with_capacity(total);
        buf.push(packet.protocol);
        buf.extend_from_slice(&(frame_len as u32).to_le_bytes());
        buf.push(packet.opcode);
        buf.extend_from_slice(&packet.payload);
        Ok(Bytes::from(buf))
    }
}
