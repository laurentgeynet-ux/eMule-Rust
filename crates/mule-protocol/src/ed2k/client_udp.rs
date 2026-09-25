// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Client-to-client UDP messages (`ClientUDPSocket.cpp`, eMule 0.72a).
//!
//! These datagrams are sent on the shared UDP socket under the eMule
//! protocol byte (`0xC5`), alongside Kademlia traffic — exactly like
//! `CClientUDPSocket::ProcessPacket` which dispatches on the first byte.
//!
//! Supported opcodes:
//! - `OP_REASKFILEPING` (`0x90`) — keep-alive toward a queued source.
//! - `OP_REASKACK` (`0x91`) — carries the upload queue rank.
//! - `OP_FILENOTFOUND` (`0x92`) — requested file is not shared.
//! - `OP_QUEUEFULL` (`0x93`) — remote upload queue is full.
//! - `OP_REASKCALLBACKUDP` (`0x94`) — buddy relay toward a LowID source.
//! - `OP_DIRECTCALLBACKREQ` (`0x95`) — outbound-connect request toward a
//!   Kad-firewalled peer.
//! - `OP_PORTTEST` (`0xFE`) — UDP connectivity test (single `0x12` byte).

use super::opcodes::{
    OP_DIRECTCALLBACKREQ, OP_FILENOTFOUND, OP_PORTTEST, OP_QUEUEFULL, OP_REASKACK,
    OP_REASKCALLBACKUDP, OP_REASKFILEPING,
};
use super::protocol::PR_EMULE;

/// Decoded client-to-client UDP message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientUdpMessage {
    /// `OP_REASKFILEPING` — `hash16` + optional extended info
    /// (`u16` complete sources, preceded by a part status block for
    /// peers whose UDP version is > 3).
    ReaskFilePing {
        /// MD4 hash of the requested file.
        file_hash: [u8; 16],
        /// Announced complete sources count, when present.
        complete_sources: Option<u16>,
    },
    /// `OP_REASKACK` — queue rank (last `u16` of the payload; an
    /// optional part status bitfield may precede it for UDP ver > 3).
    ReaskAck {
        /// 1-based position in the remote upload queue.
        queue_rank: u16,
        /// Part status block when the peer sent one (UDP ver > 3).
        part_status: Option<UdpPartStatus>,
    },
    /// `OP_FILENOTFOUND` — the peer does not share the requested file.
    FileNotFound,
    /// `OP_QUEUEFULL` — the peer's upload queue is full.
    QueueFull,
    /// `OP_REASKCALLBACKUDP` — a requester asks us (as Kad buddy) to relay
    /// a file re-request to our buddy client over TCP.
    /// Payload: `[buddy_id:16][file_hash:16][part_status..][complete_src:2]`.
    ReaskCallback {
        /// Buddy ID the packet targets (must match our buddy's ID).
        buddy_id: [u8; 16],
        /// Remaining payload after the buddy ID: `file_hash` + extended
        /// re-request info, forwarded verbatim after the requester
        /// IP/port header (eMule `memcpy(packet + 10)` packing).
        rest: Vec<u8>,
    },
    /// `OP_DIRECTCALLBACKREQ` — the sender asks us (as a Kad-firewalled
    /// peer) to open an outbound TCP connection to it
    /// (`[tcp_port:2][userhash:16][connect_options:1]`).
    DirectCallbackReq {
        /// TCP port the requester listens on.
        tcp_port: u16,
        /// Requester user hash (identity + crypt layer key).
        user_hash: [u8; 16],
        /// eMule connection options byte (crypt support/request/require).
        conn_opts: u8,
    },
    /// `OP_PORTTEST` — UDP connectivity probe. eMule answers only a
    /// single `0x12` byte, and does so over its dedicated TCP test
    /// socket rather than on this UDP channel.
    PortTest {
        /// Single request byte (eMule expects `0x12`).
        request: u8,
    },
    /// Any other opcode received on the client UDP channel.
    Unknown {
        /// Raw opcode byte.
        opcode: u8,
    },
}

/// Part-status block carried by `OP_REASKACK` for UDP version > 3 peers:
/// `u16 part_count` followed by a `ceil(part_count/8)`-byte bitfield
/// (eMule `CUpDownClient::ProcessFileStatus` — the parts the remote
/// upload client has of the requested file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdpPartStatus {
    /// Total part count the bitfield describes.
    pub part_count: u16,
    /// Availability bitfield, `ceil(part_count/8)` bytes, bit *i* set =
    /// the peer has part *i*.
    pub bitfield: Vec<u8>,
}

/// Encodes `OP_REASKFILEPING` in the UDP version > 3 wire form:
/// `hash16 + u16 part_status_count + u16 complete_sources`
/// (`DownloadClient.cpp: UDPReaskForDownload` — the part status block is
/// empty `u16(0)` for files we complete, which is also harmless for
/// version ≤ 3 peers: they read the last `u16`).
pub fn encode_reask_file_ping(file_hash: &[u8; 16], complete_sources: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(22);
    out.push(PR_EMULE);
    out.push(OP_REASKFILEPING);
    out.extend_from_slice(file_hash);
    out.extend_from_slice(&0u16.to_le_bytes()); // empty part status
    out.extend_from_slice(&complete_sources.to_le_bytes());
    out
}

/// Encodes `OP_REASKACK`.
///
/// `include_part_status` mirrors eMule `sender->GetUDPVersion() > 3`: when
/// the requester's UDP version is > 3 the reply starts with an empty part
/// status block (`u16(0)` — shared files are always complete), followed by
/// the `u16` queue rank. Older peers receive only the rank.
pub fn encode_reask_ack(queue_rank: u16, include_part_status: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(6);
    out.push(PR_EMULE);
    out.push(OP_REASKACK);
    if include_part_status {
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    out.extend_from_slice(&queue_rank.to_le_bytes());
    out
}

/// Encodes `OP_FILENOTFOUND` (empty payload).
pub fn encode_file_not_found() -> Vec<u8> {
    vec![PR_EMULE, OP_FILENOTFOUND]
}

/// Connect-options byte bit 3 (`GetMyConnectOptions`): the peer is
/// TCP-firewalled but UDP-open and accepts `OP_DIRECTCALLBACKREQ`
/// (`OtherFunctions.cpp:4049-4052`). Carried in MISCOPTIONS2 bit 12 of the
/// eD2K hello and in `TAG_ENCRYPTION` of published Kad sources.
pub const CONNECT_OPT_DIRECT_CALLBACK: u8 = 0x08;

/// Encodes `OP_DIRECTCALLBACKREQ` (`0x95`) — eMule `BaseClient.cpp:1411-1415`
/// (`TryToConnect` step 4): we ask a TCP-unreachable peer (announced via
/// `SupportsDirectUDPCallback`, MISCOPTIONS2 bit 12) to dial back our TCP
/// port. Payload: `<our TCP port u16><our userhash 16><connect options u8>`
/// — the options byte is `GetMyConnectOptions(true, false)` (crypt flags
/// only, direct-callback bit clear).
pub fn encode_direct_callback_req(tcp_port: u16, user_hash: &[u8; 16], conn_opts: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + 19);
    out.push(PR_EMULE);
    out.push(OP_DIRECTCALLBACKREQ);
    out.extend_from_slice(&tcp_port.to_le_bytes());
    out.extend_from_slice(user_hash);
    out.push(conn_opts);
    out
}

/// Encodes `OP_QUEUEFULL` (empty payload).
pub fn encode_queue_full() -> Vec<u8> {
    vec![PR_EMULE, OP_QUEUEFULL]
}

/// Decodes a client-to-client UDP datagram (first byte must be `0xC5` or
/// `0xE3`; Kademlia protocol bytes are rejected so the caller can route).
pub fn decode_client_udp(data: &[u8]) -> Result<ClientUdpMessage, String> {
    if data.len() < 2 {
        return Err("client UDP datagram too short".into());
    }
    let payload = &data[2..];
    match data[1] {
        OP_REASKFILEPING => {
            if payload.len() < 16 {
                return Err("truncated OP_REASKFILEPING".into());
            }
            let mut file_hash = [0u8; 16];
            file_hash.copy_from_slice(&payload[..16]);
            // Extended info (UDP ver > 2): trailing u16 = complete sources.
            let complete_sources = if payload.len() >= 18 {
                Some(u16::from_le_bytes([
                    payload[payload.len() - 2],
                    payload[payload.len() - 1],
                ]))
            } else {
                None
            };
            Ok(ClientUdpMessage::ReaskFilePing {
                file_hash,
                complete_sources,
            })
        }
        OP_REASKACK => {
            if payload.len() < 2 {
                return Err("truncated OP_REASKACK".into());
            }
            let rank_off = payload.len() - 2;
            let queue_rank = u16::from_le_bytes([payload[rank_off], payload[rank_off + 1]]);
            // UDP ver > 3 form: u16 bitfield count + ceil(count/8) bytes.
            let part_status = if rank_off >= 2 {
                let count = u16::from_le_bytes([payload[0], payload[1]]);
                let bitfield_len = (count as usize).div_ceil(8);
                if 2 + bitfield_len == rank_off {
                    Some(UdpPartStatus {
                        part_count: count,
                        bitfield: payload[2..rank_off].to_vec(),
                    })
                } else {
                    None
                }
            } else {
                None
            };
            Ok(ClientUdpMessage::ReaskAck {
                queue_rank,
                part_status,
            })
        }
        OP_FILENOTFOUND => Ok(ClientUdpMessage::FileNotFound),
        OP_QUEUEFULL => Ok(ClientUdpMessage::QueueFull),
        OP_DIRECTCALLBACKREQ => {
            if payload.len() < 19 {
                return Err("truncated OP_DIRECTCALLBACKREQ".into());
            }
            let mut user_hash = [0u8; 16];
            user_hash.copy_from_slice(&payload[2..18]);
            Ok(ClientUdpMessage::DirectCallbackReq {
                tcp_port: u16::from_le_bytes([payload[0], payload[1]]),
                user_hash,
                conn_opts: payload[18],
            })
        }
        OP_PORTTEST => {
            // eMule only honours a single-byte `0x12` probe.
            if payload.len() != 1 {
                return Err("malformed OP_PORTTEST".into());
            }
            Ok(ClientUdpMessage::PortTest {
                request: payload[0],
            })
        }
        OP_REASKCALLBACKUDP => {
            if payload.len() < 17 {
                return Err("truncated OP_REASKCALLBACKUDP".into());
            }
            let mut buddy_id = [0u8; 16];
            buddy_id.copy_from_slice(&payload[..16]);
            Ok(ClientUdpMessage::ReaskCallback {
                buddy_id,
                rest: payload[16..].to_vec(),
            })
        }
        other => Ok(ClientUdpMessage::Unknown { opcode: other }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_wire_format() {
        let hash = [0xAB; 16];
        let pkt = encode_reask_file_ping(&hash, 7);
        assert_eq!(pkt.len(), 22);
        assert_eq!(pkt[0], 0xC5);
        assert_eq!(pkt[1], 0x90);
        assert_eq!(&pkt[2..18], &hash);
        assert_eq!(&pkt[18..20], &[0, 0]); // empty part status
        assert_eq!(&pkt[20..22], &7u16.to_le_bytes());
    }

    #[test]
    fn decode_ping_v2_and_v4() {
        let hash = [1u8; 16];
        // v2 form: hash only
        let mut v2 = vec![PR_EMULE, OP_REASKFILEPING];
        v2.extend_from_slice(&hash);
        match decode_client_udp(&v2).unwrap() {
            ClientUdpMessage::ReaskFilePing {
                file_hash,
                complete_sources,
            } => {
                assert_eq!(file_hash, hash);
                assert_eq!(complete_sources, None);
            }
            m => panic!("unexpected {m:?}"),
        }
        // v>3 form: hash + u16 status count + u16 complete
        match decode_client_udp(&encode_reask_file_ping(&hash, 42)).unwrap() {
            ClientUdpMessage::ReaskFilePing {
                complete_sources, ..
            } => assert_eq!(complete_sources, Some(42)),
            m => panic!("unexpected {m:?}"),
        }
    }

    #[test]
    fn ack_with_and_without_part_status() {
        // Plain rank
        match decode_client_udp(&encode_reask_ack(5, false)).unwrap() {
            ClientUdpMessage::ReaskAck {
                queue_rank,
                part_status,
            } => {
                assert_eq!(queue_rank, 5);
                assert!(part_status.is_none());
            }
            m => panic!("unexpected {m:?}"),
        }
        // With empty part status
        match decode_client_udp(&encode_reask_ack(9, true)).unwrap() {
            ClientUdpMessage::ReaskAck {
                queue_rank,
                part_status,
            } => {
                assert_eq!(queue_rank, 9);
                assert_eq!(
                    part_status,
                    Some(UdpPartStatus {
                        part_count: 0,
                        bitfield: vec![]
                    })
                );
            }
            m => panic!("unexpected {m:?}"),
        }
        // With a real bitfield: count=3 + 1 byte bitfield + rank
        let raw = vec![PR_EMULE, OP_REASKACK, 3, 0, 0b101, 17, 0];
        match decode_client_udp(&raw).unwrap() {
            ClientUdpMessage::ReaskAck {
                queue_rank,
                part_status,
            } => {
                assert_eq!(queue_rank, 17);
                assert_eq!(
                    part_status,
                    Some(UdpPartStatus {
                        part_count: 3,
                        bitfield: vec![0b101]
                    })
                );
            }
            m => panic!("unexpected {m:?}"),
        }
    }

    #[test]
    fn queuefull_and_filenotfound() {
        assert_eq!(
            decode_client_udp(&encode_queue_full()).unwrap(),
            ClientUdpMessage::QueueFull
        );
        assert_eq!(
            decode_client_udp(&encode_file_not_found()).unwrap(),
            ClientUdpMessage::FileNotFound
        );
        assert_eq!(
            decode_client_udp(&[PR_EMULE, 0x55, 1]).unwrap(),
            ClientUdpMessage::Unknown { opcode: 0x55 }
        );
    }

    #[test]
    fn direct_callback_req_wire_format() {
        // eMule Opcodes.h: <TCPPort 2><Userhash 16><ConnectionOptions 1>
        let mut raw = vec![PR_EMULE, OP_DIRECTCALLBACKREQ];
        raw.extend_from_slice(&49662u16.to_le_bytes());
        raw.extend_from_slice(&[0xAB; 16]);
        raw.push(0x07);
        match decode_client_udp(&raw).unwrap() {
            ClientUdpMessage::DirectCallbackReq {
                tcp_port,
                user_hash,
                conn_opts,
            } => {
                assert_eq!(tcp_port, 49662);
                assert_eq!(user_hash, [0xAB; 16]);
                assert_eq!(conn_opts, 0x07);
            }
            m => panic!("unexpected {m:?}"),
        }
        // Truncated (< 19-byte payload) is rejected.
        assert!(decode_client_udp(&raw[..18]).is_err());

        // Outgoing encoder (`BaseClient.cpp:1411-1415`) must produce the
        // exact same wire bytes — round-trip covers framing + field order.
        let encoded = encode_direct_callback_req(49662, &[0xAB; 16], 0x07);
        assert_eq!(encoded, raw);
        match decode_client_udp(&encoded).unwrap() {
            ClientUdpMessage::DirectCallbackReq {
                tcp_port,
                user_hash,
                conn_opts,
            } => {
                assert_eq!(tcp_port, 49662);
                assert_eq!(user_hash, [0xAB; 16]);
                assert_eq!(conn_opts, 0x07);
            }
            m => panic!("unexpected {m:?}"),
        }
    }

    #[test]
    fn port_test_wire_format() {
        match decode_client_udp(&[PR_EMULE, OP_PORTTEST, 0x12]).unwrap() {
            ClientUdpMessage::PortTest { request } => assert_eq!(request, 0x12),
            m => panic!("unexpected {m:?}"),
        }
        // eMule ignores anything but a single request byte.
        assert!(decode_client_udp(&[PR_EMULE, OP_PORTTEST]).is_err());
        assert!(decode_client_udp(&[PR_EMULE, OP_PORTTEST, 0x12, 0x34]).is_err());
    }
}
