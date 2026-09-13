use rusqlite::{params, Connection};
use std::path::Path;
use uuid::Uuid;

use crate::error::CoreError;
use crate::model::{Download, DownloadStatus};
use crate::ranges::RangeSet;

/// Database persistence manager for mule-core.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens or creates a SQLite database in WAL mode.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, CoreError> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for high performance and durability against crashes
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// In-memory database helper for tests.
    pub fn open_in_memory() -> Result<Self, CoreError> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), CoreError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS downloads (
                id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                target_path TEXT NOT NULL,
                temp_path TEXT NOT NULL,
                total_size INTEGER NOT NULL,
                ed2k_hash BLOB NOT NULL,
                aich_root BLOB,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS received_ranges (
                download_id TEXT NOT NULL,
                start_offset INTEGER NOT NULL,
                end_offset INTEGER NOT NULL,
                PRIMARY KEY(download_id, start_offset)
            );

            CREATE TABLE IF NOT EXISTS verified_parts (
                download_id TEXT NOT NULL,
                part_index INTEGER NOT NULL,
                verified_at INTEGER NOT NULL,
                PRIMARY KEY(download_id, part_index)
            );
            ",
        )?;
        Ok(())
    }

    /// Saves a new download record.
    pub fn insert_download(&mut self, dl: &Download) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO downloads (id, display_name, target_path, temp_path, total_size, ed2k_hash, aich_root, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
            ],
        )?;
        Ok(())
    }

    /// Atomically replaces received ranges for a download in a single transaction.
    pub fn sync_received_ranges(&mut self, download_id: &Uuid, ranges: &RangeSet) -> Result<(), CoreError> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM received_ranges WHERE download_id = ?1",
            params![download_id.to_string()],
        )?;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO received_ranges (download_id, start_offset, end_offset) VALUES (?1, ?2, ?3)",
            )?;
            for r in ranges.ranges() {
                stmt.execute(params![download_id.to_string(), r.start as i64, r.end as i64])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Loads the stored RangeSet for a download.
    pub fn load_received_ranges(&self, download_id: &Uuid) -> Result<RangeSet, CoreError> {
        let mut stmt = self.conn.prepare(
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

    /// Records that a part has been verified.
    pub fn mark_part_verified(&mut self, download_id: &Uuid, part_index: u32) -> Result<(), CoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT OR REPLACE INTO verified_parts (download_id, part_index, verified_at) VALUES (?1, ?2, ?3)",
            params![download_id.to_string(), part_index as i64, now],
        )?;
        Ok(())
    }

    /// Unmarks a part upon corruption detection.
    pub fn unmark_part_verified(&mut self, download_id: &Uuid, part_index: u32) -> Result<(), CoreError> {
        self.conn.execute(
            "DELETE FROM verified_parts WHERE download_id = ?1 AND part_index = ?2",
            params![download_id.to_string(), part_index as i64],
        )?;
        Ok(())
    }

    /// Retrieves all verified part indexes.
    pub fn get_verified_parts(&self, download_id: &Uuid) -> Result<Vec<u32>, CoreError> {
        let mut stmt = self.conn.prepare(
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

    /// Updates status of a download.
    pub fn update_status(&mut self, download_id: &Uuid, status: DownloadStatus) -> Result<(), CoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "UPDATE downloads SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.as_str(), now, download_id.to_string()],
        )?;
        Ok(())
    }
}
