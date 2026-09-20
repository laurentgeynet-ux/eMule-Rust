//! Server UDP wire messages (`UDPSocket.cpp`, eMule 0.72a).
//!
//! Global server queries are sent over a dedicated UDP socket to each
//! server's UDP port (its TCP port + 4 for Lugdunum servers), under the
//! `PR_ED2K` (`0xE3`) protocol byte:
//! - `OP_GLOBGETSOURCES2` (`0x94`) — sources for a file hash.
//! - `OP_GLOBFOUNDSOURCES` (`0x9B`) — multi-file source answers.
//! - `OP_GLOBSERVSTATREQ`/`RES` (`0x96`/`0x97`) — server statistics.

use super::opcodes::{
    OP_GLOBFOUNDSOURCES, OP_GLOBGETSOURCES2, OP_GLOBSEARCHREQ2, OP_GLOBSEARCHRES,
    OP_GLOBSERVSTATREQ, OP_GLOBSERVSTATRES,
};
use super::protocol::{PR_ED2K, PR_EMULE_COMPRESSED};

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
    /// `OP_GLOBSERVSTATRES` — server user/file counters.
    GlobServStatRes {
        /// Connected users reported by the server.
        users: u32,
        /// Indexed files reported by the server.
        files: u32,
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

/// Encodes `OP_GLOBSERVSTATREQ` (empty payload).
pub fn encode_glob_servstat_req() -> Vec<u8> {
    vec![PR_ED2K, OP_GLOBSERVSTATREQ]
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
                    let ip = u32::from_le_bytes(payload[pos..pos + 4].try_into().unwrap());
                    let port = u16::from_le_bytes(payload[pos + 4..pos + 6].try_into().unwrap());
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
            if payload.len() < 8 {
                return Err("truncated OP_GLOBSERVSTATRES".into());
            }
            Ok(ServerUdpMessage::GlobServStatRes {
                users: u32::from_le_bytes(payload[0..4].try_into().unwrap()),
                files: u32::from_le_bytes(payload[4..8].try_into().unwrap()),
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
        let mut raw = vec![PR_ED2K, OP_GLOBSERVSTATRES];
        raw.extend_from_slice(&10_000u32.to_le_bytes());
        raw.extend_from_slice(&500_000u32.to_le_bytes());
        assert_eq!(
            decode_server_udp(&raw).unwrap(),
            ServerUdpMessage::GlobServStatRes {
                users: 10_000,
                files: 500_000
            }
        );
        assert!(decode_server_udp(&[PR_ED2K, OP_GLOBFOUNDSOURCES, 0, 0, 0]).is_err());
    }

    #[test]
    fn globsearchreq2_wire_format() {
        let expr = [0x01, 0x02, 0x03, 0xAA];
        let pkt = encode_glob_search_req2(&expr);
        assert_eq!(pkt[0], 0xE3);
        assert_eq!(pkt[1], 0x92);
        assert_eq!(&pkt[2..], &expr);
    }
}
