// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

pub mod error;
pub mod kad;
pub mod model;
pub mod persistence;
pub mod ranges;
pub mod storage;
pub mod verification;

pub use kad::{
    KBucket, KBucketTable, KadContact, KadId, K_BUCKET_SIZE, NUM_BUCKETS, REPLACEMENT_CACHE_SIZE,
};

pub use error::CoreError;
pub use model::{
    Download, DownloadCategory, DownloadStatus, Ed2kServer, HistoryEntry, HistoryFilter,
    HistoryStats, PeerSource, PeerSourceOrigin, PeerSourceStatus, ServerFilterStatus,
};
pub use persistence::{
    Database, DownloadRepository, KadRepository, PeerSourceRepository, ServerRepository,
};
pub use ranges::RangeSet;
pub use storage::{finalize_file, AllocationPolicy, CollisionStrategy, PartFile};
pub use verification::{verify_aich_block, verify_part_hash};
