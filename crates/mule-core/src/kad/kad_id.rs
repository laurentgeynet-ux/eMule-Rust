use std::fmt;
use serde::{Deserialize, Serialize};

/// A 128-bit Kad Node ID or Key ID.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(32);
        for b in &self.0 {
            use std::fmt::Write;
            let _ = write!(s, "{:02x}", b);
        }
        s
    }

    /// Computes XOR distance between two Kad IDs.
    pub fn xor_distance(&self, other: &KadId) -> KadId {
        let mut res = [0u8; 16];
        for (i, byte) in res.iter_mut().enumerate() {
            *byte = self.0[i] ^ other.0[i];
        }
        KadId(res)
    }

    /// Number of leading zero bits in this ID (viewed as a 128-bit big-endian integer).
    pub fn leading_zeros(&self) -> u32 {
        let mut total = 0;
        for &b in &self.0 {
            let lz = b.leading_zeros();
            total += lz;
            if lz < 8 {
                break;
            }
        }
        total
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
