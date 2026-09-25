// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

pub mod hash_tree;
pub mod proof;
pub mod recovery;

pub use hash_tree::{
    calculate_aich_root, AichHash, AichNode, AICH_BLOCK_SIZE, BLOCKS_PER_PART,
    LAST_BLOCK_SIZE_FULL_PART,
};
pub use proof::{
    compute_expected_path, verify_aich_proof, AichProof, AichProofError, AichSibling,
    SiblingPosition,
};
pub use recovery::{
    aich_block_hash, aich_part_hashes, build_aich_tree, build_aich_tree_from_reader,
    create_part_recovery_data, read_recovery_data, set_hash_by_ident, AichRecoveryEntry,
};
