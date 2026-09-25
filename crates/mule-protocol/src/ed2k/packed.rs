// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! `OP_PACKEDPROT` (`0xD4`) packet compression — eMule `Packet::PackPacket`
//! / `Packet::UnPackPacket` (`Packets.cpp:200-244`).
//!
//! A packed frame keeps the same header layout — `[0xD4][u32 len][opcode]
//! [zlib stream]` — the opcode stays uncompressed so the receiver can
//! route before inflating. Once unpacked, eMule re-tags the packet
//! `OP_EMULEPROT` (Kad frames `0xD5` map back to `OP_KADEMLIAHEADER`), so
//! packed payloads can only carry PR_EMULE opcodes on the peer channel.

use std::io::Read;

/// eMule `Packet::UnPackPacket` default cap for **peer** packets
/// (`uMaxDecompressedSize = 50000u`). Server packets use 250 KB upstream;
/// callers pick the bound via `unpack_payload`'s argument.
pub const PACKED_PEER_MAX_DECOMPRESSED: usize = 50_000;

/// eMule emits `ANSWERSOURCES`/`ANSWERSOURCES2` compressed when the packet
/// exceeds this size (`KnownFile.cpp:1149`, `PartFile.cpp:3708` —
/// `result->size > 354`, where `size` counts opcode + payload).
pub const PACK_SRCINFO_MIN_SIZE: usize = 354;

/// eMule `CKademliaUDPListener::SendPacket` packs any Kad UDP payload
/// (datagram minus the 2-byte `[proto][opcode]` header) larger than this
/// (`KademliaUDPListener.cpp:1875/1891/1904` — `uLenData > 200`).
pub const KAD_UDP_PACK_MIN_SIZE: usize = 200;

/// eMule `ClientUDPSocket` inflate cap for `OP_KADEMLIAPACKEDPROT`
/// (`ClientUDPSocket.cpp:93` — retry loop bounded by `nNewSize < 250000`).
pub const PACKED_KAD_UDP_MAX_DECOMPRESSED: usize = 250_000;

/// Compresses `payload` with zlib at `Z_BEST_COMPRESSION` and returns the
/// compressed stream **only when strictly smaller** than the input —
/// eMule sends the packet uncompressed otherwise (`Packets.cpp:204-211`).
pub fn pack_payload(payload: &[u8]) -> Option<Vec<u8>> {
    let compressed = {
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        use std::io::Write;
        enc.write_all(payload).ok()?;
        enc.finish().ok()?
    };
    (compressed.len() < payload.len()).then_some(compressed)
}

/// Inflates a packed payload with a hard decompressed-size cap, mirroring
/// `Packet::UnPackPacket(uMaxDecompressedSize)`. Returns `None` on
/// truncated, corrupt or oversized data.
pub fn unpack_payload(payload: &[u8], max_decompressed: usize) -> Option<Vec<u8>> {
    let mut decoder = flate2::read::ZlibDecoder::new(payload).take(max_decompressed as u64 + 1);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    // eMule accepts a valid zlib stream even when it inflates to zero
    // bytes (`size = unpackedsize` — the packet is dispatched empty).
    if out.len() > max_decompressed {
        return None;
    }
    Some(out)
}

/// Unpacks a `PR_EMULE_COMPRESSED` (`0xD4`) frame into the packet eMule's
/// dispatchers see: same opcode, inflated payload, protocol re-tagged
/// `PR_EMULE` (`Packets.cpp:240` — packed non-Kad packets always land in
/// the EMULEPROT branch). Returns `None` unchanged-protocol packets or
/// when inflation fails.
pub fn unpack_packet(
    packet: &crate::framing::RawPacket,
    max_decompressed: usize,
) -> Option<crate::framing::RawPacket> {
    if packet.protocol != super::protocol::PR_EMULE_COMPRESSED {
        return None;
    }
    let payload = unpack_payload(&packet.payload, max_decompressed)?;
    Some(crate::framing::RawPacket {
        protocol: super::protocol::PR_EMULE,
        opcode: packet.opcode,
        payload: payload.into(),
    })
}

/// Socket-level normalization mirroring `CEMSocket::ProcessPacket` /
/// `Packet::UnPackPacket`: passes `packet` through unchanged unless its
/// protocol is `PR_EMULE_COMPRESSED`, in which case the payload is inflated
/// and the packet re-tagged `PR_EMULE`. Returns `None` when a packed payload
/// is corrupt or exceeds `max_decompressed` — the caller drops the packet.
pub fn unpack_owned(
    packet: crate::framing::RawPacket,
    max_decompressed: usize,
) -> Option<crate::framing::RawPacket> {
    if packet.protocol != super::protocol::PR_EMULE_COMPRESSED {
        return Some(packet);
    }
    unpack_packet(&packet, max_decompressed)
}

/// Returns the packet to put on the wire: re-framed as `PR_EMULE_COMPRESSED`
/// (`0xD4`) when `packet.frame_len()` (opcode + payload, like eMule's
/// `result->size`) exceeds `pack_min` **and** zlib compression strictly
/// shrinks the payload — the `Packet::PackPacket` gate applied by
/// `CreateSrcInfoPacket` on `OP_ANSWERSOURCES{,2}`. Otherwise returns the
/// packet unchanged.
#[must_use]
pub fn maybe_pack(
    packet: &crate::framing::RawPacket,
    pack_min: usize,
) -> crate::framing::RawPacket {
    if packet.frame_len() > pack_min {
        if let Some(compressed) = pack_payload(&packet.payload) {
            return crate::framing::RawPacket {
                protocol: super::protocol::PR_EMULE_COMPRESSED,
                opcode: packet.opcode,
                payload: compressed.into(),
            };
        }
    }
    packet.clone()
}

/// Packs a Kad UDP datagram `[proto][opcode][payload]` the way
/// `CKademliaUDPListener::SendPacket` does: when the payload exceeds
/// `KAD_UDP_PACK_MIN_SIZE` and zlib strictly shrinks it, the datagram is
/// re-emitted as `[PR_KADEMLIA_PACKED (0xE5)][opcode][zlib stream]`.
/// Anything else — non-Kad protocol, small payload, incompressible data —
/// is returned unchanged (borrowed, no copy).
pub fn pack_kad_datagram(datagram: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    use super::super::kad::opcodes::{PR_KADEMLIA, PR_KADEMLIA_PACKED};
    if datagram.len() > 2
        && datagram[0] == PR_KADEMLIA
        && datagram.len() - 2 > KAD_UDP_PACK_MIN_SIZE
    {
        if let Some(compressed) = pack_payload(&datagram[2..]) {
            let mut out = Vec::with_capacity(2 + compressed.len());
            out.push(PR_KADEMLIA_PACKED);
            out.push(datagram[1]);
            out.extend_from_slice(&compressed);
            return out.into();
        }
    }
    datagram.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack_roundtrip() {
        // Repetitive data compresses well below the threshold.
        let payload = vec![b'A'; 400];
        let packed = pack_payload(&payload).expect("should pack");
        assert!(packed.len() < payload.len());
        assert_eq!(
            unpack_payload(&packed, PACKED_PEER_MAX_DECOMPRESSED).unwrap(),
            payload
        );
    }

    #[test]
    fn pack_rejects_incompressible_data() {
        // High-entropy input stays uncompressed, like eMule.
        let mut payload = Vec::with_capacity(300);
        let mut s: u32 = 0x1234_5678;
        for _ in 0..300 {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            payload.push((s >> 24) as u8);
        }
        assert!(pack_payload(&payload).is_none());
    }

    #[test]
    fn unpack_enforces_cap() {
        // ~300 KB of zeros → a few hundred compressed bytes (bomb shape).
        let big = vec![0u8; 300_000];
        let packed = pack_payload(&big).expect("zeros compress");
        assert!(unpack_payload(&packed, PACKED_PEER_MAX_DECOMPRESSED).is_none());
        assert!(unpack_payload(&packed, 250_000).is_none());
        // At the inflated size the same data must pass.
        assert_eq!(unpack_payload(&packed, 300_000).unwrap().len(), 300_000);
    }

    #[test]
    fn unpack_rejects_garbage_and_truncation() {
        assert!(unpack_payload(b"not zlib at all", PACKED_PEER_MAX_DECOMPRESSED).is_none());
        let mut packed = pack_payload(&vec![b'x'; 400]).unwrap();
        packed.truncate(packed.len() - 2);
        assert!(unpack_payload(&packed, PACKED_PEER_MAX_DECOMPRESSED).is_none());
        // A 1-byte payload can't shrink — never emitted packed.
        assert!(pack_payload(&[0xAA]).is_none());
    }

    #[test]
    fn unpack_owned_retags_emuleprot() {
        use super::super::protocol::{PR_EMULE, PR_EMULE_COMPRESSED};
        let body = b"source exchange answer payload".repeat(20);
        let pkt = crate::framing::RawPacket {
            protocol: PR_EMULE_COMPRESSED,
            opcode: 0x99,
            payload: pack_payload(&body).unwrap().into(),
        };
        let unpacked = unpack_owned(pkt, PACKED_PEER_MAX_DECOMPRESSED).unwrap();
        assert_eq!(unpacked.protocol, PR_EMULE);
        assert_eq!(unpacked.opcode, 0x99);
        assert_eq!(&*unpacked.payload, &*body);
        // Non-packed packets pass through untouched.
        let plain = crate::framing::RawPacket {
            protocol: PR_EMULE,
            opcode: 0x99,
            payload: body.clone().into(),
        };
        let same = unpack_owned(plain, PACKED_PEER_MAX_DECOMPRESSED).unwrap();
        assert_eq!(same.protocol, PR_EMULE);
        assert_eq!(&*same.payload, &*body);
        // Corrupt packed packet is dropped, not dispatched.
        let bad = crate::framing::RawPacket {
            protocol: PR_EMULE_COMPRESSED,
            opcode: 0x99,
            payload: b"junk".as_slice().into(),
        };
        assert!(unpack_owned(bad, PACKED_PEER_MAX_DECOMPRESSED).is_none());
    }

    #[test]
    fn maybe_pack_gates_on_size_and_gain() {
        use super::super::protocol::{PR_EMULE, PR_EMULE_COMPRESSED};
        let mk = |payload: Vec<u8>| crate::framing::RawPacket {
            protocol: PR_EMULE,
            opcode: 0x98,
            payload: payload.into(),
        };
        // Below the 354 gate: never packed, even if compressible.
        let small = mk(vec![b'z'; 200]);
        assert_eq!(maybe_pack(&small, PACK_SRCINFO_MIN_SIZE).protocol, PR_EMULE);
        // Big + compressible: packed and inflates back identically.
        let big = mk(vec![b'z'; 2_000]);
        let packed = maybe_pack(&big, PACK_SRCINFO_MIN_SIZE);
        assert_eq!(packed.protocol, PR_EMULE_COMPRESSED);
        assert!(packed.payload.len() < 2_000);
        let restored = unpack_owned(packed, PACKED_PEER_MAX_DECOMPRESSED).unwrap();
        assert_eq!(restored.protocol, PR_EMULE);
        assert_eq!(&*restored.payload, &vec![b'z'; 2_000][..]);
        // Big but incompressible: stays uncompressed.
        let mut noise = Vec::with_capacity(2_000);
        let mut s: u32 = 0xDEAD_BEEF;
        for _ in 0..2_000 {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            noise.push((s >> 24) as u8);
        }
        let n = mk(noise);
        assert_eq!(maybe_pack(&n, PACK_SRCINFO_MIN_SIZE).protocol, PR_EMULE);
    }

    #[test]
    fn pack_kad_datagram_retags_packed_prot() {
        use super::super::protocol::PR_EMULE;
        use crate::kad::opcodes::{PR_KADEMLIA, PR_KADEMLIA_PACKED};
        // Datagramme Kad compressible > 200 octets de payload → 0xE5.
        let mut dgram = vec![PR_KADEMLIA, 0x3B];
        dgram.extend_from_slice(&vec![0xAA; 500]);
        let packed = pack_kad_datagram(&dgram);
        assert_eq!(packed[0], PR_KADEMLIA_PACKED);
        assert_eq!(packed[1], 0x3B);
        assert!(packed.len() < dgram.len());
        // …et il s'inflate de nouveau à l'identique.
        assert_eq!(
            unpack_payload(&packed[2..], PACKED_KAD_UDP_MAX_DECOMPRESSED)
                .unwrap()
                .len(),
            500
        );
        // Petit datagramme → inchangé (emprunté, pas de copie).
        let small = vec![PR_KADEMLIA, 0x10, 0x00];
        assert!(matches!(
            pack_kad_datagram(&small),
            std::borrow::Cow::Borrowed(_)
        ));
        // Proto non-Kad jamais packé.
        let mut not_kad = vec![PR_EMULE, 0x99];
        not_kad.extend_from_slice(&vec![0xAA; 500]);
        assert_eq!(pack_kad_datagram(&not_kad)[0], PR_EMULE);
    }
}
