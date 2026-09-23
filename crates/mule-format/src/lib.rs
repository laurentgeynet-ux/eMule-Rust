// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

pub mod aich;
pub mod ed2k;
pub mod error;
pub mod fixtures;
pub mod kad;
pub mod met;

pub use kad::{
    KadContactEntry, NodesDatFile, MAX_NODES_COUNT, MAX_NODES_DAT_SIZE, MAX_TAGS_PER_NODE,
};

pub use aich::{
    aich_block_hash, build_aich_tree, build_aich_tree_from_reader, calculate_aich_root,
    compute_expected_path, create_part_recovery_data, read_recovery_data, set_hash_by_ident,
    verify_aich_proof, AichHash, AichNode, AichProof, AichProofError, AichRecoveryEntry,
    AichSibling, SiblingPosition, AICH_BLOCK_SIZE, BLOCKS_PER_PART, LAST_BLOCK_SIZE_FULL_PART,
};
pub use ed2k::{
    hash_reader, Ed2kFileLink, Ed2kHash, Ed2kHashResult, Ed2kHasher, PeerSource, ED2K_PART_SIZE,
    MD4_EMPTY_DIGEST,
};
pub use error::FormatError;
pub use fixtures::{
    BytePatternFixture, PatternReader, FIXTURE_1_BYTE, FIXTURE_EXACT_ONE_PART,
    FIXTURE_EXACT_TWO_PARTS, FIXTURE_JUST_UNDER_PART, FIXTURE_ONE_PART_PLUS_ONE,
    REPEATING_0_TO_255,
};
pub use met::{
    read_tag, CountingReader, MetError, PartMetFile, ServerEntry, ServerMetFile, Tag, TagValue,
    MAX_SERVER_COUNT, PARTFILE_SPLITTEDVERSION, PARTFILE_VERSION, PARTFILE_VERSION_LARGEFILE,
    SERVER_MET_VERSION, SN_DESCRIPTION, SN_DYNIP, SN_FAIL, SN_HARDFILES, SN_IP, SN_LASTPING,
    SN_MAXUSERS, SN_NAME, SN_PING, SN_PORT, SN_PREFERENCE, SN_SOFTFILES, SN_VERSION,
};
