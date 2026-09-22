use rusqlite::{params, Connection};

use crate::error::CoreError;
use crate::model::{PeerSource, PeerSourceOrigin, PeerSourceStatus};

pub fn upsert_peer_sources(
    conn: &mut Connection,
    file_hash: &str,
    sources: &[PeerSource],
) -> Result<usize, CoreError> {
    let tx = conn.transaction()?;
    let mut count = 0;
    {
        let mut insert_stmt = tx.prepare(
            "INSERT INTO peer_sources (
                file_hash, address, ip, port, origin,
                discovered_at_unix_ms, last_seen_unix_ms, status, fail_count, last_error, cooldown_until_unix_ms,
                user_hash, crypt_options, buddy_id, software, availability
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            ON CONFLICT(file_hash, address) DO UPDATE SET
                last_seen_unix_ms = excluded.last_seen_unix_ms,
                ip = excluded.ip,
                port = excluded.port,
                user_hash = COALESCE(excluded.user_hash, peer_sources.user_hash),
                crypt_options = CASE WHEN excluded.crypt_options != 0 THEN excluded.crypt_options ELSE peer_sources.crypt_options END,
                buddy_id = COALESCE(excluded.buddy_id, peer_sources.buddy_id),
                software = COALESCE(excluded.software, peer_sources.software),
                availability = COALESCE(excluded.availability, peer_sources.availability)"
        )?;

        for s in sources {
            insert_stmt.execute(params![
                file_hash,
                s.address,
                s.ip as i64,
                s.port as i32,
                s.origin.as_str(),
                s.discovered_at_unix_ms,
                s.last_seen_unix_ms,
                s.status.as_str(),
                s.fail_count as i32,
                s.last_error,
                s.cooldown_until_unix_ms,
                s.user_hash,
                s.crypt_options as i32,
                s.buddy_id,
                s.software,
                s.availability,
            ])?;
            count += 1;
        }
    }
    tx.commit()?;
    Ok(count)
}

pub fn list_peer_sources(conn: &Connection, file_hash: &str) -> Result<Vec<PeerSource>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT file_hash, address, ip, port, origin,
                discovered_at_unix_ms, last_seen_unix_ms, status, fail_count, last_error, cooldown_until_unix_ms,
                user_hash, crypt_options, buddy_id, software, availability
         FROM peer_sources
         WHERE file_hash = ?1
         ORDER BY status ASC, last_seen_unix_ms DESC"
    )?;

    let rows = stmt.query_map(params![file_hash], |row| {
        let origin_str: String = row.get(4)?;
        let origin: PeerSourceOrigin = origin_str.parse().unwrap_or(PeerSourceOrigin::Server);
        let status_str: String = row.get(7)?;
        let status: PeerSourceStatus = status_str.parse().unwrap_or(PeerSourceStatus::Candidate);
        let ip_i64: i64 = row.get(2)?;
        let port_i32: i32 = row.get(3)?;
        let fail_count_i32: i32 = row.get(8)?;
        let crypt_options_i32: i32 = row.get(12).unwrap_or(0);

        Ok(PeerSource {
            file_hash: row.get(0)?,
            address: row.get(1)?,
            ip: ip_i64 as u32,
            port: port_i32 as u16,
            origin,
            discovered_at_unix_ms: row.get(5)?,
            last_seen_unix_ms: row.get(6)?,
            status,
            fail_count: fail_count_i32 as u32,
            last_error: row.get(9)?,
            cooldown_until_unix_ms: row.get(10)?,
            user_hash: row.get(11)?,
            crypt_options: crypt_options_i32 as u8,
            buddy_id: row.get(13)?,
            software: row.get(14)?,
            availability: row.get(15)?,
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn update_peer_source_status(
    conn: &mut Connection,
    file_hash: &str,
    address: &str,
    status: PeerSourceStatus,
    error: Option<&str>,
    cooldown_until_unix_ms: Option<i64>,
) -> Result<bool, CoreError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let increment_fail = status == PeerSourceStatus::Failed;
    let rows = if increment_fail {
        conn.execute(
            "UPDATE peer_sources SET status = ?1, fail_count = fail_count + 1, last_error = ?2, last_seen_unix_ms = ?3, cooldown_until_unix_ms = ?4
             WHERE file_hash = ?5 AND address = ?6",
            params![status.as_str(), error, now, cooldown_until_unix_ms, file_hash, address],
        )?
    } else {
        conn.execute(
            "UPDATE peer_sources SET status = ?1, last_error = ?2, last_seen_unix_ms = ?3, cooldown_until_unix_ms = ?4
             WHERE file_hash = ?5 AND address = ?6",
            params![status.as_str(), error, now, cooldown_until_unix_ms, file_hash, address],
        )?
    };
    Ok(rows > 0)
}

pub fn reset_peer_source_cooldown(
    conn: &mut Connection,
    file_hash: &str,
    address: &str,
) -> Result<bool, CoreError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let rows = conn.execute(
        "UPDATE peer_sources SET cooldown_until_unix_ms = NULL, status = 'allowed', last_error = NULL, last_seen_unix_ms = ?1
         WHERE file_hash = ?2 AND address = ?3",
        params![now, file_hash, address],
    )?;
    Ok(rows > 0)
}

pub fn delete_peer_source(
    conn: &mut Connection,
    file_hash: &str,
    address: &str,
) -> Result<bool, CoreError> {
    let rows = conn.execute(
        "DELETE FROM peer_sources WHERE file_hash = ?1 AND address = ?2",
        params![file_hash, address],
    )?;
    Ok(rows > 0)
}

pub fn clear_peer_sources(conn: &mut Connection, file_hash: &str) -> Result<usize, CoreError> {
    let rows = conn.execute(
        "DELETE FROM peer_sources WHERE file_hash = ?1",
        params![file_hash],
    )?;
    Ok(rows)
}

/// Updates the learned software name and/or the per-part availability bitmask
/// of a peer source (filled from the eD2K hello and OP_FILESTATUS).
pub fn update_peer_source_details(
    conn: &mut Connection,
    file_hash: &str,
    address: &str,
    software: Option<&str>,
    availability: Option<&[u8]>,
) -> Result<bool, CoreError> {
    let rows = conn.execute(
        "UPDATE peer_sources SET
            software = COALESCE(?1, software),
            availability = COALESCE(?2, availability)
         WHERE file_hash = ?3 AND address = ?4",
        params![software, availability, file_hash, address],
    )?;
    Ok(rows > 0)
}
