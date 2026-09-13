//! Safe, deterministic eD2K wire encoders with strict resource bounds.

use bytes::Bytes;

use super::error::{Ed2kEncodeError, Ed2kErrorKind};
use super::limits::Ed2kLimits;
use super::opcodes::{OP_FILESTATUS, OP_HELLO, OP_HELLO_ANSWER, OP_SENDINGPART};
use super::packet::{
    expected_mask_bytes, Ed2kFileStatus, Ed2kFileStatusReq, Ed2kHello, Ed2kHelloAnswer,
    Ed2kMessage, Ed2kRequestParts, Ed2kSendingPart,
};
use super::profile::Ed2kCompatibilityProfile;
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
    /// Active compatibility profile for wire opcode selection.
    pub profile: Ed2kCompatibilityProfile,
}

impl Ed2kEncoder {
    /// Creates a new encoder with default production limits and default profile (`Emule050a`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new encoder with an explicit compatibility profile.
    pub fn with_profile(profile: Ed2kCompatibilityProfile) -> Self {
        Self {
            limits: Ed2kLimits::default(),
            profile,
        }
    }

    /// Creates a new encoder with customized limits and default profile.
    pub fn with_limits(limits: Ed2kLimits) -> Self {
        Self {
            limits,
            profile: Ed2kCompatibilityProfile::default(),
        }
    }

    /// Creates a new encoder with customized limits and explicit compatibility profile.
    pub fn with_limits_and_profile(limits: Ed2kLimits, profile: Ed2kCompatibilityProfile) -> Self {
        Self { limits, profile }
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

    /// Encodes an `Ed2kFileStatusReq` message using the encoder's active profile opcode.
    pub fn encode_file_status_req(
        &self,
        req: &Ed2kFileStatusReq,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        let opcode = self.profile.file_status_req_opcode().ok_or_else(|| {
            Ed2kEncodeError::new(
                0,
                "file_status_req_profile",
                Ed2kErrorKind::AmbiguousOpcodeForProfile {
                    opcode: 0,
                    profile: self.profile,
                    reason: "cannot encode file status request under UnknownStrict profile",
                },
            )
        })?;
        self.encode_file_status_req_with_opcode(req, opcode)
    }

    /// Encodes an `Ed2kFileStatusReq` message into a `RawPacket` with a specified opcode.
    pub fn encode_file_status_req_with_opcode(
        &self,
        req: &Ed2kFileStatusReq,
        opcode: u8,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        let mut payload = Vec::with_capacity(16);
        payload.extend_from_slice(&req.file_hash);

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

        let raw_bytes = status.availability_mask.raw_bytes();
        let expected_bitfield_bytes = if status.part_count == 0 {
            0
        } else {
            expected_mask_bytes(status.part_count)
        };

        if raw_bytes.len() != expected_bitfield_bytes {
            return Err(Ed2kEncodeError::new(
                opcode,
                "filestatus_bitfield",
                Ed2kErrorKind::MalformedField {
                    message: format!(
                        "bitfield length {} does not match required {} for part_count {}",
                        raw_bytes.len(),
                        expected_bitfield_bytes,
                        status.part_count
                    ),
                },
            ));
        }

        let mut payload = Vec::with_capacity(18 + expected_bitfield_bytes);
        payload.extend_from_slice(&status.file_hash);
        payload.extend_from_slice(&status.part_count.to_le_bytes());
        payload.extend_from_slice(raw_bytes);

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

    /// Encodes an `Ed2kRequestParts` message using the encoder's active profile opcode.
    pub fn encode_request_parts(
        &self,
        req: &Ed2kRequestParts,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        let opcode = self.profile.request_parts_opcode().ok_or_else(|| {
            Ed2kEncodeError::new(
                0,
                "request_parts_profile",
                Ed2kErrorKind::AmbiguousOpcodeForProfile {
                    opcode: 0,
                    profile: self.profile,
                    reason: "cannot encode request parts under UnknownStrict profile",
                },
            )
        })?;
        self.encode_request_parts_with_opcode(req, opcode)
    }

    /// Encodes an `Ed2kRequestParts` message with a specified opcode.
    pub fn encode_request_parts_with_opcode(
        &self,
        req: &Ed2kRequestParts,
        opcode: u8,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        if req.ranges.len() > 3 {
            return Err(Ed2kEncodeError::new(
                opcode,
                "request_parts_ranges",
                Ed2kErrorKind::MalformedField {
                    message: format!(
                        "number of requested ranges {} exceeds standard limit of 3",
                        req.ranges.len()
                    ),
                },
            ));
        }

        for range in &req.ranges {
            if range.start >= range.end {
                return Err(Ed2kEncodeError::new(
                    opcode,
                    "request_parts_range",
                    Ed2kErrorKind::InvalidBlockOffsets {
                        start: range.start as u64,
                        end: range.end as u64,
                    },
                ));
            }
        }

        let mut payload = Vec::with_capacity(40);
        payload.extend_from_slice(&req.file_hash);

        // 3 start offsets
        for i in 0..3 {
            if let Some(range) = req.ranges.get(i) {
                payload.extend_from_slice(&range.start.to_le_bytes());
            } else {
                payload.extend_from_slice(&0u32.to_le_bytes());
            }
        }

        // 3 end offsets
        for i in 0..3 {
            if let Some(range) = req.ranges.get(i) {
                payload.extend_from_slice(&range.end.to_le_bytes());
            } else {
                payload.extend_from_slice(&0u32.to_le_bytes());
            }
        }

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

    /// Encodes an `Ed2kSendingPart` message (`0x46`).
    pub fn encode_sending_part(
        &self,
        part: &Ed2kSendingPart,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        let opcode = OP_SENDINGPART;

        if part.start_offset >= part.end_offset {
            return Err(Ed2kEncodeError::new(
                opcode,
                "sending_part_offsets",
                Ed2kErrorKind::InvalidBlockOffsets {
                    start: part.start_offset as u64,
                    end: part.end_offset as u64,
                },
            ));
        }

        let expected_len = (part.end_offset - part.start_offset) as usize;
        if part.data.len() != expected_len {
            return Err(Ed2kEncodeError::new(
                opcode,
                "sending_part_payload",
                Ed2kErrorKind::BlockPayloadMismatch {
                    expected: expected_len,
                    actual: part.data.len(),
                },
            ));
        }

        let mut payload = Vec::with_capacity(24 + part.data.len());
        payload.extend_from_slice(&part.file_hash);
        payload.extend_from_slice(&part.start_offset.to_le_bytes());
        payload.extend_from_slice(&part.end_offset.to_le_bytes());
        payload.extend_from_slice(&part.data);

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
            Ed2kMessage::FileStatusReq(req) => self.encode_file_status_req(req),
            Ed2kMessage::FileStatus(status) => self.encode_file_status(status),
            Ed2kMessage::RequestParts(req) => self.encode_request_parts(req),
            Ed2kMessage::SendingPart(part) => self.encode_sending_part(part),
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
