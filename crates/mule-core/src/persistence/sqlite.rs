use rusqlite::{params, Connection};
use std::path::Path;
use uuid::Uuid;

use crate::error::CoreError;
use crate::model::{
    Download, DownloadCategory, DownloadStatus, Ed2kServer, HistoryEntry, HistoryFilter,
    HistoryStats, ServerFilterStatus,
};
use crate::ranges::RangeSet;

/// Database persistence manager for mule-core.
pub struct Database {
    conn: Connection,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database").finish_non_exhaustive()
    }
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

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS download_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                download_id TEXT NOT NULL,
                file_name TEXT NOT NULL,
                total_size INTEGER NOT NULL,
                transferred_bytes INTEGER NOT NULL,
                ed2k_hash TEXT NOT NULL,
                event_type TEXT NOT NULL,
                status TEXT NOT NULL,
                duration_secs INTEGER NOT NULL,
                connection_attempts INTEGER NOT NULL,
                resume_count INTEGER NOT NULL,
                verification_result TEXT NOT NULL,
                error_message TEXT,
                timestamp_unix_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_history_timestamp ON download_history(timestamp_unix_ms DESC);
            CREATE INDEX IF NOT EXISTS idx_history_event_type ON download_history(event_type);
            CREATE INDEX IF NOT EXISTS idx_history_download_id ON download_history(download_id);

            CREATE TABLE IF NOT EXISTS download_categories (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                color TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                default_destination TEXT,
                default_priority TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_categories_order ON download_categories(sort_order ASC);

            CREATE TABLE IF NOT EXISTS ed2k_servers (
                address TEXT PRIMARY KEY,
                ip TEXT NOT NULL,
                port INTEGER NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                ping INTEGER NOT NULL DEFAULT 0,
                preference INTEGER NOT NULL DEFAULT 0,
                failed_count INTEGER NOT NULL DEFAULT 0,
                dyn_ip TEXT,
                version TEXT,
                filter_status TEXT NOT NULL DEFAULT 'allowed',
                last_seen_unix_ms INTEGER,
                created_at_unix_ms INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_servers_filter_status ON ed2k_servers(filter_status);
            ",
        )?;

        // Ensure category_id exists in downloads table (migration)
        let mut stmt = self.conn.prepare("PRAGMA table_info(downloads)")?;
        let mut rows = stmt.query([])?;
        let mut has_category_id = false;
        while let Some(row) = rows.next()? {
            let col_name: String = row.get(1)?;
            if col_name == "category_id" {
                has_category_id = true;
                break;
            }
        }
        if !has_category_id {
            self.conn
                .execute_batch("ALTER TABLE downloads ADD COLUMN category_id TEXT;")?;
        }

        Ok(())
    }

    /// Saves a new download record.
    pub fn insert_download(&mut self, dl: &Download) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO downloads (id, display_name, target_path, temp_path, total_size, ed2k_hash, aich_root, status, created_at, updated_at, category_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
            ],
        )?;
        Ok(())
    }

    /// Atomically replaces received ranges for a download in a single transaction.
    pub fn sync_received_ranges(
        &mut self,
        download_id: &Uuid,
        ranges: &RangeSet,
    ) -> Result<(), CoreError> {
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
    pub fn mark_part_verified(
        &mut self,
        download_id: &Uuid,
        part_index: u32,
    ) -> Result<(), CoreError> {
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
    pub fn unmark_part_verified(
        &mut self,
        download_id: &Uuid,
        part_index: u32,
    ) -> Result<(), CoreError> {
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
    pub fn update_status(
        &mut self,
        download_id: &Uuid,
        status: DownloadStatus,
    ) -> Result<(), CoreError> {
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

    /// Retrieves all persisted downloads from the database.
    pub fn get_all_downloads(&self) -> Result<Vec<Download>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, display_name, target_path, temp_path, total_size, ed2k_hash, aich_root, status, created_at, updated_at, category_id FROM downloads ORDER BY created_at ASC",
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
            });
        }
        Ok(downloads)
    }

    /// Deletes a download record and its associated ranges and verified parts.
    pub fn delete_download(&mut self, download_id: &Uuid) -> Result<bool, CoreError> {
        let tx = self.conn.transaction()?;
        let id_str = download_id.to_string();
        tx.execute(
            "DELETE FROM received_ranges WHERE download_id = ?1",
            params![&id_str],
        )?;
        tx.execute(
            "DELETE FROM verified_parts WHERE download_id = ?1",
            params![&id_str],
        )?;
        let rows_deleted = tx.execute(
            "DELETE FROM downloads WHERE id = ?1",
            params![&id_str],
        )?;
        tx.commit()?;
        Ok(rows_deleted > 0)
    }

    /// Gets a setting value by key.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, CoreError> {
        let mut stmt = self.conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            let val: String = row.get(0)?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    /// Sets or replaces a setting value.
    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Removes a setting by key.
    pub fn remove_setting(&mut self, key: &str) -> Result<bool, CoreError> {
        let rows = self.conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(rows > 0)
    }

    /// Updates the target finalization path of a download.
    pub fn update_target_path(
        &mut self,
        download_id: &Uuid,
        target_path: &Path,
    ) -> Result<(), CoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "UPDATE downloads SET target_path = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                target_path.to_str().unwrap_or_default(),
                now,
                download_id.to_string()
            ],
        )?;
        Ok(())
    }

    /// Inserts a new history record and returns the generated row ID.
    pub fn insert_history_entry(&mut self, entry: &HistoryEntry) -> Result<i64, CoreError> {
        self.conn.execute(
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
        Ok(self.conn.last_insert_rowid())
    }

    /// Queries history entries matching the given filter.
    pub fn query_history(&self, filter: &HistoryFilter) -> Result<Vec<HistoryEntry>, CoreError> {
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

        let mut stmt = self.conn.prepare(&sql)?;
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

    /// Aggregates historical statistics across time windows (Session, 24h, 7d, Lifetime).
    pub fn get_history_stats(&self, session_start_unix_ms: i64) -> Result<HistoryStats, CoreError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let day_24h_ms = now_ms - (86_400 * 1000);
        let day_7d_ms = now_ms - (7 * 86_400 * 1000);

        let mut stats = HistoryStats::default();

        // 1. Lifetime completed count & bytes
        let (tot_comp_cnt, tot_comp_bytes): (i64, i64) = self
            .conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(total_size), 0) FROM download_history WHERE event_type = 'completed'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((0, 0));
        stats.total_completed_count = tot_comp_cnt.max(0) as u64;
        stats.total_completed_bytes = tot_comp_bytes.max(0) as u64;

        // 2. Lifetime failures
        let tot_fail: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM download_history WHERE event_type = 'failed'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        stats.total_failed_count = tot_fail.max(0) as u64;

        // 3. Lifetime resumes
        let tot_resumes: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM download_history WHERE event_type = 'resumed'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        stats.total_resumes_count = tot_resumes.max(0) as u64;

        // 4. Lifetime integrity failures
        let tot_integrity_fails: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM download_history WHERE verification_result = 'hash_mismatch'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        stats.total_integrity_failures = tot_integrity_fails.max(0) as u64;

        // 5. Session window
        let (sess_cnt, sess_bytes): (i64, i64) = self
            .conn
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
        let (d24_cnt, d24_bytes): (i64, i64) = self
            .conn
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
        let (d7_cnt, d7_bytes): (i64, i64) = self
            .conn
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

    /// Deletes all history records.
    pub fn clear_history(&mut self) -> Result<usize, CoreError> {
        let rows = self.conn.execute("DELETE FROM download_history", [])?;
        Ok(rows)
    }

    /// Prunes history records older than the specified retention days.
    /// If retention_days is 0, retention is unlimited and no rows are deleted.
    pub fn apply_retention_policy(&mut self, retention_days: u32) -> Result<usize, CoreError> {
        if retention_days == 0 {
            return Ok(0);
        }
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let cutoff_ms = now_ms - (retention_days as i64 * 86_400 * 1000);
        let rows = self.conn.execute(
            "DELETE FROM download_history WHERE timestamp_unix_ms < ?1",
            params![cutoff_ms],
        )?;
        Ok(rows)
    }

    /// Retrieves all categories ordered by sort_order ascending, then created_at ascending.
    pub fn get_categories(&self) -> Result<Vec<DownloadCategory>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, color, sort_order, default_destination, default_priority, created_at, updated_at
             FROM download_categories ORDER BY sort_order ASC, created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(DownloadCategory {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                sort_order: row.get(3)?,
                default_destination: row.get(4)?,
                default_priority: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Retrieves a category by its ID.
    pub fn get_category(&self, id: &str) -> Result<Option<DownloadCategory>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, color, sort_order, default_destination, default_priority, created_at, updated_at
             FROM download_categories WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(DownloadCategory {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                sort_order: row.get(3)?,
                default_destination: row.get(4)?,
                default_priority: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Inserts a new category.
    pub fn insert_category(&mut self, cat: &DownloadCategory) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO download_categories (id, name, color, sort_order, default_destination, default_priority, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                cat.id,
                cat.name,
                cat.color,
                cat.sort_order,
                cat.default_destination,
                cat.default_priority,
                cat.created_at,
                cat.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Updates an existing category.
    pub fn update_category(&mut self, cat: &DownloadCategory) -> Result<bool, CoreError> {
        let rows = self.conn.execute(
            "UPDATE download_categories
             SET name = ?1, color = ?2, sort_order = ?3, default_destination = ?4, default_priority = ?5, updated_at = ?6
             WHERE id = ?7",
            params![
                cat.name,
                cat.color,
                cat.sort_order,
                cat.default_destination,
                cat.default_priority,
                cat.updated_at,
                cat.id,
            ],
        )?;
        Ok(rows > 0)
    }

    /// Deletes a category and atomically reassigns all affected downloads to NULL.
    /// Returns the number of reassigned downloads.
    /// Invariant: NEVER deletes any downloads or download files.
    pub fn delete_category(&mut self, id: &str) -> Result<usize, CoreError> {
        let tx = self.conn.transaction()?;
        let reassigned = tx.execute(
            "UPDATE downloads SET category_id = NULL WHERE category_id = ?1",
            params![id],
        )?;
        tx.execute(
            "DELETE FROM download_categories WHERE id = ?1",
            params![id],
        )?;
        tx.commit()?;
        Ok(reassigned)
    }

    /// Reorders categories based on the provided ordered list of category IDs.
    pub fn reorder_categories(&mut self, category_ids: &[String]) -> Result<(), CoreError> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE download_categories SET sort_order = ?1 WHERE id = ?2")?;
            for (index, id) in category_ids.iter().enumerate() {
                stmt.execute(params![index as i32, id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Sets or clears the category assigned to a download.
    pub fn set_download_category(
        &mut self,
        download_id: &Uuid,
        category_id: Option<&str>,
    ) -> Result<bool, CoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let rows = self.conn.execute(
            "UPDATE downloads SET category_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![category_id, now, download_id.to_string()],
        )?;
        Ok(rows > 0)
    }

    /// Upserts a batch of eD2K servers.
    /// If an address already exists, updates metadata while preserving its manual enabled/disabled status if previously disabled.
    pub fn upsert_servers(&mut self, servers: &[Ed2kServer]) -> Result<usize, CoreError> {
        let tx = self.conn.transaction()?;
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

    /// Lists all persisted eD2K servers ordered by preference DESC, name ASC.
    pub fn list_servers(&self) -> Result<Vec<Ed2kServer>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT address, ip, port, name, description, ping, preference, failed_count,
                    dyn_ip, version, filter_status, last_seen_unix_ms, created_at_unix_ms, updated_at_unix_ms
             FROM ed2k_servers
             ORDER BY preference DESC, name ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            let filter_status_str: String = row.get(10)?;
            let filter_status: ServerFilterStatus = filter_status_str.parse().unwrap_or(ServerFilterStatus::Allowed);

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

    /// Updates the enabled/disabled state of a server.
    /// If enabled is true, sets status to 'allowed'; if false, sets to 'disabled'.
    pub fn set_server_enabled(&mut self, address: &str, enabled: bool) -> Result<bool, CoreError> {
        let new_status = if enabled {
            ServerFilterStatus::Allowed.as_str()
        } else {
            ServerFilterStatus::Disabled.as_str()
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let rows = self.conn.execute(
            "UPDATE ed2k_servers SET filter_status = ?1, updated_at_unix_ms = ?2 WHERE address = ?3",
            params![new_status, now, address],
        )?;
        Ok(rows > 0)
    }

    /// Deletes a server from SQLite by address ("ip:port").
    pub fn delete_server(&mut self, address: &str) -> Result<bool, CoreError> {
        let rows = self.conn.execute(
            "DELETE FROM ed2k_servers WHERE address = ?1",
            params![address],
        )?;
        Ok(rows > 0)
    }

    /// Removes all servers from SQLite.
    pub fn clear_servers(&mut self) -> Result<usize, CoreError> {
        let rows = self.conn.execute("DELETE FROM ed2k_servers", [])?;
        Ok(rows)
    }
}
