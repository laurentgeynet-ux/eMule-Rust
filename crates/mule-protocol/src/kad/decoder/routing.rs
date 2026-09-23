// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! Decoders for Kad bootstrap and find node messages.

use crate::ed2k::{Ed2kErrorKind, Ed2kParseError};
use crate::kad::types::*;
use bytes::Buf;

/// Decodes an `OP_KADEMLIA2_PING` payload.
pub fn decode_ping(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 18 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadPing",
            Ed2kErrorKind::UnexpectedEof {
                needed: 18,
                available: payload.len(),
            },
        ));
    }
    let mut kad_id = [0u8; 16];
    payload.copy_to_slice(&mut kad_id);
    let udp_port = payload.get_u16_le();
    Ok(KadMessage::Ping(KadPing { kad_id, udp_port }))
}

/// Decodes an `OP_KADEMLIA2_PONG` payload.
pub fn decode_pong(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 2 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadPong",
            Ed2kErrorKind::UnexpectedEof {
                needed: 2,
                available: payload.len(),
            },
        ));
    }
    let client_port = payload.get_u16_le();
    Ok(KadMessage::Pong(KadPong { client_port }))
}

/// Shared hello body parser. eMule `SendMyDetails` layout:
/// `[kadID 16][tcp_port u16][version u8][tag_count u8][tags]` where the
/// advertised UDP port lives in `TAG_SOURCEUPORT` (0xFC) and firewall /
/// ack-request bits in `TAG_KADMISCOPTIONS` (0xF2, Kad >= 8).
fn decode_hello_body(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<(KadIdBytes, u16, u8, u16, Option<u8>), Ed2kParseError> {
    if payload.len() < 20 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadHello",
            Ed2kErrorKind::UnexpectedEof {
                needed: 20,
                available: payload.len(),
            },
        ));
    }
    let mut kad_id = [0u8; 16];
    kad_id.copy_from_slice(&payload[0..16]);
    let tcp_port = u16::from_le_bytes([payload[16], payload[17]]);
    let version = payload[18];
    let tag_count = payload[19];

    let mut udp_port = 0u16;
    let mut misc_options = None;
    let mut reader = crate::ed2k::tags::TagReader::new(&payload[20..], 0, protocol, Some(opcode));
    let limits = crate::ed2k::limits::Ed2kLimits::default();
    for _ in 0..tag_count {
        let (typed_opt, _raw) = reader.read_tag(&limits)?;
        if let Some(tag) = typed_opt {
            match &tag.name {
                crate::ed2k::tags::TagName::Id(crate::kad::opcodes::TAG_SOURCEUPORT) => {
                    match tag.value {
                        crate::ed2k::tags::TagValue::UInt16(p) => udp_port = p,
                        crate::ed2k::tags::TagValue::UInt32(p) => udp_port = p as u16,
                        _ => {}
                    }
                }
                crate::ed2k::tags::TagName::Id(crate::kad::opcodes::TAG_KADMISCOPTIONS) => {
                    match tag.value {
                        crate::ed2k::tags::TagValue::UInt8(v) => misc_options = Some(v),
                        crate::ed2k::tags::TagValue::UInt16(v) => misc_options = Some(v as u8),
                        crate::ed2k::tags::TagValue::UInt32(v) => misc_options = Some(v as u8),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    Ok((kad_id, tcp_port, version, udp_port, misc_options))
}

/// Decodes an `OP_KADEMLIA2_HELLO_REQ` payload.
pub fn decode_hello_req(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    let (kad_id, tcp_port, version, udp_port, misc_options) =
        decode_hello_body(protocol, opcode, payload)?;
    Ok(KadMessage::HelloReq(KadHelloReq {
        kad_id,
        udp_port,
        version,
        tcp_port,
        misc_options,
    }))
}

/// Decodes an `OP_KADEMLIA2_HELLO_RES` payload.
pub fn decode_hello_res(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    let (kad_id, tcp_port, version, udp_port, misc_options) =
        decode_hello_body(protocol, opcode, payload)?;
    Ok(KadMessage::HelloRes(KadHelloRes {
        kad_id,
        udp_port,
        version,
        tcp_port,
        misc_options,
    }))
}

/// Decodes an `KADEMLIA2_HELLO_RES_ACK` (0x22) payload: `[kadID 16][tag_count u8][tags]`.
pub fn decode_hello_res_ack(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 17 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadHelloResAck",
            Ed2kErrorKind::UnexpectedEof {
                needed: 17,
                available: payload.len(),
            },
        ));
    }
    let mut kad_id = [0u8; 16];
    kad_id.copy_from_slice(&payload[0..16]);
    Ok(KadMessage::HelloResAck(KadHelloResAck { kad_id }))
}

/// Decodes a `KADEMLIA_FIREWALLED_REQ` (0x50) payload: `[tcp_port u16]`.
pub fn decode_firewalled_req(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 2 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadFirewalledReq",
            Ed2kErrorKind::UnexpectedEof {
                needed: 2,
                available: payload.len(),
            },
        ));
    }
    Ok(KadMessage::FirewalledReq(KadFirewalledReq {
        tcp_port: u16::from_le_bytes([payload[0], payload[1]]),
    }))
}

/// Decodes a `KADEMLIA_FIREWALLED2_REQ` (0x53) payload:
/// `[tcp_port u16][user_hash 16][connect_options u8]` — 19 bytes.
pub fn decode_firewalled2_req(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 19 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadFirewalledReq2",
            Ed2kErrorKind::UnexpectedEof {
                needed: 19,
                available: payload.len(),
            },
        ));
    }
    let mut user_hash = [0u8; 16];
    user_hash.copy_from_slice(&payload[2..18]);
    Ok(KadMessage::FirewalledReq2(KadFirewalledReq2 {
        tcp_port: u16::from_le_bytes([payload[0], payload[1]]),
        user_hash,
        connect_options: payload[18],
    }))
}

/// Decodes a `KADEMLIA_FIREWALLED_RES` (0x58) payload: `[ip u32]`.
pub fn decode_firewalled_res(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() != 4 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadFirewalledRes",
            Ed2kErrorKind::UnexpectedEof {
                needed: 4,
                available: payload.len(),
            },
        ));
    }
    Ok(KadMessage::FirewalledRes(KadFirewalledRes {
        ip: u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]),
    }))
}

/// Decodes an `OP_KADEMLIA2_BOOTSTRAP_REQ` payload.
pub fn decode_bootstrap_req(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.is_empty() {
        Ok(KadMessage::BootstrapReq(KadBootstrapReq {
            kad_id: [0u8; 16],
            udp_port: 0,
        }))
    } else if payload.len() >= 18 {
        let mut kad_id = [0u8; 16];
        payload.copy_to_slice(&mut kad_id);
        let udp_port = payload.get_u16_le();
        Ok(KadMessage::BootstrapReq(KadBootstrapReq {
            kad_id,
            udp_port,
        }))
    } else {
        Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadBootstrapReq",
            Ed2kErrorKind::UnexpectedEof {
                needed: 18,
                available: payload.len(),
            },
        ))
    }
}

/// Decodes an `OP_KADEMLIA2_BOOTSTRAP_RES` payload.
pub fn decode_bootstrap_res(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 21 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadBootstrapRes",
            Ed2kErrorKind::UnexpectedEof {
                needed: 21,
                available: payload.len(),
            },
        ));
    }
    let mut kad_id = [0u8; 16];
    payload.copy_to_slice(&mut kad_id);
    let udp_port = payload.get_u16_le();
    let version = payload.get_u8();
    let count = payload.get_u16_le() as usize;

    if count > 100 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            21,
            "KadBootstrapRes::contacts",
            Ed2kErrorKind::FrameTooLarge {
                size: count,
                limit: 100,
            },
        ));
    }

    let max_contacts = (payload.len() / 25).min(count).min(MAX_KAD_RES_CONTACTS);
    let mut contacts = Vec::with_capacity(max_contacts);
    for _ in 0..max_contacts {
        let mut c_id = [0u8; 16];
        payload.copy_to_slice(&mut c_id);
        let mut raw_ip = [0u8; 4];
        payload.copy_to_slice(&mut raw_ip);
        // eMule writes contact IP via WriteUInt32(uIP) where uIP is in host byte order.
        // On x86 wire, this writes bytes [d, c, b, a] for IPv4 a.b.c.d.
        let ip = [raw_ip[3], raw_ip[2], raw_ip[1], raw_ip[0]];
        let c_udp = payload.get_u16_le();
        let c_tcp = payload.get_u16_le();
        let c_ver = payload.get_u8();
        contacts.push(KadContactWire {
            kad_id: c_id,
            ip,
            udp_port: c_udp,
            tcp_port: c_tcp,
            version: c_ver,
        });
    }

    Ok(KadMessage::BootstrapRes(KadBootstrapRes {
        kad_id,
        udp_port,
        version,
        contacts,
    }))
}

/// Decodes an `OP_KADEMLIA2_REQ` payload.
pub fn decode_req(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 33 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadReq",
            Ed2kErrorKind::UnexpectedEof {
                needed: 33,
                available: payload.len(),
            },
        ));
    }
    let req_type = payload.get_u8();
    let mut target_id = [0u8; 16];
    payload.copy_to_slice(&mut target_id);
    let mut sender_id = [0u8; 16];
    payload.copy_to_slice(&mut sender_id);
    Ok(KadMessage::Req(KadReq {
        req_type,
        target_id,
        sender_id,
    }))
}

/// Decodes an `OP_KADEMLIA2_RES` payload (0x29, Find Node response).
/// Official format: [target_id: 16] [count: u8] then count * 25-byte contact records:
/// [kad_id: 16] [ip: 4] [udp_port: 2] [tcp_port: 2] [version: 1].
pub fn decode_res(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 17 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            17,
            "KadRes",
            Ed2kErrorKind::UnexpectedEof {
                needed: 17,
                available: payload.len(),
            },
        ));
    }

    let mut target_id = [0u8; 16];
    payload.copy_to_slice(&mut target_id);
    let count = payload.get_u8() as usize;
    let max_contacts = (payload.len() / 25).min(count).min(MAX_KAD_RES_CONTACTS);
    let mut contacts = Vec::with_capacity(max_contacts);
    for _ in 0..max_contacts {
        let mut c_id = [0u8; 16];
        payload.copy_to_slice(&mut c_id);
        let mut raw_ip = [0u8; 4];
        payload.copy_to_slice(&mut raw_ip);
        // eMule writes contact IP via WriteUInt32(uIP) where uIP is in host byte order.
        // On x86 wire, this writes bytes [d, c, b, a] for IPv4 a.b.c.d.
        let ip = [raw_ip[3], raw_ip[2], raw_ip[1], raw_ip[0]];
        let c_udp = payload.get_u16_le();
        let c_tcp = payload.get_u16_le();
        let c_ver = payload.get_u8();
        contacts.push(KadContactWire {
            kad_id: c_id,
            ip,
            udp_port: c_udp,
            tcp_port: c_tcp,
            version: c_ver,
        });
    }

    Ok(KadMessage::BootstrapRes(KadBootstrapRes {
        kad_id: target_id,
        udp_port: 0,
        version: 2,
        contacts,
    }))
}
