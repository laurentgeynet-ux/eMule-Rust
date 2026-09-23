// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use rusqlite::{params, Connection};

use crate::error::CoreError;
use crate::model::PeerCredit;

pub fn upsert_peer_credit(conn: &mut Connection, credit: &PeerCredit) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO peer_credits (pubkey_hash, pubkey_hex, uploaded_bytes, downloaded_bytes, first_seen_unix_ms, last_seen_unix_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(pubkey_hash) DO UPDATE SET
            pubkey_hex = excluded.pubkey_hex,
            uploaded_bytes = excluded.uploaded_bytes,
            downloaded_bytes = excluded.downloaded_bytes,
            last_seen_unix_ms = excluded.last_seen_unix_ms",
        params![
            credit.pubkey_hash,
            credit.pubkey_hex,
            credit.uploaded_bytes as i64,
            credit.downloaded_bytes as i64,
            credit.first_seen_unix_ms,
            credit.last_seen_unix_ms,
        ],
    )?;
    Ok(())
}

pub fn get_peer_credit(
    conn: &Connection,
    pubkey_hash: &str,
) -> Result<Option<PeerCredit>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT pubkey_hash, pubkey_hex, uploaded_bytes, downloaded_bytes, first_seen_unix_ms, last_seen_unix_ms
         FROM peer_credits
         WHERE pubkey_hash = ?1",
    )?;
    let mut rows = stmt.query(params![pubkey_hash])?;
    if let Some(row) = rows.next()? {
        let up: i64 = row.get(2)?;
        let down: i64 = row.get(3)?;
        Ok(Some(PeerCredit {
            pubkey_hash: row.get(0)?,
            pubkey_hex: row.get(1)?,
            uploaded_bytes: up.max(0) as u64,
            downloaded_bytes: down.max(0) as u64,
            first_seen_unix_ms: row.get(4)?,
            last_seen_unix_ms: row.get(5)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn record_credit_upload(
    conn: &mut Connection,
    pubkey_hash: &str,
    pubkey_hex: &str,
    bytes: u64,
) -> Result<(), CoreError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    conn.execute(
        "INSERT INTO peer_credits (pubkey_hash, pubkey_hex, uploaded_bytes, downloaded_bytes, first_seen_unix_ms, last_seen_unix_ms)
         VALUES (?1, ?2, ?3, 0, ?4, ?4)
         ON CONFLICT(pubkey_hash) DO UPDATE SET
            uploaded_bytes = uploaded_bytes + ?3,
            last_seen_unix_ms = ?4",
        params![pubkey_hash, pubkey_hex, bytes as i64, now],
    )?;
    Ok(())
}

pub fn record_credit_download(
    conn: &mut Connection,
    pubkey_hash: &str,
    pubkey_hex: &str,
    bytes: u64,
) -> Result<(), CoreError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    conn.execute(
        "INSERT INTO peer_credits (pubkey_hash, pubkey_hex, uploaded_bytes, downloaded_bytes, first_seen_unix_ms, last_seen_unix_ms)
         VALUES (?1, ?2, 0, ?3, ?4, ?4)
         ON CONFLICT(pubkey_hash) DO UPDATE SET
            downloaded_bytes = downloaded_bytes + ?3,
            last_seen_unix_ms = ?4",
        params![pubkey_hash, pubkey_hex, bytes as i64, now],
    )?;
    Ok(())
}

pub fn list_peer_credits(conn: &Connection, limit: usize) -> Result<Vec<PeerCredit>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT pubkey_hash, pubkey_hex, uploaded_bytes, downloaded_bytes, first_seen_unix_ms, last_seen_unix_ms
         FROM peer_credits
         ORDER BY downloaded_bytes DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        let up: i64 = row.get(2)?;
        let down: i64 = row.get(3)?;
        Ok(PeerCredit {
            pubkey_hash: row.get(0)?,
            pubkey_hex: row.get(1)?,
            uploaded_bytes: up.max(0) as u64,
            downloaded_bytes: down.max(0) as u64,
            first_seen_unix_ms: row.get(4)?,
            last_seen_unix_ms: row.get(5)?,
        })
    })?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}
