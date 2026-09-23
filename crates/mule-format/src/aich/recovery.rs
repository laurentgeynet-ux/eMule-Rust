// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! AICH part recovery data (eMule `CAICHRecoveryHashSet::CreatePartRecoveryData`
//! / `ReadRecoveryData` wire semantics).
//!
//! Recovery data is a list of `(hash_ident, SHA-1 hash)` pairs where the
//! identifier encodes the node path inside the AICH hash tree: the ident is a
//! `u32`/`u16` whose most significant set bit is a leading marker (the root's
//! `is_left_branch = true` contribution); each subsequent bit selects the child
//! at that level (`1` = left branch node, `0` = right branch node).
//!
//! On the wire the list is serialized as:
//!   `u16 count16` + `count16 * (u16 ident + hash[20])` +
//!   `u16 count32` + `count32 * (u32 ident + hash[20])`
//! Only one of the two lists is populated (16-bit for regular files, 32-bit
//! identifiers for large files > 4 GiB).

use sha1::{Digest, Sha1};
use std::io::Read;

use crate::ed2k::hash::ED2K_PART_SIZE;
use crate::error::FormatError;

use super::hash_tree::{AichHash, AichNode, AICH_BLOCK_SIZE};

/// One recovery-data entry: tree path identifier + SHA-1 hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AichRecoveryEntry {
    /// Hash identifier (tree path encoding, MSB marker included).
    pub ident: u32,
    /// SHA-1 hash of the addressed tree node.
    pub hash: AichHash,
}

/// Builds the complete AICH hash tree over `data` (all node hashes computed).
pub fn build_aich_tree(data: &[u8]) -> Result<AichNode, FormatError> {
    if data.is_empty() {
        return Err(FormatError::EmptyInput);
    }
    let mut tree = AichNode::new_tree(data.len() as u64)?;
    let mut ok = true;
    AichNode::for_each_block_range(data.len() as u64, |start, len| {
        let digest: [u8; 20] = Sha1::digest(&data[start as usize..(start + len) as usize]).into();
        if !tree.set_leaf_hash(start, len, AichHash(digest)) {
            ok = false;
        }
    });
    if !ok {
        return Err(FormatError::AichVerificationFailed(
            "failed to set leaf hash".to_string(),
        ));
    }
    tree.recalculate_hash()?;
    Ok(tree)
}

/// Builds the complete AICH hash tree by streaming `file_size` bytes from `r`.
/// Memory usage stays bounded to the tree structure, not the file contents.
pub fn build_aich_tree_from_reader<R: Read>(
    r: &mut R,
    file_size: u64,
) -> Result<AichNode, FormatError> {
    if file_size == 0 {
        return Err(FormatError::EmptyInput);
    }
    let mut tree = AichNode::new_tree(file_size)?;
    let mut buf = vec![0u8; AICH_BLOCK_SIZE];
    let mut ok = true;
    let mut err = None;
    AichNode::for_each_block_range(file_size, |start, len| {
        let chunk_len = len as usize;
        if let Err(e) = r.read_exact(&mut buf[..chunk_len]) {
            err = Some(e);
            ok = false;
            return;
        }
        let digest: [u8; 20] = Sha1::digest(&buf[..chunk_len]).into();
        if !tree.set_leaf_hash(start, len, AichHash(digest)) {
            ok = false;
        }
    });
    if let Some(e) = err {
        return Err(e.into());
    }
    if !ok {
        return Err(FormatError::AichVerificationFailed(
            "failed to set leaf hash".to_string(),
        ));
    }
    tree.recalculate_hash()?;
    Ok(tree)
}

/// Depth of the part subtree below the root (eMule `FindHash(..., &nLevel)`).
/// The part subtree is the node covering `[part_start, part_start + part_size)`.
fn find_part_node(
    root: &AichNode,
    part_start: u64,
    part_size: u64,
) -> Option<(&AichNode, u8)> {
    fn walk(
        node: &AichNode,
        start: u64,
        size: u64,
        level: u8,
    ) -> Option<(&AichNode, u8)> {
        if start == 0 && size == node.data_size {
            return Some((node, level));
        }
        if node.data_size <= node.base_size {
            return None;
        }
        let n_blocks = node.data_size.div_ceil(node.base_size);
        let left_blocks = if node.is_left_branch {
            n_blocks.div_ceil(2)
        } else {
            n_blocks / 2
        };
        let left_size = left_blocks * node.base_size;
        if start < left_size {
            if start + size > left_size {
                return None;
            }
            walk(node.left.as_ref()?, start, size, level + 1)
        } else {
            walk(node.right.as_ref()?, start - left_size, size, level + 1)
        }
    }
    walk(root, part_start, part_size, 0)
}

/// Serializes the sibling hashes along the path to the part subtree plus all
/// lowest-level (block) hashes of the part, in eMule wire order.
///
/// Equivalent to `CAICHRecoveryHashSet::CreatePartRecoveryData`:
/// `nHashesToWrite = (nLevel - 1) + ceil(partSize / EMBLOCKSIZE)`.
pub fn create_part_recovery_data(
    root: &AichNode,
    file_size: u64,
    part_index: u32,
) -> Result<Vec<AichRecoveryEntry>, FormatError> {
    if file_size == 0 || file_size <= AICH_BLOCK_SIZE as u64 {
        // eMule: no AICH recovery for files smaller than one block.
        return Err(FormatError::AichVerificationFailed(
            "file too small for AICH recovery data".to_string(),
        ));
    }
    if root.data_size != file_size {
        return Err(FormatError::AichVerificationFailed(
            "tree root does not cover the requested file size".to_string(),
        ));
    }
    let part_start = part_index as u64 * ED2K_PART_SIZE as u64;
    if part_start >= file_size {
        return Err(FormatError::AichVerificationFailed(
            "part index out of range".to_string(),
        ));
    }
    let part_size = (ED2K_PART_SIZE as u64).min(file_size - part_start);

    let mut out = Vec::new();
    if !create_recovery_inner(root, part_start, part_size, 0, &mut out) {
        return Err(FormatError::AichVerificationFailed(
            "failed to create recovery data".to_string(),
        ));
    }

    let (_, level) = find_part_node(root, part_start, part_size)
        .ok_or_else(|| FormatError::AichVerificationFailed("part node not found".to_string()))?;
    // eMule: nHashesToWrite = (nLevel - 1) + blocks, where nLevel = depth + 1.
    let expected = level as usize + part_size.div_ceil(AICH_BLOCK_SIZE as u64) as usize;
    if out.len() != expected {
        return Err(FormatError::AichVerificationFailed(format!(
            "recovery data has {} entries, expected {expected}",
            out.len()
        )));
    }
    Ok(out)
}

/// `CAICHHashTree::CreatePartRecoveryData`: writes sibling hashes along the
/// descent, then the part's lowest-level hashes left to right.
fn create_recovery_inner(
    node: &AichNode,
    part_start: u64,
    part_size: u64,
    ident: u32,
    out: &mut Vec<AichRecoveryEntry>,
) -> bool {
    if part_start == 0 && part_size == node.data_size {
        return write_lowest_level_hashes(node, ident, out);
    }
    if node.data_size <= node.base_size {
        return false;
    }
    let ident = (ident << 1) | (node.is_left_branch as u32);
    let n_blocks = node.data_size.div_ceil(node.base_size);
    let left_blocks = if node.is_left_branch {
        n_blocks.div_ceil(2)
    } else {
        n_blocks / 2
    };
    let left_size = left_blocks * node.base_size;
    let (Some(left), Some(right)) = (&node.left, &node.right) else {
        return false;
    };
    if part_start < left_size {
        if part_start + part_size > left_size {
            return false;
        }
        if !write_ident_hash(right, ident, out) {
            return false;
        }
        return create_recovery_inner(left, part_start, part_size, ident, out);
    }
    if !write_ident_hash(left, ident, out) {
        return false;
    }
    create_recovery_inner(right, part_start - left_size, part_size, ident, out)
}

/// `CAICHHashTree::WriteHash`: `(ident << 1) | is_left_branch` then hash.
fn write_ident_hash(node: &AichNode, ident: u32, out: &mut Vec<AichRecoveryEntry>) -> bool {
    let ident = (ident << 1) | (node.is_left_branch as u32);
    match node.hash {
        Some(hash) => {
            out.push(AichRecoveryEntry { ident, hash });
            true
        }
        None => false,
    }
}

/// `CAICHHashTree::WriteLowestLevelHashes` with identifiers.
fn write_lowest_level_hashes(
    node: &AichNode,
    ident: u32,
    out: &mut Vec<AichRecoveryEntry>,
) -> bool {
    let ident = (ident << 1) | (node.is_left_branch as u32);
    if node.left.is_none() && node.right.is_none() {
        return match (node.data_size <= node.base_size, node.hash) {
            (true, Some(hash)) => {
                out.push(AichRecoveryEntry { ident, hash });
                true
            }
            _ => false,
        };
    }
    let (Some(left), Some(right)) = (&node.left, &node.right) else {
        return false;
    };
    write_lowest_level_hashes(left, ident, out) && write_lowest_level_hashes(right, ident, out)
}

/// Sets a hash in the skeleton tree addressed by `ident`
/// (`CAICHHashTree::SetHash`). Returns false if the ident does not address a
/// valid node or attempts to overwrite the master hash (ident == 1).
pub fn set_hash_by_ident(
    root: &mut AichNode,
    ident: u32,
    hash: AichHash,
    allow_overwrite: bool,
) -> bool {
    if ident == 0 || ident == 1 {
        return false;
    }
    let depth = 31 - ident.leading_zeros();
    let mut node = root;
    for i in (0..depth).rev() {
        if node.data_size <= node.base_size {
            return false;
        }
        let go_left = (ident >> i) & 1 == 1;
        node = if go_left {
            match node.left.as_mut() {
                Some(l) => l,
                None => return false,
            }
        } else {
            match node.right.as_mut() {
                Some(r) => r,
                None => return false,
            }
        };
    }
    if node.hash.is_some() && !allow_overwrite {
        // Hash already present: eMule skips the write but accepts the entry.
        return node.hash == Some(hash);
    }
    node.hash = Some(hash);
    true
}

/// Recomputes parent hashes bottom-up for nodes whose subtree is fully hashed
/// (`CAICHHashTree::VerifyHashTree` semantics). Returns the resulting root hash
/// if determinable.
fn recalculate_partial(node: &mut AichNode) -> Option<AichHash> {
    if let Some(h) = node.hash {
        return Some(h);
    }
    let (left, right) = (&mut node.left, &mut node.right);
    match (left, right) {
        (Some(l), Some(r)) => {
            let lh = recalculate_partial(l)?;
            let rh = recalculate_partial(r)?;
            let mut hasher = Sha1::new();
            hasher.update(lh.as_bytes());
            hasher.update(rh.as_bytes());
            let digest: [u8; 20] = hasher.finalize().into();
            let h = AichHash(digest);
            node.hash = Some(h);
            Some(h)
        }
        _ => None,
    }
}

/// Reads recovery data received for `part_index` of a `file_size`-byte file and
/// returns the expected SHA-1 hash of every 180 KiB block of that part, after
/// verifying the reconstructed tree against `master_hash`
/// (`CAICHRecoveryHashSet::ReadRecoveryData` + `VerifyHashTree`).
pub fn read_recovery_data(
    file_size: u64,
    part_index: u32,
    master_hash: AichHash,
    entries: &[AichRecoveryEntry],
) -> Result<Vec<AichHash>, FormatError> {
    if file_size == 0 {
        return Err(FormatError::EmptyInput);
    }
    if entries.is_empty() {
        return Err(FormatError::AichVerificationFailed(
            "recovery data contains no hashes".to_string(),
        ));
    }
    let part_start = part_index as u64 * ED2K_PART_SIZE as u64;
    if part_start >= file_size {
        return Err(FormatError::AichVerificationFailed(
            "part index out of range".to_string(),
        ));
    }
    let part_size = (ED2K_PART_SIZE as u64).min(file_size - part_start);
    let blocks_in_part = part_size.div_ceil(AICH_BLOCK_SIZE as u64) as usize;

    let mut tree = AichNode::new_tree(file_size)?;
    for entry in entries {
        if !set_hash_by_ident(&mut tree, entry.ident, entry.hash, false) {
            return Err(FormatError::AichVerificationFailed(format!(
                "invalid hash identifier {:#x} in recovery data",
                entry.ident
            )));
        }
    }

    match recalculate_partial(&mut tree) {
        Some(root) if root == master_hash => {}
        Some(_) => {
            return Err(FormatError::AichVerificationFailed(
                "reconstructed AICH root does not match master hash".to_string(),
            ))
        }
        None => {
            return Err(FormatError::AichVerificationFailed(
                "incomplete AICH tree after recovery data".to_string(),
            ))
        }
    }

    // Extract the part's leaf block hashes in left-to-right order.
    let mut block_hashes = Vec::with_capacity(blocks_in_part);
    collect_part_block_hashes(&tree, part_start, part_size, &mut block_hashes);
    if block_hashes.len() != blocks_in_part {
        return Err(FormatError::AichVerificationFailed(format!(
            "recovery data only covers {} of {blocks_in_part} blocks",
            block_hashes.len()
        )));
    }
    Ok(block_hashes)
}

fn collect_part_block_hashes(node: &AichNode, start: u64, size: u64, out: &mut Vec<AichHash>) {
    if node.data_size <= node.base_size {
        if node.data_size == size && start == 0 {
            if let Some(h) = node.hash {
                out.push(h);
            }
        }
        return;
    }
    let n_blocks = node.data_size.div_ceil(node.base_size);
    let left_blocks = if node.is_left_branch {
        n_blocks.div_ceil(2)
    } else {
        n_blocks / 2
    };
    let left_size = left_blocks * node.base_size;
    if start < left_size {
        if let Some(l) = &node.left {
            collect_part_block_hashes(l, start, size.min(left_size - start), out);
        }
        if start + size > left_size {
            if let Some(r) = &node.right {
                collect_part_block_hashes(r, 0, start + size - left_size, out);
            }
        }
    } else if let Some(r) = &node.right {
        collect_part_block_hashes(r, start - left_size, size, out);
    }
}

/// SHA-1 hash of a single AICH block (<= 180 KiB of file data).
pub fn aich_block_hash(block: &[u8]) -> AichHash {
    AichHash(Sha1::digest(block).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data(blocks: usize) -> Vec<u8> {
        let mut data = Vec::with_capacity(blocks * AICH_BLOCK_SIZE);
        for i in 0..blocks {
            for j in 0..AICH_BLOCK_SIZE {
                data.push((i * 31 + j % 7) as u8);
            }
        }
        data
    }

    #[test]
    fn recovery_roundtrip_full_part() {
        // 3 AICH blocks in a single (partial) eD2K part.
        let data = sample_data(3);
        let tree = build_aich_tree(&data).unwrap();
        let root = tree.root_hash().unwrap();

        let entries = create_part_recovery_data(&tree, data.len() as u64, 0).unwrap();
        assert_eq!(entries.len(), 3);

        let block_hashes = read_recovery_data(data.len() as u64, 0, root, &entries).unwrap();
        assert_eq!(block_hashes.len(), 3);
        for (i, h) in block_hashes.iter().enumerate() {
            let block = &data[i * AICH_BLOCK_SIZE..(i + 1) * AICH_BLOCK_SIZE];
            assert_eq!(*h, aich_block_hash(block));
        }
    }

    #[test]
    fn recovery_roundtrip_last_partial_block() {
        // 2 full blocks + a 100-byte trailing block.
        let mut data = sample_data(2);
        data.extend_from_slice(&[0x55u8; 100]);
        let tree = build_aich_tree(&data).unwrap();
        let root = tree.root_hash().unwrap();

        let entries = create_part_recovery_data(&tree, data.len() as u64, 0).unwrap();
        let block_hashes = read_recovery_data(data.len() as u64, 0, root, &entries).unwrap();
        assert_eq!(block_hashes.len(), 3);
        assert_eq!(block_hashes[2], aich_block_hash(&[0x55u8; 100]));
    }

    #[test]
    fn recovery_rejects_wrong_master_hash() {
        let data = sample_data(3);
        let tree = build_aich_tree(&data).unwrap();
        let entries = create_part_recovery_data(&tree, data.len() as u64, 0).unwrap();

        let wrong = AichHash([0xEE; 20]);
        assert!(read_recovery_data(data.len() as u64, 0, wrong, &entries).is_err());
    }

    #[test]
    fn recovery_rejects_invalid_ident() {
        let data = sample_data(3);
        let tree = build_aich_tree(&data).unwrap();
        let root = tree.root_hash().unwrap();
        let mut entries = create_part_recovery_data(&tree, data.len() as u64, 0).unwrap();
        // ident 0 / 1 address the master hash itself — forbidden.
        entries[0].ident = 1;
        assert!(read_recovery_data(data.len() as u64, 0, root, &entries).is_err());
        entries[0].ident = 0;
        assert!(read_recovery_data(data.len() as u64, 0, root, &entries).is_err());
        // ident pointing beyond the tree depth.
        entries[0].ident = 0x7FFF_FFFF;
        assert!(read_recovery_data(data.len() as u64, 0, root, &entries).is_err());
    }

    #[test]
    fn recovery_rejects_incomplete_tree() {
        let data = sample_data(4);
        let tree = build_aich_tree(&data).unwrap();
        let root = tree.root_hash().unwrap();
        let mut entries = create_part_recovery_data(&tree, data.len() as u64, 0).unwrap();
        entries.pop();
        assert!(read_recovery_data(data.len() as u64, 0, root, &entries).is_err());
    }

    #[test]
    fn recovery_multi_part_tree() {
        // File spanning 2 eD2K parts worth of AICH blocks is too large for a
        // test; emulate a 60-block file (1 full part boundary semantics kept).
        let data = sample_data(60);
        let tree = build_aich_tree(&data).unwrap();
        let root = tree.root_hash().unwrap();

        let entries = create_part_recovery_data(&tree, data.len() as u64, 0).unwrap();
        // Part 0 covers blocks 0..53; sibling hashes for the remaining 7 blocks
        // are included.
        let block_hashes = read_recovery_data(data.len() as u64, 0, root, &entries).unwrap();
        assert_eq!(block_hashes.len(), 53);
        // Block boundaries are part-aligned: block 52 covers only 143,360
        // bytes (rest of part 0).
        let mut expected = Vec::new();
        AichNode::for_each_block_range(data.len() as u64, |start, len| {
            expected.push(aich_block_hash(
                &data[start as usize..(start + len) as usize],
            ));
        });
        assert_eq!(block_hashes, expected[..53]);
    }

    #[test]
    fn streaming_reader_matches_in_memory() {
        let data = sample_data(5);
        let t1 = build_aich_tree(&data).unwrap();
        let mut cursor = std::io::Cursor::new(&data);
        let t2 = build_aich_tree_from_reader(&mut cursor, data.len() as u64).unwrap();
        assert_eq!(t1.root_hash(), t2.root_hash());
    }
}
