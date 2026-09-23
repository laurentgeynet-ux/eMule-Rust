// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use rusqlite::{params, Connection};

use crate::error::CoreError;
use crate::model::{Ed2kServer, ServerFilterStatus};

pub fn upsert_servers(conn: &mut Connection, servers: &[Ed2kServer]) -> Result<usize, CoreError> {
    let tx = conn.transaction()?;
    let mut count = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO ed2k_servers (
                address, ip, port, name, description, ping, preference, failed_count,
                dyn_ip, version, filter_status, last_seen_unix_ms, created_at_unix_ms, updated_at_unix_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(address) DO UPDATE SET
                ip = excluded.ip,
                port = excluded.port,
                name = excluded.name,
                description = excluded.description,
                ping = excluded.ping,
                preference = excluded.preference,
                failed_count = excluded.failed_count,
                dyn_ip = excluded.dyn_ip,
                version = excluded.version,
                filter_status = CASE
                    WHEN ed2k_servers.filter_status = 'disabled' THEN 'disabled'
                    ELSE excluded.filter_status
                END,
                updated_at_unix_ms = excluded.updated_at_unix_ms"
        )?;

        for s in servers {
            stmt.execute(params![
                s.address,
                s.ip,
                s.port,
                s.name,
                s.description,
                s.ping,
                s.preference,
                s.failed_count,
                s.dyn_ip,
                s.version,
                s.filter_status.as_str(),
                s.last_seen_unix_ms,
                s.created_at_unix_ms,
                s.updated_at_unix_ms,
            ])?;
            count += 1;
        }
    }
    tx.commit()?;
    Ok(count)
}

pub fn list_servers(conn: &Connection) -> Result<Vec<Ed2kServer>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT address, ip, port, name, description, ping, preference, failed_count,
                dyn_ip, version, filter_status, last_seen_unix_ms, created_at_unix_ms, updated_at_unix_ms
         FROM ed2k_servers
         ORDER BY preference DESC, name ASC"
    )?;

    let rows = stmt.query_map([], |row| {
        let filter_status_str: String = row.get(10)?;
        let filter_status: ServerFilterStatus = filter_status_str
            .parse()
            .unwrap_or(ServerFilterStatus::Allowed);

        Ok(Ed2kServer {
            address: row.get(0)?,
            ip: row.get(1)?,
            port: row.get(2)?,
            name: row.get(3)?,
            description: row.get(4)?,
            ping: row.get(5)?,
            preference: row.get(6)?,
            failed_count: row.get(7)?,
            dyn_ip: row.get(8)?,
            version: row.get(9)?,
            filter_status,
            last_seen_unix_ms: row.get(11)?,
            created_at_unix_ms: row.get(12)?,
            updated_at_unix_ms: row.get(13)?,
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn set_server_enabled(
    conn: &mut Connection,
    address: &str,
    enabled: bool,
) -> Result<bool, CoreError> {
    let new_status = if enabled {
        ServerFilterStatus::Allowed.as_str()
    } else {
        ServerFilterStatus::Disabled.as_str()
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let rows = conn.execute(
        "UPDATE ed2k_servers SET filter_status = ?1, updated_at_unix_ms = ?2 WHERE address = ?3",
        params![new_status, now, address],
    )?;
    Ok(rows > 0)
}

pub fn delete_server(conn: &mut Connection, address: &str) -> Result<bool, CoreError> {
    let rows = conn.execute(
        "DELETE FROM ed2k_servers WHERE address = ?1",
        params![address],
    )?;
    Ok(rows > 0)
}

pub fn clear_servers(conn: &mut Connection) -> Result<usize, CoreError> {
    let rows = conn.execute("DELETE FROM ed2k_servers", [])?;
    Ok(rows)
}
