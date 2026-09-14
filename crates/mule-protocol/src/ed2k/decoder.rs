//! Safe, read-only eD2K wire decoders with strict bounds and context tracking.

use bytes::Bytes;

use super::error::{Ed2kErrorKind, Ed2kParseError};
use super::limits::{Ed2kLimits, ED2K_MAX_PARTS};
use super::opcodes::{
    OP_FILESTATUS, OP_FILESTATUSREQ, OP_HELLO, OP_HELLO_ANSWER, OP_REQUESTPARTS,
    OP_REQUESTPARTS_EMULE, OP_SENDINGPART, OP_SETREQFILEID_EMULE,
};
use super::packet::{
    expected_mask_bytes, Ed2kBlockRange, Ed2kFileStatus, Ed2kFileStatusReq, Ed2kHello,
    Ed2kHelloAnswer, Ed2kMessage, Ed2kRequestParts, Ed2kSendingPart, PartAvailabilityMask,
};
use super::profile::Ed2kCompatibilityProfile;
use super::protocol::{is_valid_ed2k_protocol, PR_ED2K, PR_EMULE_COMPRESSED};
use super::tags::TagReader;
use crate::framing::RawPacket;
use crate::limits::HEADER_SIZE;

/// Read-only wire decoder for eD2K protocol frames.
#[derive(Debug, Clone, Default)]
pub struct Ed2kDecoder {
    /// Safety limits and resource bounds.
    pub limits: Ed2kLimits,
    /// Active compatibility profile for wire opcode and layout disambiguation.
    pub profile: Ed2kCompatibilityProfile,
}

impl Ed2kDecoder {
    /// Creates a new decoder with default limits and default profile (`Emule050a`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new decoder with an explicit compatibility profile and default limits.
    pub fn with_profile(profile: Ed2kCompatibilityProfile) -> Self {
        Self {
            limits: Ed2kLimits::default(),
            profile,
        }
    }

    /// Creates a new decoder with customized limits and default profile.
    pub fn with_limits(limits: Ed2kLimits) -> Self {
        Self {
            limits,
            profile: Ed2kCompatibilityProfile::default(),
        }
    }

    /// Creates a new decoder with customized limits and explicit compatibility profile.
    pub fn with_limits_and_profile(limits: Ed2kLimits, profile: Ed2kCompatibilityProfile) -> Self {
        Self { limits, profile }
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
            (PR_ED2K, OP_FILESTATUSREQ) | (PR_ED2K, OP_SETREQFILEID_EMULE) => {
                match self.profile {
                    Ed2kCompatibilityProfile::UnknownStrict => {
                        Err(Ed2kParseError::new(
                            protocol,
                            Some(opcode),
                            5,
                            "opcode",
                            Ed2kErrorKind::AmbiguousOpcodeForProfile {
                                opcode,
                                profile: self.profile,
                                reason: "file status request opcode cannot be determined under UnknownStrict profile",
                            },
                        ))
                    }
                    Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
                        if opcode == OP_SETREQFILEID_EMULE {
                            let req = self.decode_file_status_req_with_profile(
                                self.profile,
                                protocol,
                                opcode,
                                payload,
                                HEADER_SIZE + 1,
                            )?;
                            Ok(Ed2kMessage::FileStatusReq(req))
                        } else {
                            let raw_packet =
                                RawPacket::new(protocol, opcode, Bytes::copy_from_slice(payload));
                            Ok(Ed2kMessage::Raw(raw_packet))
                        }
                    }
                    Ed2kCompatibilityProfile::LegacyEdonkey => {
                        if opcode == OP_FILESTATUSREQ {
                            let req = self.decode_file_status_req_with_profile(
                                self.profile,
                                protocol,
                                opcode,
                                payload,
                                HEADER_SIZE + 1,
                            )?;
                            Ok(Ed2kMessage::FileStatusReq(req))
                        } else {
                            let raw_packet =
                                RawPacket::new(protocol, opcode, Bytes::copy_from_slice(payload));
                            Ok(Ed2kMessage::Raw(raw_packet))
                        }
                    }
                }
            }
            (PR_ED2K, OP_FILESTATUS) => {
                let status = self.decode_file_status(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::FileStatus(status))
            }
            (PR_ED2K, OP_REQUESTPARTS) | (PR_ED2K, OP_REQUESTPARTS_EMULE) => {
                match self.profile {
                    Ed2kCompatibilityProfile::UnknownStrict => {
                        Err(Ed2kParseError::new(
                            protocol,
                            Some(opcode),
                            5,
                            "opcode",
                            Ed2kErrorKind::AmbiguousOpcodeForProfile {
                                opcode,
                                profile: self.profile,
                                reason: "request parts opcode cannot be determined under UnknownStrict profile",
                            },
                        ))
                    }
                    Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
                        if opcode == OP_REQUESTPARTS_EMULE {
                            let req = self.decode_request_parts_with_profile(
                                self.profile,
                                protocol,
                                opcode,
                                payload,
                                HEADER_SIZE + 1,
                            )?;
                            Ok(Ed2kMessage::RequestParts(req))
                        } else {
                            let raw_packet =
                                RawPacket::new(protocol, opcode, Bytes::copy_from_slice(payload));
                            Ok(Ed2kMessage::Raw(raw_packet))
                        }
                    }
                    Ed2kCompatibilityProfile::LegacyEdonkey => {
                        if opcode == OP_REQUESTPARTS {
                            let req = self.decode_request_parts_with_profile(
                                self.profile,
                                protocol,
                                opcode,
                                payload,
                                HEADER_SIZE + 1,
                            )?;
                            Ok(Ed2kMessage::RequestParts(req))
                        } else {
                            let raw_packet =
                                RawPacket::new(protocol, opcode, Bytes::copy_from_slice(payload));
                            Ok(Ed2kMessage::Raw(raw_packet))
                        }
                    }
                }
            }
            (PR_ED2K, OP_SENDINGPART) => {
                let part = self.decode_sending_part(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::SendingPart(part))
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
            (PR_ED2K, OP_FILESTATUSREQ) | (PR_ED2K, OP_SETREQFILEID_EMULE) => {
                match self.profile {
                    Ed2kCompatibilityProfile::UnknownStrict => {
                        Err(Ed2kParseError::new(
                            packet.protocol,
                            Some(packet.opcode),
                            0,
                            "opcode",
                            Ed2kErrorKind::AmbiguousOpcodeForProfile {
                                opcode: packet.opcode,
                                profile: self.profile,
                                reason: "file status request opcode cannot be determined under UnknownStrict profile",
                            },
                        ))
                    }
                    Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
                        if packet.opcode == OP_SETREQFILEID_EMULE {
                            let req = self.decode_file_status_req_with_profile(
                                self.profile,
                                packet.protocol,
                                packet.opcode,
                                &packet.payload,
                                0,
                            )?;
                            Ok(Ed2kMessage::FileStatusReq(req))
                        } else {
                            Ok(Ed2kMessage::Raw(packet.clone()))
                        }
                    }
                    Ed2kCompatibilityProfile::LegacyEdonkey => {
                        if packet.opcode == OP_FILESTATUSREQ {
                            let req = self.decode_file_status_req_with_profile(
                                self.profile,
                                packet.protocol,
                                packet.opcode,
                                &packet.payload,
                                0,
                            )?;
                            Ok(Ed2kMessage::FileStatusReq(req))
                        } else {
                            Ok(Ed2kMessage::Raw(packet.clone()))
                        }
                    }
                }
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
            (PR_ED2K, OP_REQUESTPARTS) | (PR_ED2K, OP_REQUESTPARTS_EMULE) => {
                match self.profile {
                    Ed2kCompatibilityProfile::UnknownStrict => {
                        Err(Ed2kParseError::new(
                            packet.protocol,
                            Some(packet.opcode),
                            0,
                            "opcode",
                            Ed2kErrorKind::AmbiguousOpcodeForProfile {
                                opcode: packet.opcode,
                                profile: self.profile,
                                reason: "request parts opcode cannot be determined under UnknownStrict profile",
                            },
                        ))
                    }
                    Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
                        if packet.opcode == OP_REQUESTPARTS_EMULE {
                            let req = self.decode_request_parts_with_profile(
                                self.profile,
                                packet.protocol,
                                packet.opcode,
                                &packet.payload,
                                0,
                            )?;
                            Ok(Ed2kMessage::RequestParts(req))
                        } else {
                            Ok(Ed2kMessage::Raw(packet.clone()))
                        }
                    }
                    Ed2kCompatibilityProfile::LegacyEdonkey => {
                        if packet.opcode == OP_REQUESTPARTS {
                            let req = self.decode_request_parts_with_profile(
                                self.profile,
                                packet.protocol,
                                packet.opcode,
                                &packet.payload,
                                0,
                            )?;
                            Ok(Ed2kMessage::RequestParts(req))
                        } else {
                            Ok(Ed2kMessage::Raw(packet.clone()))
                        }
                    }
                }
            }
            (PR_ED2K, OP_SENDINGPART) => {
                let part = self.decode_sending_part(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::SendingPart(part))
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

        // In standard eD2K client-to-client handshake, OP_HELLO payload begins with hash length (0x10 = 16).
        // If present, consume it. If absent (e.g. legacy/synthetic fixtures), read hash directly.
        if reader.remaining() >= 17 && reader.peek_u8() == Some(0x10) {
            let _ = reader.read_u8("hello_user_hash_len")?;
        }

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

    /// Decodes a file status request payload using the decoder's configured profile.
    pub fn decode_file_status_req(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kFileStatusReq, Ed2kParseError> {
        self.decode_file_status_req_with_profile(
            self.profile,
            protocol,
            opcode,
            payload,
            base_offset,
        )
    }

    /// Decodes a file status request payload with an explicit compatibility profile.
    pub fn decode_file_status_req_with_profile(
        &self,
        profile: Ed2kCompatibilityProfile,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kFileStatusReq, Ed2kParseError> {
        match profile {
            Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
                if opcode == OP_FILESTATUSREQ {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        base_offset,
                        "file_status_req_opcode",
                        Ed2kErrorKind::AmbiguousOpcodeForProfile {
                            opcode,
                            profile,
                            reason:
                                "opcode 0x49 is OP_END_OF_DOWNLOAD in eMule/aMule, not OP_FILESTATUSREQ",
                        },
                    ));
                }
                if opcode != OP_SETREQFILEID_EMULE {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        base_offset,
                        "file_status_req_opcode",
                        Ed2kErrorKind::AmbiguousOpcodeForProfile {
                            opcode,
                            profile,
                            reason: "expected OP_SETREQFILEID_EMULE (0x4F)",
                        },
                    ));
                }
            }
            Ed2kCompatibilityProfile::LegacyEdonkey => {
                if opcode == OP_SETREQFILEID_EMULE {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        base_offset,
                        "file_status_req_opcode",
                        Ed2kErrorKind::AmbiguousOpcodeForProfile {
                            opcode,
                            profile,
                            reason: "opcode 0x4F is not recognized in legacy eDonkey profile",
                        },
                    ));
                }
                if opcode != OP_FILESTATUSREQ {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        base_offset,
                        "file_status_req_opcode",
                        Ed2kErrorKind::AmbiguousOpcodeForProfile {
                            opcode,
                            profile,
                            reason: "expected OP_FILESTATUSREQ (0x49)",
                        },
                    ));
                }
            }
            Ed2kCompatibilityProfile::UnknownStrict => {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset,
                    "file_status_req_profile",
                    Ed2kErrorKind::AmbiguousOpcodeForProfile {
                        opcode,
                        profile,
                        reason:
                            "ambiguous file status request opcode cannot be determined under UnknownStrict profile",
                    },
                ));
            }
        }

        let mut reader = TagReader::new(payload, 0, protocol, Some(opcode));
        let file_hash = reader.read_hash("filestatusreq_file_hash")?;
        if file_hash == [0u8; 16] {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset,
                "filestatusreq_file_hash",
                Ed2kErrorKind::MalformedField {
                    message: "zero file hash is not permitted in file status request".to_string(),
                },
            ));
        }
        if reader.remaining() > 0 {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + reader.pos(),
                "filestatusreq_trailing_bytes",
                Ed2kErrorKind::MalformedField {
                    message: format!(
                        "unexpected {} trailing bytes after file hash in file status request",
                        reader.remaining()
                    ),
                },
            ));
        }
        Ok(Ed2kFileStatusReq { file_hash })
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
            expected_mask_bytes(part_count)
        };

        let bitfield_bytes = if bitfield_bytes_needed > 0 {
            reader.read_bytes(bitfield_bytes_needed, "filestatus_bitfield")?
        } else {
            &[]
        };

        if reader.remaining() > 0 {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + reader.pos(),
                "filestatus_bitfield_trailing_bytes",
                Ed2kErrorKind::MalformedField {
                    message: format!(
                        "unexpected {} trailing bytes in file_status bitfield",
                        reader.remaining()
                    ),
                },
            ));
        }

        let availability_mask = PartAvailabilityMask::from_bytes(
            Bytes::copy_from_slice(bitfield_bytes),
            part_count,
        )
        .map_err(|k| {
            Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + 18,
                "filestatus_bitfield",
                k,
            )
        })?;

        Ok(Ed2kFileStatus {
            file_hash,
            part_count,
            availability_mask,
        })
    }

    /// Decodes an `OP_REQUESTPARTS` payload using the decoder's configured profile.
    pub fn decode_request_parts(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kRequestParts, Ed2kParseError> {
        self.decode_request_parts_with_profile(
            self.profile,
            protocol,
            opcode,
            payload,
            base_offset,
        )
    }

    /// Decodes a request parts payload according to an explicit `Ed2kCompatibilityProfile`.
    pub fn decode_request_parts_with_profile(
        &self,
        profile: Ed2kCompatibilityProfile,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kRequestParts, Ed2kParseError> {
        match profile {
            Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
                if opcode == OP_REQUESTPARTS {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        base_offset,
                        "request_parts_opcode",
                        Ed2kErrorKind::AmbiguousOpcodeForProfile {
                            opcode,
                            profile,
                            reason:
                                "opcode 0x4B is OP_ASKSHAREDFILESANSWER in eMule/aMule, not OP_REQUESTPARTS",
                        },
                    ));
                }
                if opcode != OP_REQUESTPARTS_EMULE {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        base_offset,
                        "request_parts_opcode",
                        Ed2kErrorKind::AmbiguousOpcodeForProfile {
                            opcode,
                            profile,
                            reason: "expected OP_REQUESTPARTS_EMULE (0x47)",
                        },
                    ));
                }
                if payload.len() == 24 {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        base_offset + 16,
                        "request_parts_layout",
                        Ed2kErrorKind::MalformedField {
                            message:
                                "24-byte single-slot layout is not permitted under eMule/aMule profile (expected 40-byte 3-slot layout)"
                                    .to_string(),
                        },
                    ));
                }
            }
            Ed2kCompatibilityProfile::LegacyEdonkey => {
                if opcode == OP_REQUESTPARTS_EMULE {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        base_offset,
                        "request_parts_opcode",
                        Ed2kErrorKind::AmbiguousOpcodeForProfile {
                            opcode,
                            profile,
                            reason:
                                "opcode 0x47 was OP_SETREQFILEID in legacy eDonkey, not OP_REQUESTPARTS",
                        },
                    ));
                }
                if opcode != OP_REQUESTPARTS {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        base_offset,
                        "request_parts_opcode",
                        Ed2kErrorKind::AmbiguousOpcodeForProfile {
                            opcode,
                            profile,
                            reason: "expected OP_REQUESTPARTS (0x4B)",
                        },
                    ));
                }
            }
            Ed2kCompatibilityProfile::UnknownStrict => {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset,
                    "request_parts_profile",
                    Ed2kErrorKind::AmbiguousOpcodeForProfile {
                        opcode,
                        profile,
                        reason:
                            "ambiguous request parts opcode cannot be determined under UnknownStrict profile",
                    },
                ));
            }
        }

        if payload.len() < 24 {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + payload.len(),
                "request_parts_header",
                Ed2kErrorKind::UnexpectedEof {
                    needed: 24,
                    available: payload.len(),
                },
            ));
        }

        let mut file_hash = [0u8; 16];
        file_hash.copy_from_slice(&payload[0..16]);

        if file_hash == [0u8; 16] {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset,
                "request_parts_file_hash",
                Ed2kErrorKind::MalformedField {
                    message: "zero file hash is not permitted in request parts".to_string(),
                },
            ));
        }

        if payload.len() == 24 {
            let start = u32::from_le_bytes([payload[16], payload[17], payload[18], payload[19]]);
            let end = u32::from_le_bytes([payload[20], payload[21], payload[22], payload[23]]);

            if start >= end {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset + 16,
                    "request_parts_range",
                    Ed2kErrorKind::InvalidBlockOffsets {
                        start: start as u64,
                        end: end as u64,
                    },
                ));
            }

            return Ok(Ed2kRequestParts::single(file_hash, start, end));
        }

        if payload.len() != 40 {
            if payload.len() < 40 {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset + payload.len(),
                    "request_parts_payload",
                    Ed2kErrorKind::UnexpectedEof {
                        needed: 40,
                        available: payload.len(),
                    },
                ));
            } else {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset + 40,
                    "request_parts_payload",
                    Ed2kErrorKind::MalformedField {
                        message: format!(
                            "unexpected payload length {} for request parts (expected 24 or 40)",
                            payload.len()
                        ),
                    },
                ));
            }
        }

        let mut ranges = Vec::with_capacity(3);
        for i in 0..3 {
            let start = u32::from_le_bytes([
                payload[16 + i * 4],
                payload[17 + i * 4],
                payload[18 + i * 4],
                payload[19 + i * 4],
            ]);
            let end = u32::from_le_bytes([
                payload[28 + i * 4],
                payload[29 + i * 4],
                payload[30 + i * 4],
                payload[31 + i * 4],
            ]);

            if (start == 0 && end == 0) || (start == u32::MAX && end == u32::MAX) {
                continue;
            }

            if start >= end {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    base_offset + 16 + i * 4,
                    "request_parts_range",
                    Ed2kErrorKind::InvalidBlockOffsets {
                        start: start as u64,
                        end: end as u64,
                    },
                ));
            }

            ranges.push(Ed2kBlockRange::new(start, end));
        }

        if ranges.is_empty() {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + 16,
                "request_parts_ranges",
                Ed2kErrorKind::MalformedField {
                    message: "no valid non-zero block ranges in request parts".to_string(),
                },
            ));
        }

        Ok(Ed2kRequestParts::multi(file_hash, ranges))
    }

    /// Decodes an `OP_SENDINGPART` (`0x46`) payload.
    pub fn decode_sending_part(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kSendingPart, Ed2kParseError> {
        if payload.len() < 24 {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + payload.len(),
                "sending_part_header",
                Ed2kErrorKind::UnexpectedEof {
                    needed: 24,
                    available: payload.len(),
                },
            ));
        }

        let mut file_hash = [0u8; 16];
        file_hash.copy_from_slice(&payload[0..16]);

        let start_offset = u32::from_le_bytes([payload[16], payload[17], payload[18], payload[19]]);
        let end_offset = u32::from_le_bytes([payload[20], payload[21], payload[22], payload[23]]);

        if start_offset >= end_offset {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + 16,
                "sending_part_offsets",
                Ed2kErrorKind::InvalidBlockOffsets {
                    start: start_offset as u64,
                    end: end_offset as u64,
                },
            ));
        }

        let expected_len = (end_offset - start_offset) as usize;
        let actual_len = payload.len() - 24;

        if actual_len != expected_len {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                base_offset + 24,
                "sending_part_payload",
                Ed2kErrorKind::BlockPayloadMismatch {
                    expected: expected_len,
                    actual: actual_len,
                },
            ));
        }

        let data = Bytes::copy_from_slice(&payload[24..]);

        Ok(Ed2kSendingPart::new(
            file_hash,
            start_offset,
            end_offset,
            data,
        ))
    }
}
