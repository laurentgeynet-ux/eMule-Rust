// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Safe, read-only eD2K wire decoders with strict bounds and context tracking.

use bytes::Bytes;

use super::error::{Ed2kErrorKind, Ed2kParseError};
use super::limits::Ed2kLimits;
use super::opcodes::{
    OP_AICHFILEHASHREQ, OP_AICHFILEHASHRES, OP_AICH_ANSWER, OP_AICH_REQUEST, OP_COMPRESSEDPART_I64,
    OP_EMULE_INFO, OP_EMULE_INFO_ANSWER, OP_FILEREQANSNOFILE, OP_FILESTATUS, OP_FILESTATUSREQ,
    OP_HASHSETANSWER, OP_HASHSETREQUEST, OP_HELLO, OP_HELLO_ANSWER, OP_PUBLICKEY, OP_REQUESTPARTS,
    OP_REQUESTPARTS_EMULE, OP_REQUESTPARTS_I64, OP_SECIDENTSTATE, OP_SENDINGPART,
    OP_SENDINGPART_I64, OP_SETREQFILEID_EMULE, OP_SIGNATURE,
    OP_CHANGE_SLOT, OP_CHATCAPTCHAREQ, OP_CHATCAPTCHARES, OP_FILEDESC, OP_HASHSETANSWER2,
    OP_HASHSETREQUEST2, OP_MESSAGE, OP_MULTIPACKET, OP_MULTIPACKETANSWER,
    OP_MULTIPACKETANSWER_EXT2, OP_MULTIPACKET_EXT, OP_MULTIPACKET_EXT2, OP_PORTTEST,
    OP_QUEUERANK, OP_QUEUERANKING,
};
use super::packet::{
    Ed2kAichAnswer, Ed2kAichFileHashReq, Ed2kAichFileHashRes, Ed2kAichRequest, Ed2kEmuleInfo,
    Ed2kFileReqAnsNoFile, Ed2kFileStatus, Ed2kFileStatusReq, Ed2kHashSetAnswer, Ed2kHashSetReq,
    Ed2kHello, Ed2kHelloAnswer, Ed2kMessage, Ed2kPublicKey, Ed2kRequestParts, Ed2kSecIdentState,
    Ed2kSendingPart, Ed2kSignature,
};
use super::profile::Ed2kCompatibilityProfile;
use super::protocol::{is_valid_ed2k_protocol, PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED};
use crate::framing::RawPacket;
use crate::limits::HEADER_SIZE;

pub mod aich;
pub mod extended;
pub mod handshake;
pub mod security;
pub mod status;
pub mod transfers;

pub use aich::{
    decode_aich_answer, decode_aich_file_hash_req, decode_aich_file_hash_res, decode_aich_request,
};
pub use extended::{
    decode_captcha_request, decode_captcha_response, decode_change_slot, decode_file_desc,
    decode_hashset_answer2, decode_hashset_request2, decode_message, decode_multipacket_answer,
    decode_multipacket_request, decode_port_test, decode_queue_rank, decode_queue_ranking,
};
pub use handshake::{decode_emule_info, decode_hello, decode_hello_answer};
pub use security::{decode_public_key, decode_secident_state, decode_signature};
pub use status::{
    decode_file_req_ans_no_file, decode_file_status, decode_file_status_req_with_profile,
};
pub use transfers::{
    decode_compressed_part_i64, decode_hashset_answer, decode_hashset_request,
    decode_request_parts_i64, decode_request_parts_with_profile, decode_sending_part,
    decode_sending_part_i64,
};

/// Read-only wire decoder for eD2K protocol frames.
#[derive(Debug, Clone)]
pub struct Ed2kDecoder {
    /// Safety limits and resource bounds.
    pub limits: Ed2kLimits,
    /// Active compatibility profile for wire opcode and layout disambiguation.
    pub profile: Ed2kCompatibilityProfile,
    /// Negotiated `ExtendedRequestsVersion` of the remote peer
    /// (eMule `m_byExtendedRequestsVer`, exchanged via `OP_EMULEINFO`).
    ///
    /// Drives deterministic parsing of the `OP_REQFILENAME` (`0x58`)
    /// sub-request ext info inside `MULTIPACKET*` bodies:
    /// `0` = absent, `1` = part-status bitfield, `>=2` = bitfield + complete
    /// sources count. Defaults to `2` (eMule 0.72a capability); set to `0` to
    /// decode captures from peers that did not negotiate extended requests.
    pub extended_requests_version: u8,
}

impl Default for Ed2kDecoder {
    fn default() -> Self {
        Self {
            limits: Ed2kLimits::default(),
            profile: Ed2kCompatibilityProfile::default(),
            extended_requests_version: 2,
        }
    }
}

impl Ed2kDecoder {
    /// Creates a new decoder with default limits and default profile (`Emule072a`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new decoder with an explicit compatibility profile and default limits.
    pub fn with_profile(profile: Ed2kCompatibilityProfile) -> Self {
        Self {
            limits: Ed2kLimits::default(),
            profile,
            extended_requests_version: 2,
        }
    }

    /// Creates a new decoder with customized limits and default profile.
    pub fn with_limits(limits: Ed2kLimits) -> Self {
        Self {
            limits,
            profile: Ed2kCompatibilityProfile::default(),
            extended_requests_version: 2,
        }
    }

    /// Creates a new decoder with customized limits and an explicit compatibility profile.
    pub fn with_limits_and_profile(limits: Ed2kLimits, profile: Ed2kCompatibilityProfile) -> Self {
        Self {
            limits,
            profile,
            extended_requests_version: 2,
        }
    }

    /// Sets the negotiated `ExtendedRequestsVersion` used for `MULTIPACKET*`
    /// ext-info parsing (`0`, `1` or `2`).
    pub fn with_extended_requests_version(mut self, version: u8) -> Self {
        self.extended_requests_version = version;
        self
    }

    /// Decodes a framed byte slice into a typed `Ed2kMessage`.
    pub fn decode(&self, src: &[u8]) -> Result<Ed2kMessage, Ed2kParseError> {
        self.decode_packet(src)
    }

    /// Decodes a framed byte slice into a typed `Ed2kMessage`.
    pub fn decode_packet(&self, src: &[u8]) -> Result<Ed2kMessage, Ed2kParseError> {
        if src.len() < HEADER_SIZE + 1 {
            return Err(Ed2kParseError::new(
                src.first().copied().unwrap_or(PR_ED2K),
                src.get(5).copied(),
                0,
                "header",
                Ed2kErrorKind::UnexpectedEof {
                    needed: HEADER_SIZE + 1,
                    available: src.len(),
                },
            ));
        }

        let protocol = src[0];
        if protocol == PR_EMULE_COMPRESSED {
            return Err(Ed2kParseError::new(
                protocol,
                Some(src[5]),
                0,
                "protocol_byte",
                Ed2kErrorKind::CompressedProtocolUnsupported,
            ));
        }
        if !is_valid_ed2k_protocol(protocol) {
            return Err(Ed2kParseError::new(
                protocol,
                Some(src[5]),
                0,
                "protocol_byte",
                Ed2kErrorKind::UnsupportedProtocol { byte: protocol },
            ));
        }

        let frame_len = u32::from_le_bytes([src[1], src[2], src[3], src[4]]) as usize;
        let opcode = src[5];

        if frame_len == 0 {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                1,
                "packet_length",
                Ed2kErrorKind::FrameTooSmall { size: 0 },
            ));
        }

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
                    Ed2kCompatibilityProfile::Emule072a | Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
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
            (PR_ED2K, OP_FILEREQANSNOFILE) => {
                let no_file = self.decode_file_req_ans_no_file(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::FileReqAnsNoFile(no_file))
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
                    Ed2kCompatibilityProfile::Emule072a | Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
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
            (PR_ED2K, OP_REQUESTPARTS_I64) => {
                let req = decode_request_parts_i64(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::RequestPartsI64(req))
            }
            (PR_ED2K, OP_SENDINGPART) => {
                let part = self.decode_sending_part(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::SendingPart(part))
            }
            (PR_ED2K, OP_SENDINGPART_I64) => {
                let part =
                    decode_sending_part_i64(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::SendingPartI64(part))
            }
            (_, OP_COMPRESSEDPART_I64) => {
                let part =
                    decode_compressed_part_i64(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::CompressedPartI64(part))
            }
            (PR_ED2K, OP_HASHSETREQUEST) => {
                let req = self.decode_hashset_request(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::HashSetReq(req))
            }
            (PR_ED2K, OP_HASHSETANSWER) => {
                let ans = self.decode_hashset_answer(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::HashSetAnswer(ans))
            }
            (PR_EMULE, OP_EMULE_INFO) | (PR_EMULE, OP_EMULE_INFO_ANSWER) => {
                let info = self.decode_emule_info(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::EmuleInfo(info))
            }
            (PR_EMULE, OP_AICHFILEHASHREQ) => {
                let req = self.decode_aich_file_hash_req(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::AichFileHashReq(req))
            }
            (PR_EMULE, OP_AICHFILEHASHRES) => {
                let res = self.decode_aich_file_hash_res(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::AichFileHashRes(res))
            }
            (PR_EMULE, OP_AICH_REQUEST) => {
                let req = self.decode_aich_request(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::AichRequest(req))
            }
            (PR_EMULE, OP_AICH_ANSWER) => {
                let ans = self.decode_aich_answer(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::AichAnswer(ans))
            }
            (PR_EMULE, OP_PUBLICKEY) => {
                let key = self.decode_public_key(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::PublicKey(key))
            }
            (PR_EMULE, OP_SIGNATURE) => {
                let sig = self.decode_signature(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::Signature(sig))
            }
            (PR_EMULE, OP_SECIDENTSTATE) => {
                let state = self.decode_secident_state(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::SecIdentState(state))
            }
            (PR_ED2K, OP_MESSAGE) => {
                let msg = decode_message(protocol, opcode, payload, HEADER_SIZE + 1, &self.limits)?;
                Ok(Ed2kMessage::Message(msg))
            }
            (PR_ED2K, OP_QUEUERANK) => {
                let qr = decode_queue_rank(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::QueueRank(qr))
            }
            (PR_ED2K, OP_CHANGE_SLOT) => {
                let cs = decode_change_slot(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::ChangeSlot(cs))
            }
            (PR_ED2K, OP_PORTTEST) => {
                let pt = decode_port_test(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::PortTest(pt))
            }
            (PR_EMULE, OP_QUEUERANKING) => {
                let qr = decode_queue_ranking(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::QueueRanking(qr))
            }
            (PR_EMULE, OP_FILEDESC) => {
                let fd = decode_file_desc(protocol, opcode, payload, HEADER_SIZE + 1, &self.limits)?;
                Ok(Ed2kMessage::FileDesc(fd))
            }
            (PR_EMULE, OP_MULTIPACKET)
            | (PR_EMULE, OP_MULTIPACKET_EXT)
            | (PR_EMULE, OP_MULTIPACKET_EXT2) => {
                let req = decode_multipacket_request(
                    protocol,
                    opcode,
                    payload,
                    HEADER_SIZE + 1,
                    &self.limits,
                    self.extended_requests_version,
                )?;
                Ok(Ed2kMessage::MultiPacketRequest(req))
            }
            (PR_EMULE, OP_MULTIPACKETANSWER) | (PR_EMULE, OP_MULTIPACKETANSWER_EXT2) => {
                let ans = decode_multipacket_answer(
                    protocol,
                    opcode,
                    payload,
                    HEADER_SIZE + 1,
                    &self.limits,
                )?;
                Ok(Ed2kMessage::MultiPacketAnswer(ans))
            }
            (PR_EMULE, OP_CHATCAPTCHAREQ) => {
                let req =
                    decode_captcha_request(protocol, opcode, payload, HEADER_SIZE + 1, &self.limits)?;
                Ok(Ed2kMessage::CaptchaRequest(req))
            }
            (PR_EMULE, OP_CHATCAPTCHARES) => {
                let res = decode_captcha_response(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::CaptchaResponse(res))
            }
            (PR_EMULE, OP_HASHSETREQUEST2) => {
                let req =
                    decode_hashset_request2(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::HashSetReq2(req))
            }
            (PR_EMULE, OP_HASHSETANSWER2) => {
                let ans =
                    decode_hashset_answer2(protocol, opcode, payload, HEADER_SIZE + 1)?;
                Ok(Ed2kMessage::HashSetAnswer2(ans))
            }
            _ => {
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
                    Ed2kCompatibilityProfile::Emule072a | Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
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
            (PR_ED2K, OP_FILEREQANSNOFILE) => {
                let no_file = self.decode_file_req_ans_no_file(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::FileReqAnsNoFile(no_file))
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
                    Ed2kCompatibilityProfile::Emule072a | Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
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
            (PR_ED2K, OP_REQUESTPARTS_I64) => {
                let req = decode_request_parts_i64(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::RequestPartsI64(req))
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
            (PR_ED2K, OP_SENDINGPART_I64) => {
                let part = decode_sending_part_i64(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::SendingPartI64(part))
            }
            (_, OP_COMPRESSEDPART_I64) => {
                let part = decode_compressed_part_i64(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::CompressedPartI64(part))
            }
            (PR_ED2K, OP_HASHSETREQUEST) => {
                let req = self.decode_hashset_request(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::HashSetReq(req))
            }
            (PR_ED2K, OP_HASHSETANSWER) => {
                let ans = self.decode_hashset_answer(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::HashSetAnswer(ans))
            }
            (PR_EMULE, OP_EMULE_INFO) | (PR_EMULE, OP_EMULE_INFO_ANSWER) => {
                let info = self.decode_emule_info(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::EmuleInfo(info))
            }
            (PR_EMULE, OP_AICHFILEHASHREQ) => {
                let req = self.decode_aich_file_hash_req(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::AichFileHashReq(req))
            }
            (PR_EMULE, OP_AICHFILEHASHRES) => {
                let res = self.decode_aich_file_hash_res(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::AichFileHashRes(res))
            }
            (PR_EMULE, OP_AICH_REQUEST) => {
                let req = self.decode_aich_request(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::AichRequest(req))
            }
            (PR_EMULE, OP_AICH_ANSWER) => {
                let ans = self.decode_aich_answer(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::AichAnswer(ans))
            }
            (PR_EMULE, OP_PUBLICKEY) => {
                let key = self.decode_public_key(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::PublicKey(key))
            }
            (PR_EMULE, OP_SIGNATURE) => {
                let sig = self.decode_signature(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::Signature(sig))
            }
            (PR_EMULE, OP_SECIDENTSTATE) => {
                let state = self.decode_secident_state(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                )?;
                Ok(Ed2kMessage::SecIdentState(state))
            }
            (PR_ED2K, OP_MESSAGE) => {
                let msg =
                    decode_message(packet.protocol, packet.opcode, &packet.payload, 0, &self.limits)?;
                Ok(Ed2kMessage::Message(msg))
            }
            (PR_ED2K, OP_QUEUERANK) => {
                let qr = decode_queue_rank(packet.protocol, packet.opcode, &packet.payload, 0)?;
                Ok(Ed2kMessage::QueueRank(qr))
            }
            (PR_ED2K, OP_CHANGE_SLOT) => {
                let cs = decode_change_slot(packet.protocol, packet.opcode, &packet.payload, 0)?;
                Ok(Ed2kMessage::ChangeSlot(cs))
            }
            (PR_ED2K, OP_PORTTEST) => {
                let pt = decode_port_test(packet.protocol, packet.opcode, &packet.payload, 0)?;
                Ok(Ed2kMessage::PortTest(pt))
            }
            (PR_EMULE, OP_QUEUERANKING) => {
                let qr = decode_queue_ranking(packet.protocol, packet.opcode, &packet.payload, 0)?;
                Ok(Ed2kMessage::QueueRanking(qr))
            }
            (PR_EMULE, OP_FILEDESC) => {
                let fd = decode_file_desc(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                    &self.limits,
                )?;
                Ok(Ed2kMessage::FileDesc(fd))
            }
            (PR_EMULE, OP_MULTIPACKET)
            | (PR_EMULE, OP_MULTIPACKET_EXT)
            | (PR_EMULE, OP_MULTIPACKET_EXT2) => {
                let req = decode_multipacket_request(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                    &self.limits,
                    self.extended_requests_version,
                )?;
                Ok(Ed2kMessage::MultiPacketRequest(req))
            }
            (PR_EMULE, OP_MULTIPACKETANSWER) | (PR_EMULE, OP_MULTIPACKETANSWER_EXT2) => {
                let ans = decode_multipacket_answer(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                    &self.limits,
                )?;
                Ok(Ed2kMessage::MultiPacketAnswer(ans))
            }
            (PR_EMULE, OP_CHATCAPTCHAREQ) => {
                let req = decode_captcha_request(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                    &self.limits,
                )?;
                Ok(Ed2kMessage::CaptchaRequest(req))
            }
            (PR_EMULE, OP_CHATCAPTCHARES) => {
                let res =
                    decode_captcha_response(packet.protocol, packet.opcode, &packet.payload, 0)?;
                Ok(Ed2kMessage::CaptchaResponse(res))
            }
            (PR_EMULE, OP_HASHSETREQUEST2) => {
                let req =
                    decode_hashset_request2(packet.protocol, packet.opcode, &packet.payload, 0)?;
                Ok(Ed2kMessage::HashSetReq2(req))
            }
            (PR_EMULE, OP_HASHSETANSWER2) => {
                let ans =
                    decode_hashset_answer2(packet.protocol, packet.opcode, &packet.payload, 0)?;
                Ok(Ed2kMessage::HashSetAnswer2(ans))
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
        decode_hello(&self.limits, protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_HELLO_ANSWER` payload.
    pub fn decode_hello_answer(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kHelloAnswer, Ed2kParseError> {
        decode_hello_answer(&self.limits, protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_EMULE_INFO` (`0xC5 0x01`) payload.
    pub fn decode_emule_info(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kEmuleInfo, Ed2kParseError> {
        decode_emule_info(&self.limits, protocol, opcode, payload, base_offset)
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
        decode_file_status_req_with_profile(profile, protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_FILESTATUS` payload.
    pub fn decode_file_status(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kFileStatus, Ed2kParseError> {
        decode_file_status(&self.limits, protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_FILEREQANSNOFILE` (`0x48`) payload (16 bytes file hash).
    pub fn decode_file_req_ans_no_file(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kFileReqAnsNoFile, Ed2kParseError> {
        decode_file_req_ans_no_file(protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_REQUESTPARTS` payload using the decoder's configured profile.
    pub fn decode_request_parts(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kRequestParts, Ed2kParseError> {
        self.decode_request_parts_with_profile(self.profile, protocol, opcode, payload, base_offset)
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
        decode_request_parts_with_profile(profile, protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_SENDINGPART` (`0x46`) payload.
    pub fn decode_sending_part(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kSendingPart, Ed2kParseError> {
        decode_sending_part(protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_HASHSETREQUEST` (`0x51`) payload.
    pub fn decode_hashset_request(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kHashSetReq, Ed2kParseError> {
        decode_hashset_request(protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_HASHSETANSWER` (`0x52`) payload.
    pub fn decode_hashset_answer(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kHashSetAnswer, Ed2kParseError> {
        decode_hashset_answer(protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_AICHFILEHASHREQ` (eMule `0x9E`) payload.
    pub fn decode_aich_file_hash_req(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kAichFileHashReq, Ed2kParseError> {
        decode_aich_file_hash_req(protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_AICHFILEHASHANS` (eMule `0x9D`) payload.
    pub fn decode_aich_file_hash_res(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kAichFileHashRes, Ed2kParseError> {
        decode_aich_file_hash_res(protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_AICHREQUEST` (eMule `0x9B`) payload.
    pub fn decode_aich_request(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kAichRequest, Ed2kParseError> {
        decode_aich_request(protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_AICHANSWER` (eMule `0x9C`) payload.
    pub fn decode_aich_answer(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kAichAnswer, Ed2kParseError> {
        decode_aich_answer(protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_PUBLICKEY` (`0x85`) payload.
    pub fn decode_public_key(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kPublicKey, Ed2kParseError> {
        decode_public_key(protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_SIGNATURE` (`0x86`) payload.
    pub fn decode_signature(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kSignature, Ed2kParseError> {
        decode_signature(protocol, opcode, payload, base_offset)
    }

    /// Decodes an `OP_SECIDENTSTATE` (`0x87`) payload.
    pub fn decode_secident_state(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
        base_offset: usize,
    ) -> Result<Ed2kSecIdentState, Ed2kParseError> {
        decode_secident_state(protocol, opcode, payload, base_offset)
    }

    /// Decodes an incoming Source Exchange request (`OP_REQUESTSOURCES` or `OP_REQUESTSOURCES2`).
    pub fn decode_request_sources(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
    ) -> Result<super::source_exchange::Ed2kRequestSources, Ed2kParseError> {
        super::source_exchange::decode_request_sources(protocol, opcode, payload)
    }

    /// Decodes an incoming Source Exchange response (`OP_ANSWERSOURCES` or `OP_ANSWERSOURCES2`).
    pub fn decode_answer_sources(
        &self,
        protocol: u8,
        opcode: u8,
        payload: &[u8],
    ) -> Result<super::source_exchange::Ed2kAnswerSources, Ed2kParseError> {
        super::source_exchange::decode_answer_sources(protocol, opcode, payload)
    }
}
