// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use rusqlite::{params, Connection};

use crate::error::CoreError;
use crate::model::{HistoryEntry, HistoryFilter, HistoryStats};

pub fn insert_history_entry(conn: &mut Connection, entry: &HistoryEntry) -> Result<i64, CoreError> {
    conn.execute(
        "INSERT INTO download_history (
            download_id, file_name, total_size, transferred_bytes, ed2k_hash,
            event_type, status, duration_secs, connection_attempts, resume_count,
            verification_result, error_message, timestamp_unix_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            entry.download_id,
            entry.file_name,
            entry.total_size as i64,
            entry.transferred_bytes as i64,
            entry.ed2k_hash,
            entry.event_type,
            entry.status,
            entry.duration_secs as i64,
            entry.connection_attempts as i64,
            entry.resume_count as i64,
            entry.verification_result,
            entry.error_message,
            entry.timestamp_unix_ms,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn query_history(
    conn: &Connection,
    filter: &HistoryFilter,
) -> Result<Vec<HistoryEntry>, CoreError> {
    let mut sql = String::from(
        "SELECT id, download_id, file_name, total_size, transferred_bytes, ed2k_hash,
                event_type, status, duration_secs, connection_attempts, resume_count,
                verification_result, error_message, timestamp_unix_ms
         FROM download_history WHERE 1=1",
    );
    let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(ref et) = filter.event_type {
        sql.push_str(" AND event_type = ?");
        params_vec.push(rusqlite::types::Value::Text(et.clone()));
    }
    if let Some(ref st) = filter.status {
        sql.push_str(" AND status = ?");
        params_vec.push(rusqlite::types::Value::Text(st.clone()));
    }
    if let Some(from_ts) = filter.from_timestamp_ms {
        sql.push_str(" AND timestamp_unix_ms >= ?");
        params_vec.push(rusqlite::types::Value::Integer(from_ts));
    }
    if let Some(to_ts) = filter.to_timestamp_ms {
        sql.push_str(" AND timestamp_unix_ms <= ?");
        params_vec.push(rusqlite::types::Value::Integer(to_ts));
    }

    sql.push_str(" ORDER BY timestamp_unix_ms DESC, id DESC");

    let limit = filter.limit.unwrap_or(200);
    sql.push_str(&format!(" LIMIT {limit}"));

    if let Some(offset) = filter.offset {
        sql.push_str(&format!(" OFFSET {offset}"));
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), |row| {
        let total_size_i64: i64 = row.get(3)?;
        let transferred_bytes_i64: i64 = row.get(4)?;
        let duration_secs_i64: i64 = row.get(8)?;
        let attempts_i64: i64 = row.get(9)?;
        let resumes_i64: i64 = row.get(10)?;

        Ok(HistoryEntry {
            id: row.get(0)?,
            download_id: row.get(1)?,
            file_name: row.get(2)?,
            total_size: total_size_i64.max(0) as u64,
            transferred_bytes: transferred_bytes_i64.max(0) as u64,
            ed2k_hash: row.get(5)?,
            event_type: row.get(6)?,
            status: row.get(7)?,
            duration_secs: duration_secs_i64.max(0) as u64,
            connection_attempts: attempts_i64.max(0) as u32,
            resume_count: resumes_i64.max(0) as u32,
            verification_result: row.get(11)?,
            error_message: row.get(12)?,
            timestamp_unix_ms: row.get(13)?,
        })
    })?;

    let mut entries = Vec::new();
    for r in rows {
        entries.push(r?);
    }
    Ok(entries)
}

pub fn get_history_stats(
    conn: &Connection,
    session_start_unix_ms: i64,
) -> Result<HistoryStats, CoreError> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let day_24h_ms = now_ms - (86_400 * 1000);
    let day_7d_ms = now_ms - (7 * 86_400 * 1000);

    let mut stats = HistoryStats::default();

    // 1. Lifetime completed count & bytes
    let (tot_comp_cnt, tot_comp_bytes): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(total_size), 0) FROM download_history WHERE event_type = 'completed'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    stats.total_completed_count = tot_comp_cnt.max(0) as u64;
    stats.total_completed_bytes = tot_comp_bytes.max(0) as u64;

    // 2. Lifetime failures
    let tot_fail: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM download_history WHERE event_type = 'failed'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    stats.total_failed_count = tot_fail.max(0) as u64;

    // 3. Lifetime resumes
    let tot_resumes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM download_history WHERE event_type = 'resumed'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    stats.total_resumes_count = tot_resumes.max(0) as u64;

    // 4. Lifetime integrity failures
    let tot_integrity_fails: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM download_history WHERE verification_result = 'hash_mismatch'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    stats.total_integrity_failures = tot_integrity_fails.max(0) as u64;

    // 5. Session window
    let (sess_cnt, sess_bytes): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(total_size), 0) FROM download_history
             WHERE event_type = 'completed' AND timestamp_unix_ms >= ?1",
            params![session_start_unix_ms],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    stats.session_completed_count = sess_cnt.max(0) as u64;
    stats.session_completed_bytes = sess_bytes.max(0) as u64;

    // 6. 24 Hours window
    let (d24_cnt, d24_bytes): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(total_size), 0) FROM download_history
             WHERE event_type = 'completed' AND timestamp_unix_ms >= ?1",
            params![day_24h_ms],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    stats.last_24h_completed_count = d24_cnt.max(0) as u64;
    stats.last_24h_completed_bytes = d24_bytes.max(0) as u64;

    // 7. 7 Days window
    let (d7_cnt, d7_bytes): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(total_size), 0) FROM download_history
             WHERE event_type = 'completed' AND timestamp_unix_ms >= ?1",
            params![day_7d_ms],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    stats.last_7d_completed_count = d7_cnt.max(0) as u64;
    stats.last_7d_completed_bytes = d7_bytes.max(0) as u64;

    Ok(stats)
}

pub fn clear_history(conn: &mut Connection) -> Result<usize, CoreError> {
    let rows = conn.execute("DELETE FROM download_history", [])?;
    Ok(rows)
}

pub fn apply_retention_policy(
    conn: &mut Connection,
    retention_days: u32,
) -> Result<usize, CoreError> {
    if retention_days == 0 {
        return Ok(0);
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let cutoff_ms = now_ms - (retention_days as i64 * 86_400 * 1000);
    let rows = conn.execute(
        "DELETE FROM download_history WHERE timestamp_unix_ms < ?1",
        params![cutoff_ms],
    )?;
    Ok(rows)
}
