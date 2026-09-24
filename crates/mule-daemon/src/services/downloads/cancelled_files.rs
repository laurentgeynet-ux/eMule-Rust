// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! eMule cancelled-files tracking — `CKnownFileList::AddCancelledFileID`,
//! `IsCancelledFileByID` and `cancelled.met` import
//! (`KnownFileList.cpp:384-411, 167-194`).
//!
//! The set stores masked hashes `MD5(seed || file_hash)` only — like
//! eMule, we never persist the real file hash. Imported `0x0F` files keep
//! their own seed; legacy `0x0E` entries (real hashes) are masked under
//! our local seed at import, exactly what eMule does at load time.
//!
//! Both the record and the lookup are gated by `IsRememberingCancelledFiles`
//! → our `remember_cancelled` setting (the same gate history categories
//! use). The set marks search results as « cancelled »; it does not hard
//! block downloads — same as eMule (`m_MyListCtrlItem` colouring).

use std::path::Path;

use mule_core::persistence::sqlite::Database;
use mule_format::met::{cancelled_mask, CancelledEntry, CancelledMetFile};
use serde::Serialize;

use crate::error::DaemonError;

use super::db_err;
use super::history_categories::setting_enabled;

/// eMule `IsRememberingCancelledFiles` → `remember_cancelled` setting.
const REMEMBER_CANCELLED_KEY: &str = "remember_cancelled";

/// Outcome of `import_cancelled_met`.
#[derive(Debug, Default, Serialize)]
pub struct CancelledImportReport {
    /// `cancelled.met` records read (`m_mapCancelledFiles` size after load).
    pub records: usize,
    /// Masked entries stored under the file's own seed (`0x0F` entries).
    pub masked_imported: usize,
    /// Legacy real hashes masked under our seed (`0x0E` entries).
    pub legacy_masked: usize,
    /// `true` when the file used the deprecated `0x0E` layout.
    pub legacy_format: bool,
    /// Total masked hashes stored (post-import set size).
    pub total: usize,
}

/// `AddCancelledFileID` — records `hash` as cancelled under our seed.
/// No-op when `remember_cancelled` is off (eMule's gate).
pub fn record_cancelled_file(db: &mut Database, hash: &[u8; 16]) -> Result<(), DaemonError> {
    if !setting_enabled(db, REMEMBER_CANCELLED_KEY) {
        return Ok(());
    }
    let seed = db
        .get_or_create_cancelled_seed()
        .map_err(db_err("getting cancelled seed"))?;
    let masked = hex::encode(cancelled_mask(seed, hash));
    db.add_cancelled_hash(seed, &masked)
        .map_err(db_err("recording cancelled hash"))
}

/// `IsCancelledFileByID` — `true` when `hash` is in the cancelled set.
/// Checks every known seed (imported files keep their own). Always `false`
/// when `remember_cancelled` is off, like eMule.
pub fn is_cancelled_file(db: &Database, hash: &[u8; 16]) -> Result<bool, DaemonError> {
    if !setting_enabled(db, REMEMBER_CANCELLED_KEY) {
        return Ok(false);
    }
    for seed in db.cancelled_seeds().map_err(db_err("listing seeds"))? {
        let masked = hex::encode(cancelled_mask(seed, hash));
        if db
            .is_cancelled_masked(seed, &masked)
            .map_err(db_err("checking cancelled hash"))?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `CKnownFileList::LoadCancelled` — imports a `cancelled.met` file.
/// `0x0F` entries are stored under the file's seed; `0x0E` real hashes are
/// masked under our seed (eMule's load-time conversion).
pub fn import_cancelled_met(
    db: &mut Database,
    path: &Path,
) -> Result<CancelledImportReport, DaemonError> {
    let file = CancelledMetFile::open(path)
        .map_err(|e| DaemonError::Business(format!("cancelled.met: {e}")))?;
    let mut report = CancelledImportReport {
        records: file.entries.len(),
        legacy_format: file.legacy_format,
        ..Default::default()
    };
    // `0` is impossible in a valid file — only reached for the legacy
    // layout where no `Masked` entry can exist anyway.
    let file_seed = file.seed.unwrap_or(0);
    for entry in &file.entries {
        match entry {
            CancelledEntry::Masked(masked) => {
                db.add_cancelled_hash(file_seed, &hex::encode(masked))
                    .map_err(db_err("importing cancelled mask"))?;
                report.masked_imported += 1;
            }
            CancelledEntry::Real(hash) => {
                let seed = db
                    .get_or_create_cancelled_seed()
                    .map_err(db_err("getting cancelled seed"))?;
                let masked = hex::encode(cancelled_mask(seed, hash));
                db.add_cancelled_hash(seed, &masked)
                    .map_err(db_err("importing cancelled hash"))?;
                report.legacy_masked += 1;
            }
        }
    }
    report.total = db.cancelled_count().map_err(db_err("counting cancelled"))?;
    Ok(report)
}

/// Parses a hex eD2K hash into `[u8; 16]` (`None` on malformed input).
pub fn parse_hash16(ed2k_hash: &str) -> Option<[u8; 16]> {
    let bytes = hex::decode(ed2k_hash).ok()?;
    <[u8; 16]>::try_from(bytes.as_slice()).ok()
}
