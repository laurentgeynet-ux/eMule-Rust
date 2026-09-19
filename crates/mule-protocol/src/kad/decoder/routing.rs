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

/// Decodes an `OP_KADEMLIA2_HELLO_REQ` payload.
pub fn decode_hello_req(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 19 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadHelloReq",
            Ed2kErrorKind::UnexpectedEof {
                needed: 19,
                available: payload.len(),
            },
        ));
    }
    let mut kad_id = [0u8; 16];
    payload.copy_to_slice(&mut kad_id);
    let udp_port = payload.get_u16_le();
    let version = payload.get_u8();
    let tcp_port = if payload.len() >= 2 {
        payload.get_u16_le()
    } else {
        0
    };
    Ok(KadMessage::HelloReq(KadHelloReq {
        kad_id,
        udp_port,
        version,
        tcp_port,
    }))
}

/// Decodes an `OP_KADEMLIA2_HELLO_RES` payload.
pub fn decode_hello_res(
    protocol: u8,
    opcode: u8,
    mut payload: &[u8],
) -> Result<KadMessage, Ed2kParseError> {
    if payload.len() < 19 {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            2,
            "KadHelloRes",
            Ed2kErrorKind::UnexpectedEof {
                needed: 19,
                available: payload.len(),
            },
        ));
    }
    let mut kad_id = [0u8; 16];
    payload.copy_to_slice(&mut kad_id);
    let udp_port = payload.get_u16_le();
    let version = payload.get_u8();
    let tcp_port = if payload.len() >= 2 {
        payload.get_u16_le()
    } else {
        0
    };
    Ok(KadMessage::HelloRes(KadHelloRes {
        kad_id,
        udp_port,
        version,
        tcp_port,
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
