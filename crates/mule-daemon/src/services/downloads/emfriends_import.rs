// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! `emfriends.met` import — `CFriend::LoadFromFile` /
//! `CClientList::LoadFriendsList` (`Friend.cpp`, `ClientList.cpp`).
//!
//! eMule loads every record into `m_listFriends`; friends known only by
//! IP+port (`!HasUserhash`) still land in the list. Our `friends` table is
//! keyed by user hash (a friend grants an upload slot by hash), so only
//! records with `HasUserhash` are imported — the same subset eMule can
//! actually match in `IsFriend`. Imported records carry their `FF_NAME`.

use std::collections::HashSet;
use std::path::Path;

use mule_core::persistence::sqlite::Database;
use mule_format::met::EmfriendsMetFile;
use serde::Serialize;

use crate::error::DaemonError;

use super::db_err;

/// Outcome of `import_emfriends_met`.
#[derive(Debug, Default, Serialize)]
pub struct EmfriendsImportReport {
    /// Records read from `emfriends.met`.
    pub records: usize,
    /// Records without user hash — eMule lists them but our table is
    /// hash-keyed, so they cannot be matched to a peer anyway.
    pub skipped_no_userhash: usize,
    /// Records already known (same user hash) — name refreshed.
    pub updated: usize,
    /// New friends added.
    pub imported: usize,
    /// User hashes (hex) added or updated — the caller registers them
    /// with the upload manager (`set_friend`).
    pub user_hashes: Vec<String>,
}

/// `LoadFriendsList` — imports an `emfriends.met` file into `friends`.
/// Existing entries keep their slot and get their name refreshed
/// (`add_friend` is an upsert, matching `CFriend` overwrite semantics).
pub fn import_emfriends_met(
    db: &mut Database,
    path: &Path,
) -> Result<EmfriendsImportReport, DaemonError> {
    let file = EmfriendsMetFile::open(path)
        .map_err(|e| DaemonError::Business(format!("emfriends.met: {e}")))?;
    let known: HashSet<String> = db
        .list_friends()
        .map_err(db_err("listing friends"))?
        .into_iter()
        .map(|f| f.user_hash)
        .collect();
    let mut report = EmfriendsImportReport {
        records: file.records.len(),
        ..Default::default()
    };
    for record in &file.records {
        if !record.has_userhash() {
            report.skipped_no_userhash += 1;
            continue;
        }
        let user_hash = hex::encode(record.user_hash);
        let name = record.name.clone().unwrap_or_default();
        db.add_friend(&user_hash, &name)
            .map_err(db_err("importing friend"))?;
        if known.contains(&user_hash) {
            report.updated += 1;
        } else {
            report.imported += 1;
        }
        report.user_hashes.push(user_hash);
    }
    Ok(report)
}
