use rusqlite::{params, Connection};

use crate::error::CoreError;
use crate::model::SharedFile;

fn shared_file_from_row(row: &rusqlite::Row) -> rusqlite::Result<SharedFile> {
    let size_i64: i64 = row.get(2)?;
    let enabled_int: i32 = row.get(5)?;
    let aich_blob: Option<Vec<u8>> = row.get(6)?;
    let aich_root = aich_blob.and_then(|b| {
        if b.len() == 20 {
            let mut bytes = [0u8; 20];
            bytes.copy_from_slice(&b);
            Some(bytes)
        } else {
            None
        }
    });
    Ok(SharedFile {
        file_hash: row.get(0)?,
        file_name: row.get(1)?,
        file_size: size_i64.max(0) as u64,
        file_path: row.get(3)?,
        aich_root,
        shared_at_unix_ms: row.get(4)?,
        enabled: enabled_int != 0,
        up_priority: row.get::<_, Option<String>>(7)?.unwrap_or_else(|| "normal".into()),
        is_partial: false,
        completed_parts: Vec::new(),
        uploaded_bytes: 0,
        upload_rate_bps: 0,
    })
}

pub fn upsert_shared_file(conn: &mut Connection, file: &SharedFile) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO shared_files (file_hash, file_name, file_size, file_path, shared_at_unix_ms, enabled, aich_root, up_priority)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(file_hash) DO UPDATE SET
            file_name = excluded.file_name,
            file_size = excluded.file_size,
            file_path = excluded.file_path,
            enabled = excluded.enabled,
            up_priority = excluded.up_priority,
            aich_root = COALESCE(excluded.aich_root, shared_files.aich_root)",
        params![
            file.file_hash,
            file.file_name,
            file.file_size as i64,
            file.file_path,
            file.shared_at_unix_ms,
            if file.enabled { 1 } else { 0 },
            file.aich_root.map(|a| a.to_vec()),
            file.up_priority,
        ],
    )?;
    Ok(())
}

/// Caches the computed AICH root hash of a shared file.
pub fn set_shared_aich_root(
    conn: &mut Connection,
    file_hash: &str,
    aich_root: &[u8; 20],
) -> Result<bool, CoreError> {
    let rows = conn.execute(
        "UPDATE shared_files SET aich_root = ?1 WHERE file_hash = ?2",
        params![aich_root.as_slice(), file_hash],
    )?;
    Ok(rows > 0)
}

pub fn list_shared_files(conn: &Connection) -> Result<Vec<SharedFile>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT file_hash, file_name, file_size, file_path, shared_at_unix_ms, enabled, aich_root, up_priority
         FROM shared_files
         ORDER BY shared_at_unix_ms DESC",
    )?;
    let rows = stmt.query_map([], shared_file_from_row)?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn get_shared_file(
    conn: &Connection,
    file_hash: &str,
) -> Result<Option<SharedFile>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT file_hash, file_name, file_size, file_path, shared_at_unix_ms, enabled, aich_root, up_priority
         FROM shared_files
         WHERE file_hash = ?1",
    )?;
    let mut rows = stmt.query(params![file_hash])?;
    if let Some(row) = rows.next()? {
        Ok(Some(shared_file_from_row(row)?))
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

/// Updates the eMule upload priority (`PR_*`) of a shared file:
/// "verylow" | "low" | "normal" | "high" | "veryhigh" (= powershare).
pub fn set_shared_up_priority(
    conn: &mut Connection,
    file_hash: &str,
    up_priority: &str,
) -> Result<bool, CoreError> {
    let rows = conn.execute(
        "UPDATE shared_files SET up_priority = ?1 WHERE file_hash = ?2",
        params![up_priority, file_hash],
    )?;
    Ok(rows > 0)
}

pub fn set_shared_file_enabled(
    conn: &mut Connection,
    file_hash: &str,
    enabled: bool,
) -> Result<bool, CoreError> {
    let rows = conn.execute(
        "UPDATE shared_files SET enabled = ?1 WHERE file_hash = ?2",
        params![if enabled { 1 } else { 0 }, file_hash],
    )?;
    Ok(rows > 0)
}

/// Ajoute `delta` octets au total uploadé persisté d'un fichier partagé
/// (`share_upload_stats` — table séparée car les partages partiels
/// `CPartFile` n'ont pas de ligne dans `shared_files`).
pub fn add_share_uploaded_bytes(
    conn: &mut Connection,
    file_hash: &str,
    delta: u64,
) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO share_upload_stats (file_hash, uploaded_bytes) VALUES (?1, ?2)
         ON CONFLICT(file_hash) DO UPDATE SET
            uploaded_bytes = uploaded_bytes + excluded.uploaded_bytes",
        params![file_hash.to_lowercase(), delta as i64],
    )?;
    Ok(())
}

/// Totaux uploadés persistés par hash de fichier (clés en minuscules).
pub fn list_share_uploaded_bytes(
    conn: &Connection,
) -> Result<std::collections::HashMap<String, u64>, CoreError> {
    let mut stmt =
        conn.prepare("SELECT file_hash, uploaded_bytes FROM share_upload_stats")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut map = std::collections::HashMap::new();
    for r in rows {
        let (hash, bytes) = r?;
        map.insert(hash.to_lowercase(), bytes.max(0) as u64);
    }
    Ok(map)
}
