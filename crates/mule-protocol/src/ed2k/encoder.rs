//! Safe, deterministic eD2K wire encoders with strict resource bounds.

use bytes::Bytes;

use super::error::{Ed2kEncodeError, Ed2kErrorKind};
use super::limits::Ed2kLimits;
use super::packet::{
    Ed2kAichAnswer, Ed2kAichFileHashReq, Ed2kAichFileHashRes, Ed2kAichRequest, Ed2kEmuleInfo,
    Ed2kFileReqAnsNoFile, Ed2kFileStatus, Ed2kFileStatusReq, Ed2kHashSetAnswer, Ed2kHashSetReq,
    Ed2kHello, Ed2kHelloAnswer, Ed2kMessage, Ed2kPublicKey, Ed2kRequestParts, Ed2kSecIdentState,
    Ed2kSendingPart, Ed2kSignature,
};
use super::profile::Ed2kCompatibilityProfile;
use super::tags::{Ed2kTag, RawTag, TagName};
use crate::framing::RawPacket;
use crate::limits::HEADER_SIZE;

/// AICH message encoders (AichFileHashReq/Res, AichRequest/Answer).
pub mod aich;
/// Handshake message encoders (Hello, HelloAnswer, EmuleInfo).
pub mod handshake;
/// Security message encoders (PublicKey, Signature, SecIdentState).
pub mod security;
/// eD2K Tag and tag name encoders.
pub mod tags;
/// Transfer message encoders (FileStatus, RequestParts, SendingPart, FileReqAnsNoFile).
pub mod transfers;

pub use aich::{
    encode_aich_answer, encode_aich_file_hash_req, encode_aich_file_hash_res, encode_aich_request,
};
pub use handshake::{encode_emule_info, encode_hello, encode_hello_answer};
pub use security::{encode_public_key, encode_secident_state, encode_signature};
pub use tags::{encode_raw_tag, encode_tag, encode_tag_name};
pub use transfers::{
    encode_file_req_ans_no_file, encode_file_status, encode_file_status_req,
    encode_file_status_req_with_opcode, encode_hashset_answer, encode_hashset_request,
    encode_request_parts, encode_request_parts_with_opcode, encode_sending_part,
};

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
        encode_tag_name(&self.limits, opcode, name, dst)
    }

    /// Encodes a single recognized `Ed2kTag` into the destination buffer.
    pub fn encode_tag(
        &self,
        opcode: u8,
        tag: &Ed2kTag,
        dst: &mut Vec<u8>,
    ) -> Result<(), Ed2kEncodeError> {
        encode_tag(&self.limits, opcode, tag, dst)
    }

    /// Encodes a raw preserved unknown tag into the destination buffer.
    pub fn encode_raw_tag(
        &self,
        opcode: u8,
        tag: &RawTag,
        dst: &mut Vec<u8>,
    ) -> Result<(), Ed2kEncodeError> {
        encode_raw_tag(&self.limits, opcode, tag, dst)
    }

    /// Encodes an `Ed2kHello` message into a `RawPacket`.
    pub fn encode_hello(&self, hello: &Ed2kHello) -> Result<RawPacket, Ed2kEncodeError> {
        encode_hello(&self.limits, self.profile, hello)
    }

    /// Encodes an `Ed2kHelloAnswer` message into a `RawPacket`.
    pub fn encode_hello_answer(
        &self,
        answer: &Ed2kHelloAnswer,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_hello_answer(&self.limits, answer)
    }

    /// Encodes an `Ed2kFileStatusReq` message using the encoder's active profile opcode.
    pub fn encode_file_status_req(
        &self,
        req: &Ed2kFileStatusReq,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_file_status_req(&self.limits, self.profile, req)
    }

    /// Encodes an `Ed2kFileStatusReq` message into a `RawPacket` with a specified opcode.
    pub fn encode_file_status_req_with_opcode(
        &self,
        req: &Ed2kFileStatusReq,
        opcode: u8,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_file_status_req_with_opcode(&self.limits, req, opcode)
    }

    /// Encodes an `Ed2kFileStatus` message into a `RawPacket`.
    pub fn encode_file_status(
        &self,
        status: &Ed2kFileStatus,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_file_status(&self.limits, status)
    }

    /// Encodes an `Ed2kFileReqAnsNoFile` (`0x48`) message into a `RawPacket`.
    pub fn encode_file_req_ans_no_file(
        &self,
        ans: &Ed2kFileReqAnsNoFile,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_file_req_ans_no_file(&self.limits, ans)
    }

    /// Encodes an `Ed2kRequestParts` message using the encoder's active profile opcode.
    pub fn encode_request_parts(
        &self,
        req: &Ed2kRequestParts,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_request_parts(&self.limits, self.profile, req)
    }

    /// Encodes an `Ed2kRequestParts` message with a specified opcode.
    pub fn encode_request_parts_with_opcode(
        &self,
        req: &Ed2kRequestParts,
        opcode: u8,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_request_parts_with_opcode(&self.limits, req, opcode)
    }

    /// Encodes an `Ed2kSendingPart` message (`0x46`).
    pub fn encode_sending_part(
        &self,
        part: &Ed2kSendingPart,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_sending_part(&self.limits, part)
    }

    /// Encodes an `Ed2kHashSetReq` message (`0x51`).
    pub fn encode_hashset_request(
        &self,
        req: &Ed2kHashSetReq,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_hashset_request(&self.limits, req)
    }

    /// Encodes an `Ed2kHashSetAnswer` message (`0x52`).
    pub fn encode_hashset_answer(
        &self,
        ans: &Ed2kHashSetAnswer,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_hashset_answer(&self.limits, ans)
    }

    /// Encodes an `OP_AICHFILEHASHREQ` (eMule `0x14`) message.
    pub fn encode_aich_file_hash_req(
        &self,
        req: &Ed2kAichFileHashReq,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_aich_file_hash_req(&self.limits, req)
    }

    /// Encodes an `OP_AICHFILEHASHANS` (eMule `0x15`) message.
    pub fn encode_aich_file_hash_res(
        &self,
        res: &Ed2kAichFileHashRes,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_aich_file_hash_res(&self.limits, res)
    }

    /// Encodes an `OP_AICHREQUEST` (eMule `0x12`) message.
    pub fn encode_aich_request(&self, req: &Ed2kAichRequest) -> Result<RawPacket, Ed2kEncodeError> {
        encode_aich_request(&self.limits, req)
    }

    /// Encodes an `OP_AICHANSWER` (eMule `0x13`) message.
    pub fn encode_aich_answer(&self, ans: &Ed2kAichAnswer) -> Result<RawPacket, Ed2kEncodeError> {
        encode_aich_answer(&self.limits, ans)
    }

    /// Encodes an `OP_EMULE_INFO` (`0xC5 0x01`) payload.
    pub fn encode_emule_info(&self, info: &Ed2kEmuleInfo) -> Result<RawPacket, Ed2kEncodeError> {
        encode_emule_info(&self.limits, info)
    }

    /// Encodes an `OP_PUBLICKEY` (`0x85`) message into a `RawPacket`.
    pub fn encode_public_key(&self, key: &Ed2kPublicKey) -> Result<RawPacket, Ed2kEncodeError> {
        encode_public_key(&self.limits, key)
    }

    /// Encodes an `OP_SIGNATURE` (`0x86`) message into a `RawPacket`.
    pub fn encode_signature(&self, sig: &Ed2kSignature) -> Result<RawPacket, Ed2kEncodeError> {
        encode_signature(&self.limits, sig)
    }

    /// Encodes an `OP_SECIDENTSTATE` (`0x87`) message into a `RawPacket`.
    pub fn encode_secident_state(
        &self,
        state: &Ed2kSecIdentState,
    ) -> Result<RawPacket, Ed2kEncodeError> {
        encode_secident_state(&self.limits, state)
    }

    /// Encodes any `Ed2kMessage` into its `RawPacket` representation.
    pub fn encode_message(&self, msg: &Ed2kMessage) -> Result<RawPacket, Ed2kEncodeError> {
        match msg {
            Ed2kMessage::Hello(hello) => self.encode_hello(hello),
            Ed2kMessage::HelloAnswer(answer) => self.encode_hello_answer(answer),
            Ed2kMessage::EmuleInfo(info) => self.encode_emule_info(info),
            Ed2kMessage::FileStatusReq(req) => self.encode_file_status_req(req),
            Ed2kMessage::FileStatus(status) => self.encode_file_status(status),
            Ed2kMessage::FileReqAnsNoFile(ans) => self.encode_file_req_ans_no_file(ans),
            Ed2kMessage::RequestParts(req) => self.encode_request_parts(req),
            Ed2kMessage::SendingPart(part) => self.encode_sending_part(part),
            Ed2kMessage::HashSetReq(req) => self.encode_hashset_request(req),
            Ed2kMessage::HashSetAnswer(ans) => self.encode_hashset_answer(ans),
            Ed2kMessage::AichFileHashReq(req) => self.encode_aich_file_hash_req(req),
            Ed2kMessage::AichFileHashRes(res) => self.encode_aich_file_hash_res(res),
            Ed2kMessage::AichRequest(req) => self.encode_aich_request(req),
            Ed2kMessage::AichAnswer(ans) => self.encode_aich_answer(ans),
            Ed2kMessage::PublicKey(key) => self.encode_public_key(key),
            Ed2kMessage::Signature(sig) => self.encode_signature(sig),
            Ed2kMessage::SecIdentState(state) => self.encode_secident_state(state),
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

    /// Encodes a Source Exchange request (`OP_REQUESTSOURCES` or `OP_REQUESTSOURCES2`).
    pub fn encode_request_sources(
        &self,
        req: &super::source_exchange::Ed2kRequestSources,
    ) -> Result<crate::framing::RawPacket, Ed2kEncodeError> {
        super::source_exchange::encode_request_sources(req)
    }

    /// Encodes a Source Exchange response (`OP_ANSWERSOURCES2`).
    pub fn encode_answer_sources(
        &self,
        ans: &super::source_exchange::Ed2kAnswerSources,
        force_compress: bool,
    ) -> Result<crate::framing::RawPacket, Ed2kEncodeError> {
        super::source_exchange::encode_answer_sources(ans, force_compress)
    }
}
