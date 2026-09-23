// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Server UDP wire messages (`UDPSocket.cpp`, eMule 0.72a).
//!
//! Global server queries are sent over a dedicated UDP socket to each
//! server's UDP port (its TCP port + 4 for Lugdunum servers), under the
//! `PR_ED2K` (`0xE3`) protocol byte:
//! - `OP_GLOBGETSOURCES2` (`0x94`) — sources for a file hash.
//! - `OP_GLOBFOUNDSOURCES` (`0x9B`) — multi-file source answers.
//! - `OP_GLOBSERVSTATREQ`/`RES` (`0x96`/`0x97`) — server statistics.

use super::opcodes::{
    OP_GLOBFOUNDSOURCES, OP_GLOBGETSOURCES2, OP_GLOBSEARCHREQ, OP_GLOBSEARCHREQ2,
    OP_GLOBSEARCHREQ3, OP_GLOBSEARCHRES, OP_GLOBSERVSTATREQ, OP_GLOBSERVSTATRES,
};

/// eMule `SRV_UDPFLG_EXT_GETFILES` — the server answers global UDP search.
pub const SRV_UDPFLG_EXT_GETFILES: u32 = 0x0000_0002;
/// eMule `SRV_UDPFLG_LARGEFILES` — the server handles >4 GiB entries.
pub const SRV_UDPFLG_LARGEFILES: u32 = 0x0000_0100;

/// eMule `CT_SERVER_UDPSEARCH_FLAGS` (`0x0E`) tag name id, sent as the single
/// tag of `OP_GLOBSEARCHREQ3`.
pub const CT_SERVER_UDPSEARCH_FLAGS: u8 = 0x0E;
/// eMule `SRVCAP_UDP_NEWTAGS_LARGEFILES` — value carried by that tag.
pub const SRVCAP_UDP_NEWTAGS_LARGEFILES: u8 = 0x01;
use super::protocol::{PR_ED2K, PR_EMULE_COMPRESSED};
use super::tags::TagReader;

/// zlib-inflates a packed UDP payload with a hard cap (eMule
/// `Packet::UnPackPacket` — server packets cap at 250 KB).
fn unpack_udp_payload(data: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    const MAX_DECOMPRESSED: u64 = 250_000;
    let mut decoder = flate2::read::ZlibDecoder::new(data).take(MAX_DECOMPRESSED + 1);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    if out.is_empty() || out.len() as u64 > MAX_DECOMPRESSED {
        return None;
    }
    Some(out)
}

/// One file's found sources inside an `OP_GLOBFOUNDSOURCES` datagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobFoundSourcesEntry {
    /// File hash the sources refer to.
    pub file_hash: [u8; 16],
    /// `(ip_le, port)` source endpoints.
    pub sources: Vec<(u32, u16)>,
}

/// Decoded server UDP message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerUdpMessage {
    /// `OP_GLOBFOUNDSOURCES` — sources grouped per file hash.
    GlobFoundSources(Vec<GlobFoundSourcesEntry>),
    /// `OP_GLOBSERVSTATRES` — server status answer (eMule
    /// `CUDPSocket::ProcessPacket`): `[challenge:u32][users][files]
    /// [maxusers][softfiles][hardfiles][udp_flags][lowid_users]
    /// [udp_obf_port:u16][tcp_obf_port:u16][udp_key:u32]`.
    GlobServStatRes {
        /// Challenge echo (the u32 sent in `OP_GLOBSERVSTATREQ`).
        challenge: u32,
        /// Connected users reported by the server.
        users: u32,
        /// Indexed files reported by the server.
        files: u32,
        /// `SRV_UDPFLG_*` capability bitmask (0 when the server is old).
        udp_flags: u32,
        /// Dedicated TCP obfuscation port (0 = none).
        tcp_obfuscation_port: u16,
        /// Dedicated UDP obfuscation port (0 = none).
        udp_obfuscation_port: u16,
    },
    /// `OP_GLOBSEARCHRES` — raw payload kept for the caller (same entry
    /// layout as `OP_SEARCHRESULT`).
    GlobSearchRes(Vec<u8>),
    /// Any other opcode on the server UDP channel.
    Unknown {
        /// Raw opcode byte.
        opcode: u8,
    },
}

/// Encodes `OP_GLOBGETSOURCES2` (`hash16 + u32 filesize`,
/// `UDPSocket::SendPacket` for source requests).
pub fn encode_glob_get_sources2(file_hash: &[u8; 16], file_size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(22);
    out.push(PR_ED2K);
    out.push(OP_GLOBGETSOURCES2);
    out.extend_from_slice(file_hash);
    out.extend_from_slice(&file_size.to_le_bytes());
    out
}

/// Encodes `OP_GLOBSERVSTATREQ` with the u32 challenge eMule sends
/// (`CUDPSocket` sends the challenge; the answer echoes it at offset 0).
pub fn encode_glob_servstat_req(challenge: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(6);
    out.push(PR_ED2K);
    out.push(OP_GLOBSERVSTATREQ);
    out.extend_from_slice(&challenge.to_le_bytes());
    out
}

/// Encodes `OP_GLOBSEARCHREQ` (`0x98`) — the legacy UDP search eMule sends
/// to servers that do NOT advertise `SRV_UDPFLG_EXT_GETFILES`
/// (`SearchResultsWnd.cpp:305-310`). Same raw search-tree body as REQ2.
pub fn encode_glob_search_req(search_payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + search_payload.len());
    out.push(PR_ED2K);
    out.push(OP_GLOBSEARCHREQ);
    out.extend_from_slice(search_payload);
    out
}

/// Encodes `OP_GLOBSEARCHREQ2` (`0x92`): the UDP variant of
/// `OP_SEARCHREQUEST` — the payload is the raw search-expression bytes of a
/// `SearchQuery` packet (eMule reuses `m_searchpacket->pBuffer` verbatim and
/// only swaps the opcode, `SearchResultsWnd.cpp:284-310`).
pub fn encode_glob_search_req2(search_payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + search_payload.len());
    out.push(PR_ED2K);
    out.push(OP_GLOBSEARCHREQ2);
    out.extend_from_slice(search_payload);
    out
}

/// Encodes `OP_GLOBSEARCHREQ3` (`0x90`) — eMule's extended UDP search used
/// when the server advertises both `SRV_UDPFLG_EXT_GETFILES` and
/// `SRV_UDPFLG_LARGEFILES` (`SearchResultsWnd.cpp:277-291`). Layout:
/// `[u32 tag_count=1][CTag(CT_SERVER_UDPSEARCH_FLAGS, SRVCAP_UDP_NEWTAGS_
/// LARGEFILES)][search_tree]` — the same search-expression bytes as REQ2.
///
/// The flag tag tells the server the client understands new-style tags and
/// 64-bit file sizes in `OP_GLOBSEARCHRES` answers.
pub fn encode_glob_search_req3(search_payload: &[u8]) -> Vec<u8> {
    // eMule writes the flag tag with `WriteNewEd2kTag`: the tag type is
    // OR'd with 0x80 and a numeric tag name is emitted as a single byte
    // (no u16 name length). Wire: [TAGTYPE_UINT8|0x80][0x0E][0x01].
    let mut out = Vec::with_capacity(9 + search_payload.len());
    out.push(PR_ED2K);
    out.push(OP_GLOBSEARCHREQ3);
    out.extend_from_slice(&1u32.to_le_bytes()); // tag count
    out.push(crate::ed2k::tags::TAGTYPE_UINT8 | 0x80);
    out.push(CT_SERVER_UDPSEARCH_FLAGS);
    out.push(SRVCAP_UDP_NEWTAGS_LARGEFILES);
    out.extend_from_slice(search_payload);
    out
}

/// Decodes an `OP_GLOBSEARCHRES` (`0x99`) payload.
///
/// Unlike TCP `OP_SEARCHRESULT` (`[u32 count][entries]`), the UDP answer is
/// a **bare stream of entries** with no count prefix — eMule reads entries
/// until the packet is exhausted (`UDPSocket` → `SSearchFile` loop). Each
/// entry: `[hash:16][client_id:u32][client_port:u16][tag_count:u32][tags]`.
pub fn decode_glob_search_res(
    payload: &[u8],
    limits: &super::limits::Ed2kLimits,
) -> Result<super::server::SearchResult, super::error::Ed2kParseError> {
    use super::error::{Ed2kErrorKind, Ed2kParseError};
    use super::server::{SearchResult, SearchResultFile};

    let mut reader = TagReader::new(payload, 0, PR_ED2K, Some(OP_GLOBSEARCHRES));
    let mut files = Vec::new();
    // One entry needs at least 22 bytes (16+4+2); trailing partial bytes are
    // tolerated like eMule (stops at stream end).
    while reader.pos() + 22 <= payload.len() {
        let file_hash = reader.read_hash("file_hash")?;
        let client_id = reader.read_u32_le("client_id")?;
        let client_port = reader.read_u16_le("client_port")?;
        let tag_count = reader.read_u32_le("tag_count")? as usize;
        if tag_count > limits.max_tags {
            return Err(Ed2kParseError::new(
                PR_ED2K,
                Some(OP_GLOBSEARCHRES),
                reader.pos() - 4,
                "file_tag_count",
                Ed2kErrorKind::TooManyTags {
                    count: tag_count,
                    limit: limits.max_tags,
                },
            ));
        }
        let mut tags = Vec::with_capacity(tag_count.min(32));
        for _ in 0..tag_count {
            let (typed_opt, _) = reader.read_tag(limits)?;
            if let Some(tag) = typed_opt {
                tags.push(tag);
            }
        }
        files.push(SearchResultFile {
            file_hash,
            client_id,
            client_port,
            tags,
        });
        if files.len() >= limits.max_tags * 4 {
            break; // bound total entries like OP_SEARCHRESULT's count cap
        }
    }
    Ok(SearchResult { files })
}

/// Decodes one server UDP datagram (first byte `0xE3`, second opcode).
pub fn decode_server_udp(data: &[u8]) -> Result<ServerUdpMessage, String> {
    if data.len() < 2 {
        return Err("server UDP datagram too short".into());
    }
    // eMule packs UDP replies too (OP_PACKEDPROT 0xD4): the payload after
    // the opcode is a zlib stream; inflate with a bounded cap and decode
    // the decompressed body.
    let decompressed;
    let payload = if data[0] == PR_EMULE_COMPRESSED {
        decompressed = unpack_udp_payload(&data[2..])
            .ok_or_else(|| "invalid packed server UDP datagram".to_string())?;
        &decompressed[..]
    } else if data[0] == PR_ED2K {
        &data[2..]
    } else {
        return Err(format!(
            "invalid server UDP protocol byte 0x{:02X}",
            data[0]
        ));
    };
    match data[1] {
        OP_GLOBFOUNDSOURCES => {
            let mut entries = Vec::new();
            let mut pos = 0usize;
            // Repeated blocks: hash16 + u8 count + count × (u32 ip + u16 port).
            while pos + 17 <= payload.len() {
                let mut file_hash = [0u8; 16];
                file_hash.copy_from_slice(&payload[pos..pos + 16]);
                pos += 16;
                let count = payload[pos] as usize;
                pos += 1;
                if pos + count * 6 > payload.len() {
                    return Err("truncated OP_GLOBFOUNDSOURCES sources".into());
                }
                let mut sources = Vec::with_capacity(count);
                for _ in 0..count {
                    let mut ip_bytes = [0u8; 4];
                    ip_bytes.copy_from_slice(&payload[pos..pos + 4]);
                    let mut port_bytes = [0u8; 2];
                    port_bytes.copy_from_slice(&payload[pos + 4..pos + 6]);
                    let ip = u32::from_le_bytes(ip_bytes);
                    let port = u16::from_le_bytes(port_bytes);
                    pos += 6;
                    sources.push((ip, port));
                }
                entries.push(GlobFoundSourcesEntry { file_hash, sources });
            }
            if pos != payload.len() {
                return Err("truncated OP_GLOBFOUNDSOURCES entry".into());
            }
            Ok(ServerUdpMessage::GlobFoundSources(entries))
        }
        OP_GLOBSERVSTATRES => {
            // eMule layout: [challenge:u32][users][files][maxusers]
            // [softfiles][hardfiles][udp_flags][lowid_users]
            // [udp_obf_port:u16][tcp_obf_port:u16][udp_key:u32]
            // Only the first 12 bytes are mandatory.
            if payload.len() < 12 {
                return Err("truncated OP_GLOBSERVSTATRES".into());
            }
            let read_u32 = |off: usize| {
                let mut b = [0u8; 4];
                b.copy_from_slice(&payload[off..off + 4]);
                u32::from_le_bytes(b)
            };
            let read_u16 = |off: usize| {
                let mut b = [0u8; 2];
                b.copy_from_slice(&payload[off..off + 2]);
                u16::from_le_bytes(b)
            };
            Ok(ServerUdpMessage::GlobServStatRes {
                challenge: read_u32(0),
                users: read_u32(4),
                files: read_u32(8),
                udp_flags: if payload.len() >= 28 { read_u32(24) } else { 0 },
                udp_obfuscation_port: if payload.len() >= 40 { read_u16(32) } else { 0 },
                tcp_obfuscation_port: if payload.len() >= 40 { read_u16(34) } else { 0 },
            })
        }
        OP_GLOBSEARCHRES => Ok(ServerUdpMessage::GlobSearchRes(payload.to_vec())),
        _ => Ok(ServerUdpMessage::Unknown { opcode: data[1] }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn getsources2_wire_format() {
        let hash = [0xCC; 16];
        let pkt = encode_glob_get_sources2(&hash, 123_456);
        assert_eq!(pkt.len(), 22);
        assert_eq!(pkt[0], 0xE3);
        assert_eq!(pkt[1], 0x94);
        assert_eq!(&pkt[2..18], &hash);
        assert_eq!(&pkt[18..22], &123_456u32.to_le_bytes());
    }

    #[test]
    fn foundsources_multi_file() {
        let mut raw = vec![PR_ED2K, OP_GLOBFOUNDSOURCES];
        raw.extend_from_slice(&[1u8; 16]);
        raw.push(2);
        raw.extend_from_slice(&0x7F00_0001u32.to_le_bytes());
        raw.extend_from_slice(&4662u16.to_le_bytes());
        raw.extend_from_slice(&0x7F00_0002u32.to_le_bytes());
        raw.extend_from_slice(&4663u16.to_le_bytes());
        raw.extend_from_slice(&[2u8; 16]);
        raw.push(0); // second file, zero sources

        match decode_server_udp(&raw).unwrap() {
            ServerUdpMessage::GlobFoundSources(entries) => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].sources.len(), 2);
                assert_eq!(entries[0].sources[0], (0x7F00_0001, 4662));
                assert!(entries[1].sources.is_empty());
            }
            m => panic!("unexpected {m:?}"),
        }
    }

    #[test]
    fn servstatres_and_truncation() {
        // eMule layout: [challenge][users][files][maxusers][soft][hard]
        // [udp_flags][lowid][udp_obf:u16][tcp_obf:u16][udp_key:u32]
        let mut raw = vec![PR_ED2K, OP_GLOBSERVSTATRES];
        raw.extend_from_slice(&0xDEADBEEFu32.to_le_bytes()); // challenge
        raw.extend_from_slice(&10_000u32.to_le_bytes()); // users
        raw.extend_from_slice(&500_000u32.to_le_bytes()); // files
        raw.extend_from_slice(&20_000u32.to_le_bytes()); // maxusers
        raw.extend_from_slice(&400_000u32.to_le_bytes()); // softfiles
        raw.extend_from_slice(&600_000u32.to_le_bytes()); // hardfiles
        raw.extend_from_slice(&0x0000_0102u32.to_le_bytes()); // udp_flags
        raw.extend_from_slice(&5_000u32.to_le_bytes()); // lowid users
        raw.extend_from_slice(&6096u16.to_le_bytes()); // udp obf port
        raw.extend_from_slice(&6082u16.to_le_bytes()); // tcp obf port
        raw.extend_from_slice(&12345u32.to_le_bytes()); // udp key
        assert_eq!(
            decode_server_udp(&raw).unwrap(),
            ServerUdpMessage::GlobServStatRes {
                challenge: 0xDEADBEEF,
                users: 10_000,
                files: 500_000,
                udp_flags: 0x0000_0102,
                udp_obfuscation_port: 6096,
                tcp_obfuscation_port: 6082,
            }
        );

        // Short form (only mandatory 12 bytes) — flags default to 0.
        let mut short = vec![PR_ED2K, OP_GLOBSERVSTATRES];
        short.extend_from_slice(&1u32.to_le_bytes());
        short.extend_from_slice(&10_000u32.to_le_bytes());
        short.extend_from_slice(&500_000u32.to_le_bytes());
        let msg = decode_server_udp(&short).unwrap();
        match msg {
            ServerUdpMessage::GlobServStatRes {
                users,
                files,
                udp_flags,
                ..
            } => {
                assert_eq!(users, 10_000);
                assert_eq!(files, 500_000);
                assert_eq!(udp_flags, 0);
            }
            _ => panic!("expected GlobServStatRes"),
        }

        assert!(decode_server_udp(&[PR_ED2K, OP_GLOBFOUNDSOURCES, 0, 0, 0]).is_err());
        assert!(decode_server_udp(&[PR_ED2K, OP_GLOBSERVSTATRES, 0, 0, 0]).is_err());
    }

    #[test]
    fn globsearchreq2_wire_format() {
        let expr = [0x01, 0x02, 0x03, 0xAA];
        let pkt = encode_glob_search_req2(&expr);
        assert_eq!(pkt[0], 0xE3);
        assert_eq!(pkt[1], 0x92);
        assert_eq!(&pkt[2..], &expr);
    }

    #[test]
    fn globsearchreq3_wire_format() {
        let expr = [0x01, 0x02, 0x03, 0xAA];
        let pkt = encode_glob_search_req3(&expr);
        assert_eq!(pkt[0], 0xE3);
        assert_eq!(pkt[1], 0x90);
        // [tag_count:u32=1][NewEd2k tag: TAGTYPE_UINT8|0x80][name:0x0E][val:0x01]
        assert_eq!(&pkt[2..6], &1u32.to_le_bytes());
        assert_eq!(pkt[6], crate::ed2k::tags::TAGTYPE_UINT8 | 0x80);
        assert_eq!(pkt[7], 0x0E);
        assert_eq!(pkt[8], 0x01);
        // Search tree follows verbatim.
        assert_eq!(&pkt[9..], &expr);
    }

    #[test]
    fn globsearchres_decodes_bare_entry_stream() {
        // Real datagram captured from 85.17.116.222:6086 answering
        // GLOBSEARCHREQ1 for "ubuntu": E3 99 + a bare entry stream — NO
        // u32 result-count prefix (unlike TCP OP_SEARCHRESULT).
        let raw = hex_bytes(
            "e39900b00ce17050f61206b62c6bdc7ee8104f94d300ff1204000000820137\
007562756e74752d6d6174652d31362e30342d6465736b746f702d61726d68662d72\
61737062657272792d70692e696d672e787a2e6d6435830256000000831501000000\
833001000000",
        );
        let msg = decode_server_udp(&raw).unwrap();
        let ServerUdpMessage::GlobSearchRes(payload) = msg else {
            panic!("expected GlobSearchRes");
        };
        let sr = decode_glob_search_res(&payload, &crate::ed2k::limits::Ed2kLimits::for_server())
            .expect("glob search res decode");
        assert_eq!(sr.files.len(), 1);
        let f = &sr.files[0];
        assert_eq!(
            f.name().as_deref(),
            Some("ubuntu-mate-16.04-desktop-armhf-raspberry-pi.img.xz.md5")
        );
    }

    fn hex_bytes(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
