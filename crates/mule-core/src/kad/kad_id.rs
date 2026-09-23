// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use serde::{Deserialize, Serialize};
use std::fmt;

/// A 128-bit Kad Node ID or Key ID.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KadId(pub [u8; 16]);

impl KadId {
    pub const ZERO: Self = Self([0u8; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn random() -> Self {
        use rand::RngCore;
        let mut bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut bytes);
        Self(bytes)
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        if s.len() != 32 {
            return None;
        }
        let mut bytes = [0u8; 16];
        for i in 0..16 {
            bytes[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Self(bytes))
    }

    /// Converts a 16-byte eD2K big-endian hash (eD2K file hash, user hash, or keyword MD4 digest)
    /// into eMule Kad wire format (where each 4-byte DWORD is byte-reversed,
    /// matching eMule's CUInt128::SetValueBE / GetData).
    pub fn from_hash_be(hash: &[u8; 16]) -> Self {
        let mut res = [0u8; 16];
        for i in 0..4 {
            res[i * 4] = hash[i * 4 + 3];
            res[i * 4 + 1] = hash[i * 4 + 2];
            res[i * 4 + 2] = hash[i * 4 + 1];
            res[i * 4 + 3] = hash[i * 4];
        }
        KadId(res)
    }

    /// Converts an eD2K hash hex string into eMule Kad wire format.
    pub fn from_hash_hex(hex_str: &str) -> Option<Self> {
        let raw = KadId::from_hex(hex_str)?;
        Some(Self::from_hash_be(&raw.0))
    }

    /// Converts a Kad wire ID back to an eD2K big-endian hash
    /// (matching eMule's CUInt128::ToByteArray).
    pub fn to_hash_be(&self) -> [u8; 16] {
        let mut res = [0u8; 16];
        for i in 0..4 {
            res[i * 4] = self.0[i * 4 + 3];
            res[i * 4 + 1] = self.0[i * 4 + 2];
            res[i * 4 + 2] = self.0[i * 4 + 1];
            res[i * 4 + 3] = self.0[i * 4];
        }
        res
    }

    /// Converts a Kad wire ID back to an eD2K hash hex string.
    pub fn to_hash_hex(&self) -> String {
        let hash = self.to_hash_be();
        let mut s = String::with_capacity(32);
        for b in &hash {
            use std::fmt::Write;
            let _ = write!(s, "{:02x}", b);
        }
        s
    }

    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(32);
        for b in &self.0 {
            use std::fmt::Write;
            let _ = write!(s, "{:02x}", b);
        }
        s
    }

    /// Returns the 32-bit DWORD chunk at `index` (0..4), where chunk 0 is the most significant DWORD.
    /// Matches eMule `CUInt128::Get32BitChunk(index)`.
    #[inline]
    pub fn dword(&self, index: usize) -> u32 {
        let start = index * 4;
        u32::from_le_bytes([
            self.0[start],
            self.0[start + 1],
            self.0[start + 2],
            self.0[start + 3],
        ])
    }

    /// Computes XOR distance between two Kad IDs.
    pub fn xor_distance(&self, other: &KadId) -> KadId {
        let mut res = [0u8; 16];
        for (i, byte) in res.iter_mut().enumerate() {
            *byte = self.0[i] ^ other.0[i];
        }
        KadId(res)
    }

    /// Number of leading zero bits in this ID (viewed as a 128-bit integer,
    /// matching eMule's bit 0 at `(m_uData[0] >> 31) & 1` down to bit 127 at `(m_uData[3] >> 0) & 1`).
    pub fn leading_zeros(&self) -> u32 {
        let mut total = 0;
        for i in 0..4 {
            let dw = self.dword(i);
            let lz = dw.leading_zeros();
            total += lz;
            if lz < 32 {
                break;
            }
        }
        total
    }

    /// Checks whether this contact's distance to `target` is within eMule's SEARCHTOLERANCE (0x01000000).
    /// In eMule C++, nodes drop search/publish requests if `distance.Get32BitChunk(0) > SEARCHTOLERANCE`.
    #[inline]
    pub fn is_within_search_tolerance(&self, target: &KadId) -> bool {
        const SEARCH_TOLERANCE: u32 = 0x01000000;
        let dist = self.xor_distance(target);
        dist.dword(0) <= SEARCH_TOLERANCE
    }

    /// Calculates the bucket index [0..127] for another ID relative to this local ID.
    /// Bucket 0: distance leading zeros == 127 (differ only in the last bit).
    /// Bucket 127: distance leading zeros == 0 (differ in the most significant bit).
    pub fn bucket_index_for(&self, other: &KadId) -> Option<usize> {
        let dist = self.xor_distance(other);
        let lz = dist.leading_zeros();
        if lz >= 128 {
            None // Same ID (distance is 0)
        } else {
            Some(127 - lz as usize)
        }
    }
}

impl Ord for KadId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        for i in 0..4 {
            let a = self.dword(i);
            let b = other.dword(i);
            match a.cmp(&b) {
                std::cmp::Ordering::Equal => continue,
                ord => return ord,
            }
        }
        std::cmp::Ordering::Equal
    }
}

impl PartialOrd for KadId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Debug for KadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KadId({})", self.to_hex())
    }
}

impl fmt::Display for KadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kad_id_dword_endian_swap_exact_vector() {
        // eD2K raw MD4 big-endian hash
        let ed2k_hash = [
            0x00, 0x01, 0x02, 0x03, // DWORD 0
            0x04, 0x05, 0x06, 0x07, // DWORD 1
            0x08, 0x09, 0x0A, 0x0B, // DWORD 2
            0x0C, 0x0D, 0x0E, 0x0F, // DWORD 3
        ];

        // In eMule CUInt128::SetValueBE on little-endian x86/x64:
        // each 32-bit DWORD is stored with bytes reversed:
        let expected_wire = [
            0x03, 0x02, 0x01, 0x00, 0x07, 0x06, 0x05, 0x04, 0x0B, 0x0A, 0x09, 0x08, 0x0F, 0x0E,
            0x0D, 0x0C,
        ];

        let kad_id = KadId::from_hash_be(&ed2k_hash);
        assert_eq!(kad_id.0, expected_wire);

        // Converting back to big-endian hash via to_hash_be()
        let roundtrip_hash = kad_id.to_hash_be();
        assert_eq!(roundtrip_hash, ed2k_hash);
    }

    #[test]
    fn test_kad_id_hex_conversions() {
        let hash_hex = "d37c98517e79ddc1688e27d1fe849be5";
        let kad_id = KadId::from_hash_hex(hash_hex).expect("valid hex");
        assert_eq!(kad_id.to_hash_hex(), hash_hex);
    }

    #[test]
    fn test_kad_id_dword_and_comparison() {
        // ID with DWORD 0 = 0x00000001 (small distance)
        let mut b1 = [0u8; 16];
        b1[0] = 0x01; // little-endian LSB of DWORD 0 -> 0x00000001
        let id_small = KadId(b1);
        assert_eq!(id_small.dword(0), 1);

        // ID with DWORD 0 = 0x01000000 (SEARCHTOLERANCE boundary)
        let mut b2 = [0u8; 16];
        b2[3] = 0x01; // byte 3 is MSB of DWORD 0 -> 0x01000000
        let id_boundary = KadId(b2);
        assert_eq!(id_boundary.dword(0), 0x01000000);

        // ID with DWORD 0 = 0x02000000 (exceeds SEARCHTOLERANCE)
        let mut b3 = [0u8; 16];
        b3[3] = 0x02;
        let id_large = KadId(b3);
        assert_eq!(id_large.dword(0), 0x02000000);

        // Ord must compare DWORD 0 first: id_small < id_boundary < id_large
        assert!(id_small < id_boundary);
        assert!(id_boundary < id_large);

        // Tolerance check
        let target = KadId::ZERO;
        assert!(id_small.is_within_search_tolerance(&target));
        assert!(id_boundary.is_within_search_tolerance(&target));
        assert!(!id_large.is_within_search_tolerance(&target));
    }
}
