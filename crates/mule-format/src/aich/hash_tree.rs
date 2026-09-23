// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use sha1::{Digest, Sha1};
use std::fmt;

use crate::ed2k::hash::ED2K_PART_SIZE;
use crate::error::FormatError;

/// Exact AICH block size: 184 320 bytes = 180 KiB.
pub const AICH_BLOCK_SIZE: usize = 184_320;

/// Number of AICH blocks in a full eD2K part: 53.
/// (52 * 184 320 + 143 360 = 9 728 000).
pub const BLOCKS_PER_PART: usize = 53;

/// Size of the last AICH block in a full eD2K part.
pub const LAST_BLOCK_SIZE_FULL_PART: usize = 143_360;

/// A 20-byte SHA-1 hash used by AICH.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AichHash(pub [u8; 20]);

impl AichHash {
    pub const fn from_bytes(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn to_base32(&self) -> String {
        data_encoding::BASE32_NOPAD.encode(&self.0)
    }

    pub fn from_base32(s: &str) -> Result<Self, FormatError> {
        let trimmed = s.trim();
        let mut out = [0u8; 20];
        data_encoding::BASE32_NOPAD
            .decode_mut(trimmed.as_bytes(), &mut out)
            .map_err(|_| FormatError::InvalidAichBase32(s.to_string()))?;
        Ok(Self(out))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for AichHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AichHash({})", self.to_base32())
    }
}

impl fmt::Display for AichHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base32())
    }
}

/// A node in the AICH Merkle Hash Tree.
#[derive(Debug, Clone)]
pub struct AichNode {
    pub data_size: u64,
    pub base_size: u64,
    pub is_left_branch: bool,
    pub hash: Option<AichHash>,
    pub left: Option<Box<AichNode>>,
    pub right: Option<Box<AichNode>>,
}

impl AichNode {
    /// Builds the tree structure for a given data size.
    pub fn new_tree(data_size: u64) -> Result<Self, FormatError> {
        if data_size == 0 {
            return Err(FormatError::EmptyInput);
        }
        let base_size = if data_size <= ED2K_PART_SIZE as u64 {
            AICH_BLOCK_SIZE as u64
        } else {
            ED2K_PART_SIZE as u64
        };
        Ok(Self::build_node(data_size, true, base_size))
    }

    fn build_node(data_size: u64, is_left_branch: bool, base_size: u64) -> Self {
        if data_size <= base_size {
            // Leaf node
            Self {
                data_size,
                base_size,
                is_left_branch,
                hash: None,
                left: None,
                right: None,
            }
        } else {
            let n_blocks = data_size.div_ceil(base_size);
            let left_blocks = if is_left_branch {
                n_blocks.div_ceil(2)
            } else {
                n_blocks / 2
            };
            let left_size = left_blocks * base_size;
            let right_size = data_size - left_size;

            // eMule: each child's base size depends on its own coverage
            // (part granularity above one part, block granularity below).
            let child_base = |size: u64| {
                if size <= ED2K_PART_SIZE as u64 {
                    AICH_BLOCK_SIZE as u64
                } else {
                    ED2K_PART_SIZE as u64
                }
            };
            let left = Box::new(Self::build_node(left_size, true, child_base(left_size)));
            let right = Box::new(Self::build_node(right_size, false, child_base(right_size)));

            Self {
                data_size,
                base_size,
                is_left_branch,
                hash: None,
                left: Some(left),
                right: Some(right),
            }
        }
    }

    /// Iterates the AICH leaf blocks of a `file_size`-byte file.
    /// Blocks are aligned to eD2K part boundaries (eMule semantics): each
    /// part holds 52 full 180 KiB blocks plus a 143,360-byte final block.
    pub fn for_each_block_range(file_size: u64, mut f: impl FnMut(u64, u64)) {
        let mut pos = 0u64;
        while pos < file_size {
            let part_end = ((pos / ED2K_PART_SIZE as u64) + 1) * ED2K_PART_SIZE as u64;
            let end = (pos + AICH_BLOCK_SIZE as u64).min(part_end).min(file_size);
            f(pos, end - pos);
            pos = end;
        }
    }

    /// Sets the SHA-1 hash of a leaf block at `start_pos`.
    pub fn set_leaf_hash(&mut self, start_pos: u64, block_size: u64, hash: AichHash) -> bool {
        if self.left.is_none() && self.right.is_none() {
            if self.data_size == block_size && start_pos == 0 {
                self.hash = Some(hash);
                return true;
            }
            return false;
        }

        let left_size = self.left.as_ref().unwrap().data_size;
        if start_pos < left_size {
            if start_pos + block_size > left_size {
                return false;
            }
            self.left
                .as_mut()
                .unwrap()
                .set_leaf_hash(start_pos, block_size, hash)
        } else {
            let relative_pos = start_pos - left_size;
            if relative_pos + block_size > self.right.as_ref().unwrap().data_size {
                return false;
            }
            self.right
                .as_mut()
                .unwrap()
                .set_leaf_hash(relative_pos, block_size, hash)
        }
    }

    /// Recursively computes parent hashes up to the root.
    pub fn recalculate_hash(&mut self) -> Result<AichHash, FormatError> {
        if let (Some(left), Some(right)) = (&mut self.left, &mut self.right) {
            let left_h = left.recalculate_hash()?;
            let right_h = right.recalculate_hash()?;

            let mut hasher = Sha1::new();
            hasher.update(left_h.as_bytes());
            hasher.update(right_h.as_bytes());
            let digest: [u8; 20] = hasher.finalize().into();
            let parent_hash = AichHash(digest);
            self.hash = Some(parent_hash);
            Ok(parent_hash)
        } else if let Some(h) = self.hash {
            Ok(h)
        } else {
            Err(FormatError::AichVerificationFailed(
                "Incomplete hash tree".to_string(),
            ))
        }
    }

    /// Returns the root hash if already calculated.
    pub fn root_hash(&self) -> Option<AichHash> {
        self.hash
    }
}

/// Convenience function to build a full AICH tree from a data slice.
pub fn calculate_aich_root(data: &[u8]) -> Result<AichHash, FormatError> {
    if data.is_empty() {
        return Err(FormatError::EmptyInput);
    }
    let mut tree = AichNode::new_tree(data.len() as u64)?;
    let mut ok = true;
    AichNode::for_each_block_range(data.len() as u64, |start, len| {
        let block_bytes = &data[start as usize..(start + len) as usize];
        let mut hasher = Sha1::new();
        hasher.update(block_bytes);
        let digest: [u8; 20] = hasher.finalize().into();
        if !tree.set_leaf_hash(start, len, AichHash(digest)) {
            ok = false;
        }
    });
    if !ok {
        return Err(FormatError::AichVerificationFailed(
            "failed to set leaf hash".to_string(),
        ));
    }
    tree.recalculate_hash()
}
