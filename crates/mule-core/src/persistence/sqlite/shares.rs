use rusqlite::{params, Connection};

use crate::error::CoreError;
use crate::model::SharedFile;

pub fn upsert_shared_file(conn: &mut Connection, file: &SharedFile) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO shared_files (file_hash, file_name, file_size, file_path, shared_at_unix_ms, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(file_hash) DO UPDATE SET
            file_name = excluded.file_name,
            file_size = excluded.file_size,
            file_path = excluded.file_path,
            enabled = excluded.enabled",
        params![
            file.file_hash,
            file.file_name,
            file.file_size as i64,
            file.file_path,
            file.shared_at_unix_ms,
            if file.enabled { 1 } else { 0 },
        ],
    )?;
    Ok(())
}

pub fn list_shared_files(conn: &Connection) -> Result<Vec<SharedFile>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT file_hash, file_name, file_size, file_path, shared_at_unix_ms, enabled
         FROM shared_files
         ORDER BY shared_at_unix_ms DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let size_i64: i64 = row.get(2)?;
        let enabled_int: i32 = row.get(5)?;
        Ok(SharedFile {
            file_hash: row.get(0)?,
            file_name: row.get(1)?,
            file_size: size_i64.max(0) as u64,
            file_path: row.get(3)?,
            shared_at_unix_ms: row.get(4)?,
            enabled: enabled_int != 0,
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn get_shared_file(conn: &Connection, file_hash: &str) -> Result<Option<SharedFile>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT file_hash, file_name, file_size, file_path, shared_at_unix_ms, enabled
         FROM shared_files
         WHERE file_hash = ?1",
    )?;
    let mut rows = stmt.query(params![file_hash])?;
    if let Some(row) = rows.next()? {
        let size_i64: i64 = row.get(2)?;
        let enabled_int: i32 = row.get(5)?;
        Ok(Some(SharedFile {
            file_hash: row.get(0)?,
            file_name: row.get(1)?,
            file_size: size_i64.max(0) as u64,
            file_path: row.get(3)?,
            shared_at_unix_ms: row.get(4)?,
            enabled: enabled_int != 0,
        }))
    } else {
        Ok(None)
    }
}

pub fn delete_shared_file(conn: &mut Connection, file_hash: &str) -> Result<bool, CoreError> {
    let rows = conn.execute(
        "DELETE FROM shared_files WHERE file_hash = ?1",
        params![file_hash],
    )?;
    Ok(rows > 0)
}

pub fn set_shared_file_enabled(conn: &mut Connection, file_hash: &str, enabled: bool) -> Result<bool, CoreError> {
    let rows = conn.execute(
        "UPDATE shared_files SET enabled = ?1 WHERE file_hash = ?2",
        params![if enabled { 1 } else { 0 }, file_hash],
    )?;
    Ok(rows > 0)
}
