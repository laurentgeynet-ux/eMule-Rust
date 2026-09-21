use rusqlite::{params, Connection};
use std::path::Path;
use uuid::Uuid;

use crate::error::CoreError;
use crate::model::{Download, DownloadStatus};
use crate::ranges::RangeSet;

pub fn insert_download(conn: &mut Connection, dl: &Download) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO downloads (id, display_name, target_path, temp_path, total_size, ed2k_hash, aich_root, status, created_at, updated_at, category_id, priority)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            dl.id.to_string(),
            dl.display_name,
            dl.target_path.to_str().unwrap_or_default(),
            dl.temp_path.to_str().unwrap_or_default(),
            dl.total_size as i64,
            dl.ed2k_hash.0.as_slice(),
            dl.aich_root.as_ref().map(|a| a.0.as_slice()),
            dl.status.as_str(),
            dl.created_at,
            dl.updated_at,
            dl.category_id,
            dl.priority,
        ],
    )?;
    Ok(())
}

/// Persists the per-download priority ("low" | "normal" | "high").
pub fn update_priority(
    conn: &mut Connection,
    download_id: &Uuid,
    priority: &str,
) -> Result<(), CoreError> {
    conn.execute(
        "UPDATE downloads SET priority = ?1 WHERE id = ?2",
        params![priority, download_id.to_string()],
    )?;
    Ok(())
}

/// Persists the trusted AICH root hash for a download.
pub fn update_aich_root(
    conn: &mut Connection,
    download_id: &Uuid,
    aich_root: &[u8; 20],
) -> Result<(), CoreError> {
    conn.execute(
        "UPDATE downloads SET aich_root = ?1 WHERE id = ?2",
        params![aich_root.as_slice(), download_id.to_string()],
    )?;
    Ok(())
}

pub fn sync_received_ranges(
    conn: &mut Connection,
    download_id: &Uuid,
    ranges: &RangeSet,
) -> Result<(), CoreError> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM received_ranges WHERE download_id = ?1",
        params![download_id.to_string()],
    )?;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO received_ranges (download_id, start_offset, end_offset) VALUES (?1, ?2, ?3)",
        )?;
        for r in ranges.ranges() {
            stmt.execute(params![
                download_id.to_string(),
                r.start as i64,
                r.end as i64
            ])?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn load_received_ranges(conn: &Connection, download_id: &Uuid) -> Result<RangeSet, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT start_offset, end_offset FROM received_ranges WHERE download_id = ?1 ORDER BY start_offset ASC",
    )?;

    let mut rows = stmt.query(params![download_id.to_string()])?;
    let mut set = RangeSet::new();

    while let Some(row) = rows.next()? {
        let s: i64 = row.get(0)?;
        let e: i64 = row.get(1)?;
        set.insert(s as u64..e as u64)?;
    }

    Ok(set)
}

pub fn mark_part_verified(
    conn: &mut Connection,
    download_id: &Uuid,
    part_index: u32,
) -> Result<(), CoreError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    conn.execute(
        "INSERT OR REPLACE INTO verified_parts (download_id, part_index, verified_at) VALUES (?1, ?2, ?3)",
        params![download_id.to_string(), part_index as i64, now],
    )?;
    Ok(())
}

pub fn unmark_part_verified(
    conn: &mut Connection,
    download_id: &Uuid,
    part_index: u32,
) -> Result<(), CoreError> {
    conn.execute(
        "DELETE FROM verified_parts WHERE download_id = ?1 AND part_index = ?2",
        params![download_id.to_string(), part_index as i64],
    )?;
    Ok(())
}

pub fn get_verified_parts(conn: &Connection, download_id: &Uuid) -> Result<Vec<u32>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT part_index FROM verified_parts WHERE download_id = ?1 ORDER BY part_index ASC",
    )?;
    let mut rows = stmt.query(params![download_id.to_string()])?;
    let mut parts = Vec::new();
    while let Some(row) = rows.next()? {
        let p: i64 = row.get(0)?;
        parts.push(p as u32);
    }
    Ok(parts)
}

/// Stores the full per-part MD4 hashset for a file (keyed by lowercase hex ed2k hash).
pub fn store_part_hashes(
    conn: &mut Connection,
    file_hash: &str,
    part_hashes: &[[u8; 16]],
) -> Result<(), CoreError> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM file_part_hashes WHERE file_hash = ?1",
        params![file_hash],
    )?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO file_part_hashes (file_hash, part_index, hash) VALUES (?1, ?2, ?3)",
        )?;
        for (i, h) in part_hashes.iter().enumerate() {
            stmt.execute(params![file_hash, i as i64, h.as_slice()])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Loads the stored per-part MD4 hashset for a file, ordered by part index.
pub fn get_part_hashes(conn: &Connection, file_hash: &str) -> Result<Vec<[u8; 16]>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT part_index, hash FROM file_part_hashes WHERE file_hash = ?1 ORDER BY part_index ASC",
    )?;
    let mut rows = stmt.query(params![file_hash])?;
    let mut hashes = Vec::new();
    while let Some(row) = rows.next()? {
        let blob: Vec<u8> = row.get(1)?;
        if blob.len() != 16 {
            return Err(CoreError::Database("Invalid part hash length in DB".into()));
        }
        let mut h = [0u8; 16];
        h.copy_from_slice(&blob);
        hashes.push(h);
    }
    Ok(hashes)
}

/// Deletes the stored per-part hashset for a file.
pub fn delete_part_hashes(conn: &mut Connection, file_hash: &str) -> Result<(), CoreError> {
    conn.execute(
        "DELETE FROM file_part_hashes WHERE file_hash = ?1",
        params![file_hash],
    )?;
    Ok(())
}

pub fn update_status(
    conn: &mut Connection,
    download_id: &Uuid,
    status: DownloadStatus,
) -> Result<(), CoreError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    conn.execute(
        "UPDATE downloads SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status.as_str(), now, download_id.to_string()],
    )?;
    Ok(())
}

pub fn get_all_downloads(conn: &Connection) -> Result<Vec<Download>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, display_name, target_path, temp_path, total_size, ed2k_hash, aich_root, status, created_at, updated_at, category_id, priority FROM downloads ORDER BY created_at ASC",
    )?;
    let mut rows = stmt.query([])?;
    let mut downloads = Vec::new();
    while let Some(row) = rows.next()? {
        let id_str: String = row.get(0)?;
        let id = Uuid::parse_str(&id_str)
            .map_err(|e| CoreError::Database(format!("Invalid UUID in database: {e}")))?;
        let display_name: String = row.get(1)?;
        let target_path: String = row.get(2)?;
        let temp_path: String = row.get(3)?;
        let total_size: i64 = row.get(4)?;
        let ed2k_blob: Vec<u8> = row.get(5)?;
        let aich_blob: Option<Vec<u8>> = row.get(6)?;
        let status_str: String = row.get(7)?;
        let created_at: i64 = row.get(8)?;
        let updated_at: i64 = row.get(9)?;
        let category_id: Option<String> = row.get(10)?;
        let priority: String = row.get(11).unwrap_or_else(|_| "normal".to_string());

        if ed2k_blob.len() != 16 {
            return Err(CoreError::Database("Invalid ed2k_hash length in DB".into()));
        }
        let mut ed2k_hash_bytes = [0u8; 16];
        ed2k_hash_bytes.copy_from_slice(&ed2k_blob);

        let aich_root = if let Some(b) = aich_blob {
            if b.len() == 20 {
                let mut aich_bytes = [0u8; 20];
                aich_bytes.copy_from_slice(&b);
                Some(mule_format::AichHash(aich_bytes))
            } else {
                None
            }
        } else {
            None
        };

        let status = status_str.parse().unwrap_or(DownloadStatus::Queued);

        downloads.push(Download {
            id,
            display_name,
            target_path: std::path::PathBuf::from(target_path),
            temp_path: std::path::PathBuf::from(temp_path),
            total_size: total_size as u64,
            ed2k_hash: mule_format::Ed2kHash(ed2k_hash_bytes),
            aich_root,
            status,
            created_at,
            updated_at,
            category_id,
            priority,
        });
    }
    Ok(downloads)
}

pub fn get_download(conn: &Connection, id: &Uuid) -> Result<Option<Download>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, display_name, target_path, temp_path, total_size, ed2k_hash, aich_root, status, created_at, updated_at, category_id, priority FROM downloads WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id.to_string()])?;
    if let Some(row) = rows.next()? {
        let id_str: String = row.get(0)?;
        let id = Uuid::parse_str(&id_str)
            .map_err(|e| CoreError::Database(format!("Invalid UUID in database: {e}")))?;
        let display_name: String = row.get(1)?;
        let target_path: String = row.get(2)?;
        let temp_path: String = row.get(3)?;
        let total_size: i64 = row.get(4)?;
        let ed2k_blob: Vec<u8> = row.get(5)?;
        let aich_blob: Option<Vec<u8>> = row.get(6)?;
        let status_str: String = row.get(7)?;
        let created_at: i64 = row.get(8)?;
        let updated_at: i64 = row.get(9)?;
        let category_id: Option<String> = row.get(10)?;
        let priority: String = row.get(11).unwrap_or_else(|_| "normal".to_string());

        if ed2k_blob.len() != 16 {
            return Err(CoreError::Database("Invalid ed2k_hash length in DB".into()));
        }
        let mut ed2k_hash_bytes = [0u8; 16];
        ed2k_hash_bytes.copy_from_slice(&ed2k_blob);

        let aich_root = if let Some(b) = aich_blob {
            if b.len() == 20 {
                let mut aich_bytes = [0u8; 20];
                aich_bytes.copy_from_slice(&b);
                Some(mule_format::AichHash(aich_bytes))
            } else {
                None
            }
        } else {
            None
        };

        let status = status_str.parse().unwrap_or(DownloadStatus::Queued);

        Ok(Some(Download {
            id,
            display_name,
            target_path: std::path::PathBuf::from(target_path),
            temp_path: std::path::PathBuf::from(temp_path),
            total_size: total_size as u64,
            ed2k_hash: mule_format::Ed2kHash(ed2k_hash_bytes),
            aich_root,
            status,
            created_at,
            updated_at,
            category_id,
            priority,
        }))
    } else {
        Ok(None)
    }
}

pub fn delete_download(conn: &mut Connection, download_id: &Uuid) -> Result<bool, CoreError> {
    let tx = conn.transaction()?;
    let id_str = download_id.to_string();
    tx.execute(
        "DELETE FROM received_ranges WHERE download_id = ?1",
        params![&id_str],
    )?;
    tx.execute(
        "DELETE FROM verified_parts WHERE download_id = ?1",
        params![&id_str],
    )?;
    let rows_deleted = tx.execute("DELETE FROM downloads WHERE id = ?1", params![&id_str])?;
    tx.commit()?;
    Ok(rows_deleted > 0)
}

pub fn update_target_path(
    conn: &mut Connection,
    download_id: &Uuid,
    target_path: &Path,
) -> Result<(), CoreError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    conn.execute(
        "UPDATE downloads SET target_path = ?1, updated_at = ?2 WHERE id = ?3",
        params![
            target_path.to_str().unwrap_or_default(),
            now,
            download_id.to_string()
        ],
    )?;
    Ok(())
}
