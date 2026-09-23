// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

pub mod sqlite;
pub mod traits;

pub use sqlite::Database;
pub use traits::{DownloadRepository, KadRepository, PeerSourceRepository, ServerRepository};
