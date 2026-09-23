// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use rusqlite::{params, Connection};

use crate::error::CoreError;
use crate::model::Friend;

pub fn add_friend(conn: &mut Connection, user_hash: &str, name: &str) -> Result<(), CoreError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    conn.execute(
        "INSERT OR REPLACE INTO friends (user_hash, name, added_at_unix_ms) VALUES (?1, ?2, ?3)",
        params![user_hash.to_lowercase(), name, now],
    )?;
    Ok(())
}

pub fn remove_friend(conn: &mut Connection, user_hash: &str) -> Result<(), CoreError> {
    conn.execute(
        "DELETE FROM friends WHERE user_hash = ?1",
        params![user_hash.to_lowercase()],
    )?;
    Ok(())
}

pub fn list_friends(conn: &Connection) -> Result<Vec<Friend>, CoreError> {
    let mut stmt =
        conn.prepare("SELECT user_hash, name, added_at_unix_ms FROM friends ORDER BY added_at_unix_ms")?;
    let mut rows = stmt.query([])?;
    let mut friends = Vec::new();
    while let Some(row) = rows.next()? {
        friends.push(Friend {
            user_hash: row.get(0)?,
            name: row.get(1)?,
            added_at_unix_ms: row.get(2)?,
        });
    }
    Ok(friends)
}
