// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use md4::{Digest, Md4};
use std::fmt;
use std::io::Read;

use crate::error::FormatError;

/// Exact eD2K part size: 9 728 000 bytes = 9 500 KiB.
pub const ED2K_PART_SIZE: usize = 9_728_000;

/// MD4 digest of an empty stream (0 bytes).
pub const MD4_EMPTY_DIGEST: [u8; 16] = [
    0x31, 0xd6, 0xcf, 0xe0, 0xd1, 0x6a, 0xe9, 0x31, 0xb7, 0x3c, 0x59, 0xd7, 0xe0, 0xc0, 0x89, 0xc0,
];

/// A 16-byte eD2K hash (MD4 digest).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Ed2kHash(pub [u8; 16]);

impl Ed2kHash {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> Result<Self, FormatError> {
        let trimmed = s.trim();
        if trimmed.len() != 32 {
            return Err(FormatError::InvalidEd2kHashString(s.to_string()));
        }
        let mut bytes = [0u8; 16];
        hex::decode_to_slice(trimmed, &mut bytes)
            .map_err(|_| FormatError::InvalidEd2kHashString(s.to_string()))?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for Ed2kHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ed2kHash({})", self.to_hex())
    }
}

impl fmt::Display for Ed2kHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Result of hashing a file or stream with the eD2K algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kHashResult {
    /// Final eD2K root hash.
    pub root_hash: Ed2kHash,
    /// List of individual part hashes (MD4 of each part).
    pub part_hashes: Vec<[u8; 16]>,
    /// Total bytes processed.
    pub total_bytes: u64,
}

/// Streaming eD2K Hasher.
pub struct Ed2kHasher {
    current_part_hasher: Md4,
    current_part_bytes: usize,
    part_hashes: Vec<[u8; 16]>,
    total_bytes: u64,
}

impl Default for Ed2kHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Ed2kHasher {
    pub fn new() -> Self {
        Self {
            current_part_hasher: Md4::new(),
            current_part_bytes: 0,
            part_hashes: Vec::new(),
            total_bytes: 0,
        }
    }

    /// Feeds arbitrary data chunks into the hasher.
    pub fn update(&mut self, mut data: &[u8]) {
        self.total_bytes += data.len() as u64;

        while !data.is_empty() {
            let space_in_part = ED2K_PART_SIZE - self.current_part_bytes;
            if data.len() < space_in_part {
                self.current_part_hasher.update(data);
                self.current_part_bytes += data.len();
                break;
            } else {
                let (head, tail) = data.split_at(space_in_part);
                self.current_part_hasher.update(head);
                let digest: [u8; 16] = self.current_part_hasher.finalize_reset().into();
                self.part_hashes.push(digest);
                self.current_part_bytes = 0;
                data = tail;
            }
        }
    }

    /// Finalizes the calculation and returns the eD2K hash result.
    ///
    /// Empty streams (0 bytes) are rejected by default as invalid eD2K files.
    pub fn finalize(mut self) -> Result<Ed2kHashResult, FormatError> {
        if self.total_bytes == 0 {
            return Err(FormatError::EmptyInput);
        }

        // Case 1: File is strictly smaller than one part
        if self.total_bytes < ED2K_PART_SIZE as u64 {
            let single_digest: [u8; 16] = self.current_part_hasher.finalize().into();
            return Ok(Ed2kHashResult {
                root_hash: Ed2kHash(single_digest),
                part_hashes: vec![single_digest],
                total_bytes: self.total_bytes,
            });
        }

        // Case 2: File is >= ED2K_PART_SIZE
        if self.current_part_bytes > 0 {
            // There was a trailing partial part
            let last_digest: [u8; 16] = self.current_part_hasher.finalize().into();
            self.part_hashes.push(last_digest);
        } else if self.total_bytes.is_multiple_of(ED2K_PART_SIZE as u64) {
            // eMule compatibility: exact multiples append MD4(empty)
            self.part_hashes.push(MD4_EMPTY_DIGEST);
        }

        // Compute outer MD4 over the concatenation of all part digests
        let mut outer_hasher = Md4::new();
        for ph in &self.part_hashes {
            outer_hasher.update(ph);
        }
        let root: [u8; 16] = outer_hasher.finalize().into();

        Ok(Ed2kHashResult {
            root_hash: Ed2kHash(root),
            part_hashes: self.part_hashes,
            total_bytes: self.total_bytes,
        })
    }
}

/// Dérive la racine eD2K d'un fichier depuis son hashset de parts sans
/// relire le disque — mêmes règles que [`Ed2kHasher::finalize`] :
/// fichier < 1 part → la part unique est la racine ; sinon MD4 de la
/// concaténation des hash de parts, avec `MD4_EMPTY_DIGEST` ajouté aux
/// multiples exacts de [`ED2K_PART_SIZE`] quand le hashset ne le porte
/// pas déjà. Retourne `None` quand le hashset ne couvre pas le fichier.
///
/// `part_hashes` peut contenir `total_parts` ou `total_parts + 1`
/// entrées (la part vide eMule) ; toute autre cardinalité est rejetée.
pub fn ed2k_root_from_part_hashes(total_bytes: u64, part_hashes: &[[u8; 16]]) -> Option<Ed2kHash> {
    if total_bytes == 0 || part_hashes.is_empty() {
        return None;
    }

    if total_bytes < ED2K_PART_SIZE as u64 {
        return Some(Ed2kHash(*part_hashes.first()?));
    }

    let real_parts = total_bytes.div_ceil(ED2K_PART_SIZE as u64);
    let exact_multiple = total_bytes.is_multiple_of(ED2K_PART_SIZE as u64);
    let len = part_hashes.len() as u64;

    let mut hashes: Vec<[u8; 16]> = part_hashes.to_vec();
    if exact_multiple {
        match len {
            l if l == real_parts => hashes.push(MD4_EMPTY_DIGEST),
            l if l == real_parts + 1 => {}
            _ => return None,
        }
    } else if len != real_parts {
        return None;
    }

    let mut outer_hasher = Md4::new();
    for ph in &hashes {
        outer_hasher.update(ph);
    }
    let root: [u8; 16] = outer_hasher.finalize().into();
    Some(Ed2kHash(root))
}

/// Convenience helper to hash any reader with a specified buffer size.
pub fn hash_reader<R: Read>(reader: &mut R) -> Result<Ed2kHashResult, FormatError> {
    let mut hasher = Ed2kHasher::new();
    let mut buffer = [0u8; 64 * 1024]; // 64 KiB read buffer
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_bytes(data: &[u8]) -> Ed2kHashResult {
        hash_reader(&mut std::io::Cursor::new(data)).expect("hash_reader")
    }

    fn pattern(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn root_from_part_hashes_sub_part_file() {
        let data = pattern(1234);
        let res = hash_bytes(&data);
        assert_eq!(
            ed2k_root_from_part_hashes(res.total_bytes, &res.part_hashes),
            Some(res.root_hash)
        );
    }

    #[test]
    fn root_from_part_hashes_multi_part_with_tail() {
        let data = pattern(ED2K_PART_SIZE + 123);
        let res = hash_bytes(&data);
        assert_eq!(res.part_hashes.len(), 2);
        assert_eq!(
            ed2k_root_from_part_hashes(res.total_bytes, &res.part_hashes),
            Some(res.root_hash)
        );
    }

    #[test]
    fn root_from_part_hashes_exact_multiple_accepts_both_cardinalities() {
        let data = pattern(ED2K_PART_SIZE * 2);
        let res = hash_bytes(&data);
        // eMule-style hashset: trailing MD4(empty) part included.
        assert_eq!(res.part_hashes.len(), 3);
        assert_eq!(
            ed2k_root_from_part_hashes(res.total_bytes, &res.part_hashes),
            Some(res.root_hash)
        );
        // Hashset without the empty tail part → same derived root.
        let trimmed = &res.part_hashes[..2];
        assert_eq!(
            ed2k_root_from_part_hashes(res.total_bytes, trimmed),
            Some(res.root_hash)
        );
    }

    #[test]
    fn root_from_part_hashes_rejects_incomplete_or_empty_sets() {
        let data = pattern(ED2K_PART_SIZE + 123);
        let res = hash_bytes(&data);
        // Hashset tronqué (1 part au lieu de 2).
        assert_eq!(
            ed2k_root_from_part_hashes(res.total_bytes, &res.part_hashes[..1]),
            None
        );
        assert_eq!(ed2k_root_from_part_hashes(res.total_bytes, &[]), None);
        assert_eq!(ed2k_root_from_part_hashes(0, &res.part_hashes), None);
    }
}
