// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Persistent storage, sparse .part files, and crash-resilient resume state for eD2K transfers.
//!
//! Tier 3.5 formalizes physical file persistence and resume invariants:
//! - Exact-sized `.part` file pre-allocation / sparse representation on disk.
//! - Companion `.resume.json` state journal recording completed byte intervals and total bytes received.
//! - Atomic state persistence via temp file (`.tmp`) and rename to guarantee crash-consistency.
//! - Resumption from partial disk state: reloads `.resume.json`, verifies file hash and size consistency.
//! - Incremental chunk writes into `.part` file with strict duplicate / overlap protection.
//! - In-disk part integrity verification: streams completed part bytes directly into local MD4 hasher
//!   to verify part checksum against fixture/oracle without needing to hold the full part in RAM.

pub mod sink;
pub mod types;

pub use sink::Ed2kPartFileSink;
pub use types::{merge_intervals, Ed2kBlockWriter, Ed2kPartDownloadStatus, Ed2kPartResumeState};
