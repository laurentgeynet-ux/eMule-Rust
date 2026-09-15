//! Bounded encoding and decoding for Kad UDP packets (Ping, Pong, Hello, HelloAnswer, Bootstrap, Req/Res).

use bytes::{Buf, BufMut, Bytes, BytesMut};
use crate::ed2k::{Ed2kErrorKind, Ed2kParseError};
use crate::kad::opcodes::*;

/// 128-bit Kad ID alias (16 bytes).
pub type KadIdBytes = [u8; 16];

/// Maximum contacts in a Kad2 Bootstrap or Find Node response (standard K = 20).
pub const MAX_KAD_RES_CONTACTS: usize = 20;

/// Wire representation of a Kad contact in responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadContactWire {
    /// 128-bit Kad ID.
    pub kad_id: KadIdBytes,
    /// IPv4 address as u32 in network order.
    pub ip: [u8; 4],
    /// UDP port.
    pub udp_port: u16,
    /// TCP port (optional, 0 if not set).
    pub tcp_port: u16,
    /// Kad version.
    pub version: u8,
}

/// Kad2 Ping packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadPing {
    /// Sender/receiver Kad ID.
    pub kad_id: KadIdBytes,
    /// UDP port.
    pub udp_port: u16,
}

/// Kad2 Pong packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadPong {
    /// Port seen by remote peer.
    pub client_port: u16,
}

/// Kad2 Hello Request packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadHelloReq {
    /// Sender Kad ID.
    pub kad_id: KadIdBytes,
    /// Sender UDP port.
    pub udp_port: u16,
    /// Kad version (e.g., 2).
    pub version: u8,
    /// Sender TCP port (optional, 0 if none).
    pub tcp_port: u16,
}

/// Kad2 Hello Response packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadHelloRes {
    /// Responder Kad ID.
    pub kad_id: KadIdBytes,
    /// Responder UDP port.
    pub udp_port: u16,
    /// Kad version.
    pub version: u8,
    /// Responder TCP port (optional).
    pub tcp_port: u16,
}

/// Kad2 Bootstrap Request packet (0x21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadBootstrapReq {
    /// Sender Kad ID.
    pub kad_id: KadIdBytes,
    /// Sender UDP port.
    pub udp_port: u16,
}

/// Kad2 Bootstrap Response / Find Node Response packet (0x29).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadBootstrapRes {
    /// Responder Kad ID.
    pub kad_id: KadIdBytes,
    /// Responder UDP port.
    pub udp_port: u16,
    /// Kad version.
    pub version: u8,
    /// List of contacts returned (bounded to MAX_KAD_RES_CONTACTS).
    pub contacts: Vec<KadContactWire>,
}

/// Kad2 Request packet (Find Node / Search, 0x21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadReq {
    /// Request type (e.g. 0 for FIND_NODE).
    pub req_type: u8,
    /// Target Kad ID sought.
    pub target_id: KadIdBytes,
    /// Sender Kad ID.
    pub sender_id: KadIdBytes,
}

/// Enum representing decoded Kad UDP messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KadMessage {
    /// Kad2 Ping request.
    Ping(KadPing),
    /// Kad2 Pong response.
    Pong(KadPong),
    /// Kad2 Hello request.
    HelloReq(KadHelloReq),
    /// Kad2 Hello response.
    HelloRes(KadHelloRes),
    /// Kad2 Bootstrap request.
    BootstrapReq(KadBootstrapReq),
    /// Kad2 Bootstrap / Find Node response.
    BootstrapRes(KadBootstrapRes),
    /// Kad2 Generic Find Node / Search request.
    Req(KadReq),
}

/// Encoder and decoder for Kad UDP wire datagrams.
pub struct KadCodec;

impl KadCodec {
    /// Decodes a raw UDP packet into a KadMessage.
    /// Invariant: raw_packet must not exceed MAX_KAD_UDP_PACKET_SIZE.
    pub fn decode(raw: &[u8]) -> Result<KadMessage, Ed2kParseError> {
        if raw.len() > MAX_KAD_UDP_PACKET_SIZE {
            return Err(Ed2kParseError::new(
                raw.first().copied().unwrap_or(PR_KADEMLIA),
                None,
                0,
                "KadUdpPacket",
                Ed2kErrorKind::FrameTooLarge {
                    size: raw.len(),
                    limit: MAX_KAD_UDP_PACKET_SIZE,
                },
            ));
        }
        if raw.len() < 2 {
            return Err(Ed2kParseError::new(
                raw.first().copied().unwrap_or(PR_KADEMLIA),
                None,
                0,
                "KadUdpPacket",
                Ed2kErrorKind::UnexpectedEof {
                    needed: 2,
                    available: raw.len(),
                },
            ));
        }

        let protocol = raw[0];
        if !is_valid_kad_protocol(protocol) {
            return Err(Ed2kParseError::new(
                protocol,
                None,
                0,
                "KadUdpPacket::protocol",
                Ed2kErrorKind::UnsupportedProtocol { byte: protocol },
            ));
        }

        let opcode = raw[1];
        let mut payload = &raw[2..];

        match opcode {
            KADEMLIA2_PING => {
                // Sender Kad ID (16 bytes) + UDP port (2 bytes) = 18 bytes
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
            KADEMLIA2_PONG => {
                // Client port (2 bytes)
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
            KADEMLIA2_HELLO_REQ | KADEMLIA_HELLO_REQ => {
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
            KADEMLIA2_HELLO_RES | KADEMLIA_HELLO_RES => {
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
            KADEMLIA2_BOOTSTRAP_REQ => {
                if payload.len() == 18 {
                    // Bootstrap req: Kad ID (16 bytes) + UDP port (2 bytes)
                    let mut kad_id = [0u8; 16];
                    payload.copy_to_slice(&mut kad_id);
                    let udp_port = payload.get_u16_le();
                    Ok(KadMessage::BootstrapReq(KadBootstrapReq { kad_id, udp_port }))
                } else if payload.len() >= 33 {
                    // Find Node Req: req_type (1 byte) + target_id (16 bytes) + sender_id (16 bytes)
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
                } else {
                    Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        2,
                        "KadReq",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: 18,
                            available: payload.len(),
                        },
                    ))
                }
            }
            KADEMLIA2_BOOTSTRAP_RES => {
                // Kad ID (16 bytes) + UDP port (2 bytes) + Version (1 byte) + Count (2 bytes) = 21 bytes header
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

                if count > MAX_KAD_RES_CONTACTS {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        21,
                        "KadBootstrapRes::contacts",
                        Ed2kErrorKind::FrameTooLarge {
                            size: count,
                            limit: MAX_KAD_RES_CONTACTS,
                        },
                    ));
                }

                // Each contact is: Kad ID (16 bytes) + IP (4 bytes) + UDP port (2 bytes) + TCP port (2 bytes) + Version (1 byte) = 25 bytes
                let needed_bytes = count * 25;
                if payload.len() < needed_bytes {
                    return Err(Ed2kParseError::new(
                        protocol,
                        Some(opcode),
                        21,
                        "KadBootstrapRes::contacts",
                        Ed2kErrorKind::UnexpectedEof {
                            needed: needed_bytes,
                            available: payload.len(),
                        },
                    ));
                }

                let mut contacts = Vec::with_capacity(count);
                for _ in 0..count {
                    let mut c_id = [0u8; 16];
                    payload.copy_to_slice(&mut c_id);
                    let mut ip = [0u8; 4];
                    payload.copy_to_slice(&mut ip);
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
            _ => Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                1,
                "KadUdpPacket::opcode",
                Ed2kErrorKind::MalformedField {
                    message: format!("unknown kad opcode 0x{opcode:02X}"),
                },
            )),
        }
    }

    /// Encodes a KadPing request packet.
    pub fn encode_ping(kad_id: &KadIdBytes, udp_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(20);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_PING);
        buf.put_slice(kad_id);
        buf.put_u16_le(udp_port);
        buf.freeze()
    }

    /// Encodes a KadPong response packet.
    pub fn encode_pong(client_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(4);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_PONG);
        buf.put_u16_le(client_port);
        buf.freeze()
    }

    /// Encodes a Kad2 Hello request packet.
    pub fn encode_hello_req(kad_id: &KadIdBytes, udp_port: u16, version: u8, tcp_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(23);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_HELLO_REQ);
        buf.put_slice(kad_id);
        buf.put_u16_le(udp_port);
        buf.put_u8(version);
        buf.put_u16_le(tcp_port);
        buf.freeze()
    }

    /// Encodes a Kad2 Hello response packet.
    pub fn encode_hello_res(kad_id: &KadIdBytes, udp_port: u16, version: u8, tcp_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(23);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_HELLO_RES);
        buf.put_slice(kad_id);
        buf.put_u16_le(udp_port);
        buf.put_u8(version);
        buf.put_u16_le(tcp_port);
        buf.freeze()
    }

    /// Encodes a Kad2 Bootstrap request packet.
    pub fn encode_bootstrap_req(kad_id: &KadIdBytes, udp_port: u16) -> Bytes {
        let mut buf = BytesMut::with_capacity(20);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_BOOTSTRAP_REQ);
        buf.put_slice(kad_id);
        buf.put_u16_le(udp_port);
        buf.freeze()
    }

    /// Encodes a Kad2 Bootstrap / Find Node response packet.
    pub fn encode_bootstrap_res(
        kad_id: &KadIdBytes,
        udp_port: u16,
        version: u8,
        contacts: &[KadContactWire],
    ) -> Bytes {
        let count = contacts.len().min(MAX_KAD_RES_CONTACTS);
        let mut buf = BytesMut::with_capacity(2 + 16 + 2 + 1 + 2 + count * 25);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_BOOTSTRAP_RES);
        buf.put_slice(kad_id);
        buf.put_u16_le(udp_port);
        buf.put_u8(version);
        buf.put_u16_le(count as u16);
        for c in &contacts[..count] {
            buf.put_slice(&c.kad_id);
            buf.put_slice(&c.ip);
            buf.put_u16_le(c.udp_port);
            buf.put_u16_le(c.tcp_port);
            buf.put_u8(c.version);
        }
        buf.freeze()
    }

    /// Encodes a Kad2 Find Node request packet.
    pub fn encode_find_node_req(target_id: &KadIdBytes, sender_id: &KadIdBytes) -> Bytes {
        let mut buf = BytesMut::with_capacity(35);
        buf.put_u8(PR_KADEMLIA);
        buf.put_u8(KADEMLIA2_REQ);
        buf.put_u8(0); // 0 = Find Node
        buf.put_slice(target_id);
        buf.put_slice(sender_id);
        buf.freeze()
    }
}
