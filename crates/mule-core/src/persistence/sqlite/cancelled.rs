// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! eMule `cancelled.met` persistence — `CKnownFileList::AddCancelledFileID`/
//! `IsCancelledFileByID` (`KnownFileList.cpp:384-411`).
//!
//! eMule never stores the file hash itself, only `MD5(seed || hash)` —
//! the cancelled list cannot leak what was downloaded then cancelled.
//! Because a mask is bound to its seed, each row carries the seed it was
//! produced with: our own records use the local `cancelled_seed` setting,
//! imported `cancelled.met` files keep the seed written in their header.

use rusqlite::{params, Connection};

use crate::error::CoreError;

/// Setting key storing our `m_dwCancelledFilesSeed` (decimal u32,
/// `1..=0xFFFFFFFE` — generated lazily like eMule).
pub const CANCELLED_SEED_KEY: &str = "cancelled_seed";

/// Records a masked hash under `seed` (`INSERT OR IGNORE` — the set is
/// idempotent like `m_mapCancelledFiles[key] = 1`).
pub fn add_cancelled_hash(
    conn: &mut Connection,
    seed: u32,
    masked_hash_hex: &str,
) -> Result<(), CoreError> {
    conn.execute(
        "INSERT OR IGNORE INTO cancelled_files (masked_hash, seed) VALUES (?1, ?2)",
        params![masked_hash_hex.to_lowercase(), seed as i64],
    )?;
    Ok(())
}

/// Returns `true` when `masked_hash_hex` exists under `seed`.
pub fn is_cancelled_masked(
    conn: &Connection,
    seed: u32,
    masked_hash_hex: &str,
) -> Result<bool, CoreError> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cancelled_files WHERE masked_hash = ?1 AND seed = ?2)",
        params![masked_hash_hex.to_lowercase(), seed as i64],
        |r| r.get(0),
    )?;
    Ok(exists)
}

/// All distinct seeds present — `is_cancelled_file` must try every seed
/// (imported files keep their own `m_dwCancelledFilesSeed`).
pub fn distinct_seeds(conn: &Connection) -> Result<Vec<u32>, CoreError> {
    let mut stmt = conn.prepare("SELECT DISTINCT seed FROM cancelled_files")?;
    let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row? as u32);
    }
    Ok(out)
}

/// Number of stored masked hashes (inspection / import reporting).
pub fn cancelled_count(conn: &Connection) -> Result<usize, CoreError> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM cancelled_files", [], |r| r.get(0))?;
    Ok(n as usize)
}
