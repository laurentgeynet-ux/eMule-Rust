use rusqlite::{params, Connection};
use std::path::Path;
use uuid::Uuid;

use crate::error::CoreError;
use crate::model::{
    Download, DownloadCategory, DownloadStatus, Ed2kServer, HistoryEntry, HistoryFilter,
    HistoryStats, PeerCredit, PeerSource, PeerSourceOrigin, PeerSourceStatus, ServerFilterStatus,
    SharedFile,
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

            CREATE TABLE IF NOT EXISTS peer_sources (
                file_hash TEXT NOT NULL,
                address TEXT NOT NULL,
                ip INTEGER NOT NULL,
                port INTEGER NOT NULL,
                origin TEXT NOT NULL,
                discovered_at_unix_ms INTEGER NOT NULL,
                last_seen_unix_ms INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'candidate',
                fail_count INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                cooldown_until_unix_ms INTEGER,
                user_hash TEXT,
                crypt_options INTEGER DEFAULT 0,
                PRIMARY KEY(file_hash, address)
            );

            CREATE TABLE IF NOT EXISTS shared_files (
                file_hash TEXT PRIMARY KEY,
                file_name TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                file_path TEXT NOT NULL,
                shared_at_unix_ms INTEGER NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1
            );

            CREATE INDEX IF NOT EXISTS idx_shared_files_enabled ON shared_files(enabled);

            CREATE TABLE IF NOT EXISTS peer_credits (
                pubkey_hash TEXT PRIMARY KEY,
                pubkey_hex TEXT NOT NULL,
                uploaded_bytes INTEGER NOT NULL DEFAULT 0,
                downloaded_bytes INTEGER NOT NULL DEFAULT 0,
                first_seen_unix_ms INTEGER NOT NULL,
                last_seen_unix_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS kad_contacts (
                kad_id TEXT PRIMARY KEY,
                ip TEXT NOT NULL,
                udp_port INTEGER NOT NULL,
                tcp_port INTEGER,
                version INTEGER NOT NULL DEFAULT 0,
                verified INTEGER NOT NULL DEFAULT 0,
                fail_count INTEGER NOT NULL DEFAULT 0,
                first_seen_unix_ms INTEGER NOT NULL,
                last_seen_unix_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_kad_contacts_last_seen ON kad_contacts(last_seen_unix_ms DESC);
            CREATE INDEX IF NOT EXISTS idx_kad_contacts_verified ON kad_contacts(verified);

            CREATE TABLE IF NOT EXISTS kad_sources_index (
                file_hash TEXT NOT NULL,
                source_kad_id TEXT NOT NULL,
                ip TEXT NOT NULL,
                udp_port INTEGER NOT NULL,
                tcp_port INTEGER NOT NULL,
                file_size INTEGER NOT NULL,
                encryption INTEGER NOT NULL DEFAULT 0,
                expires_at_unix_ms INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL,
                PRIMARY KEY(file_hash, source_kad_id, ip, tcp_port)
            );

            CREATE INDEX IF NOT EXISTS idx_kad_sources_file_hash ON kad_sources_index(file_hash);
            CREATE INDEX IF NOT EXISTS idx_kad_sources_expires ON kad_sources_index(expires_at_unix_ms);

            CREATE TABLE IF NOT EXISTS kad_keywords_index (
                keyword_hash TEXT NOT NULL,
                file_hash TEXT NOT NULL,
                file_name TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                source_count INTEGER NOT NULL DEFAULT 1,
                discovered_at_unix_ms INTEGER NOT NULL,
                expires_at_unix_ms INTEGER NOT NULL,
                PRIMARY KEY(keyword_hash, file_hash)
            );

            CREATE INDEX IF NOT EXISTS idx_kad_keywords_lookup ON kad_keywords_index(keyword_hash);
            CREATE INDEX IF NOT EXISTS idx_kad_keywords_expires ON kad_keywords_index(expires_at_unix_ms);
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

        // Ensure cooldown_until_unix_ms exists in peer_sources table (migration)
        let mut stmt_peer = self.conn.prepare("PRAGMA table_info(peer_sources)")?;
        let mut rows_peer = stmt_peer.query([])?;
        let mut has_cooldown = false;
        while let Some(row) = rows_peer.next()? {
            let col_name: String = row.get(1)?;
            if col_name == "cooldown_until_unix_ms" {
                has_cooldown = true;
                break;
            }
        }
        if !has_cooldown {
            self.conn
                .execute_batch("ALTER TABLE peer_sources ADD COLUMN cooldown_until_unix_ms INTEGER;")?;
        }

        // Ensure user_hash and crypt_options exist in peer_sources table (migration)
        let mut stmt_peer2 = self.conn.prepare("PRAGMA table_info(peer_sources)")?;
        let mut rows_peer2 = stmt_peer2.query([])?;
        let mut has_user_hash = false;
        let mut has_crypt_options = false;
        while let Some(row) = rows_peer2.next()? {
            let col_name: String = row.get(1)?;
            if col_name == "user_hash" {
                has_user_hash = true;
            } else if col_name == "crypt_options" {
                has_crypt_options = true;
            }
        }
        if !has_user_hash {
            self.conn
                .execute_batch("ALTER TABLE peer_sources ADD COLUMN user_hash TEXT;")?;
        }
        if !has_crypt_options {
            self.conn
                .execute_batch("ALTER TABLE peer_sources ADD COLUMN crypt_options INTEGER DEFAULT 0;")?;
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

    // ── Peer Sources Registry ───────────────────────────────────────────────

    /// Upserts a batch of peer sources for a given file hash.
    /// Deduplicates on (file_hash, address). If existing, preserves fail_count, status and updates last_seen_unix_ms.
    pub fn upsert_peer_sources(
        &mut self,
        file_hash: &str,
        sources: &[PeerSource],
    ) -> Result<usize, CoreError> {
        let tx = self.conn.transaction()?;
        let mut count = 0;
        {
            let mut insert_stmt = tx.prepare(
                "INSERT INTO peer_sources (
                    file_hash, address, ip, port, origin,
                    discovered_at_unix_ms, last_seen_unix_ms, status, fail_count, last_error, cooldown_until_unix_ms,
                    user_hash, crypt_options
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                ON CONFLICT(file_hash, address) DO UPDATE SET
                    last_seen_unix_ms = excluded.last_seen_unix_ms,
                    ip = excluded.ip,
                    port = excluded.port,
                    user_hash = COALESCE(excluded.user_hash, peer_sources.user_hash),
                    crypt_options = CASE WHEN excluded.crypt_options != 0 THEN excluded.crypt_options ELSE peer_sources.crypt_options END"
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
                ])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Lists all persisted peer sources for a given file hash, ordered by status ASC, last_seen_unix_ms DESC.
    pub fn list_peer_sources(&self, file_hash: &str) -> Result<Vec<PeerSource>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT file_hash, address, ip, port, origin,
                    discovered_at_unix_ms, last_seen_unix_ms, status, fail_count, last_error, cooldown_until_unix_ms,
                    user_hash, crypt_options
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
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Updates the status, fail_count, last_error and optional cooldown of a peer source.
    pub fn update_peer_source_status(
        &mut self,
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
            self.conn.execute(
                "UPDATE peer_sources SET status = ?1, fail_count = fail_count + 1, last_error = ?2, last_seen_unix_ms = ?3, cooldown_until_unix_ms = ?4
                 WHERE file_hash = ?5 AND address = ?6",
                params![status.as_str(), error, now, cooldown_until_unix_ms, file_hash, address],
            )?
        } else {
            self.conn.execute(
                "UPDATE peer_sources SET status = ?1, last_error = ?2, last_seen_unix_ms = ?3, cooldown_until_unix_ms = ?4
                 WHERE file_hash = ?5 AND address = ?6",
                params![status.as_str(), error, now, cooldown_until_unix_ms, file_hash, address],
            )?
        };
        Ok(rows > 0)
    }

    /// Resets the cooldown and fail count for a peer source (e.g. manual retry by user).
    pub fn reset_peer_source_cooldown(
        &mut self,
        file_hash: &str,
        address: &str,
    ) -> Result<bool, CoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let rows = self.conn.execute(
            "UPDATE peer_sources SET cooldown_until_unix_ms = NULL, status = 'allowed', last_seen_unix_ms = ?1
             WHERE file_hash = ?2 AND address = ?3",
            params![now, file_hash, address],
        )?;
        Ok(rows > 0)
    }

    /// Deletes a specific peer source from SQLite.
    pub fn delete_peer_source(&mut self, file_hash: &str, address: &str) -> Result<bool, CoreError> {
        let rows = self.conn.execute(
            "DELETE FROM peer_sources WHERE file_hash = ?1 AND address = ?2",
            params![file_hash, address],
        )?;
        Ok(rows > 0)
    }

    /// Clears all peer sources for a file hash.
    pub fn clear_peer_sources(&mut self, file_hash: &str) -> Result<usize, CoreError> {
        let rows = self.conn.execute(
            "DELETE FROM peer_sources WHERE file_hash = ?1",
            params![file_hash],
        )?;
        Ok(rows)
    }

    /// Upserts a shared file in SQLite.
    pub fn upsert_shared_file(&mut self, file: &crate::model::SharedFile) -> Result<(), CoreError> {
        self.conn.execute(
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

    /// Lists all shared files in SQLite.
    pub fn list_shared_files(&self) -> Result<Vec<crate::model::SharedFile>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT file_hash, file_name, file_size, file_path, shared_at_unix_ms, enabled
             FROM shared_files
             ORDER BY shared_at_unix_ms DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let size_i64: i64 = row.get(2)?;
            let enabled_int: i32 = row.get(5)?;
            Ok(crate::model::SharedFile {
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

    /// Retrieves a specific shared file by hash.
    pub fn get_shared_file(&self, file_hash: &str) -> Result<Option<crate::model::SharedFile>, CoreError> {
        let mut stmt = self.conn.prepare(
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

    /// Deletes a shared file entry from SQLite.
    pub fn delete_shared_file(&mut self, file_hash: &str) -> Result<bool, CoreError> {
        let rows = self.conn.execute(
            "DELETE FROM shared_files WHERE file_hash = ?1",
            params![file_hash],
        )?;
        Ok(rows > 0)
    }

    /// Toggles the enabled state of a shared file.
    pub fn set_shared_file_enabled(&mut self, file_hash: &str, enabled: bool) -> Result<bool, CoreError> {
        let rows = self.conn.execute(
            "UPDATE shared_files SET enabled = ?1 WHERE file_hash = ?2",
            params![if enabled { 1 } else { 0 }, file_hash],
        )?;
        Ok(rows > 0)
    }

    /// Upserts a peer credit record into SQLite.
    pub fn upsert_peer_credit(&mut self, credit: &PeerCredit) -> Result<(), CoreError> {
        self.conn.execute(
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

    /// Retrieves a peer credit record by public key hash.
    pub fn get_peer_credit(&self, pubkey_hash: &str) -> Result<Option<PeerCredit>, CoreError> {
        let mut stmt = self.conn.prepare(
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

    /// Increments uploaded bytes counter for a peer credit record.
    pub fn record_credit_upload(&mut self, pubkey_hash: &str, pubkey_hex: &str, bytes: u64) -> Result<(), CoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        self.conn.execute(
            "INSERT INTO peer_credits (pubkey_hash, pubkey_hex, uploaded_bytes, downloaded_bytes, first_seen_unix_ms, last_seen_unix_ms)
             VALUES (?1, ?2, ?3, 0, ?4, ?4)
             ON CONFLICT(pubkey_hash) DO UPDATE SET
                uploaded_bytes = uploaded_bytes + ?3,
                last_seen_unix_ms = ?4",
            params![pubkey_hash, pubkey_hex, bytes as i64, now],
        )?;
        Ok(())
    }

    /// Increments downloaded bytes counter for a peer credit record.
    pub fn record_credit_download(&mut self, pubkey_hash: &str, pubkey_hex: &str, bytes: u64) -> Result<(), CoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        self.conn.execute(
            "INSERT INTO peer_credits (pubkey_hash, pubkey_hex, uploaded_bytes, downloaded_bytes, first_seen_unix_ms, last_seen_unix_ms)
             VALUES (?1, ?2, 0, ?3, ?4, ?4)
             ON CONFLICT(pubkey_hash) DO UPDATE SET
                downloaded_bytes = downloaded_bytes + ?3,
                last_seen_unix_ms = ?4",
            params![pubkey_hash, pubkey_hex, bytes as i64, now],
        )?;
        Ok(())
    }

    /// Lists top peer credits.
    pub fn list_peer_credits(&self, limit: usize) -> Result<Vec<PeerCredit>, CoreError> {
        let mut stmt = self.conn.prepare(
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

    /// Upserts a Kad contact record.
    pub fn upsert_kad_contact(&mut self, contact: &crate::kad::KadContact) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO kad_contacts (
                kad_id, ip, udp_port, tcp_port, version, verified, fail_count, first_seen_unix_ms, last_seen_unix_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(kad_id) DO UPDATE SET
                ip = ?2,
                udp_port = ?3,
                tcp_port = ?4,
                version = ?5,
                verified = CASE WHEN ?6 = 1 THEN 1 ELSE verified END,
                fail_count = ?7,
                last_seen_unix_ms = ?9",
            params![
                contact.kad_id.to_hex(),
                contact.ip.to_string(),
                contact.udp_port as i64,
                contact.tcp_port.map(|p| p as i64),
                contact.version as i64,
                if contact.verified { 1 } else { 0 },
                contact.fail_count as i64,
                contact.first_seen_unix_ms as i64,
                contact.last_seen_unix_ms as i64,
            ],
        )?;
        Ok(())
    }

    /// Batch upserts Kad contacts in a single transaction.
    pub fn batch_upsert_kad_contacts(&mut self, contacts: &[crate::kad::KadContact]) -> Result<usize, CoreError> {
        let tx = self.conn.transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO kad_contacts (
                    kad_id, ip, udp_port, tcp_port, version, verified, fail_count, first_seen_unix_ms, last_seen_unix_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(kad_id) DO UPDATE SET
                    ip = ?2,
                    udp_port = ?3,
                    tcp_port = ?4,
                    version = ?5,
                    verified = CASE WHEN ?6 = 1 THEN 1 ELSE verified END,
                    fail_count = ?7,
                    last_seen_unix_ms = ?9",
            )?;

            for contact in contacts {
                stmt.execute(params![
                    contact.kad_id.to_hex(),
                    contact.ip.to_string(),
                    contact.udp_port as i64,
                    contact.tcp_port.map(|p| p as i64),
                    contact.version as i64,
                    if contact.verified { 1 } else { 0 },
                    contact.fail_count as i64,
                    contact.first_seen_unix_ms as i64,
                    contact.last_seen_unix_ms as i64,
                ])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Retrieves all Kad contacts up to a given limit, ordered by last_seen_unix_ms DESC.
    pub fn list_kad_contacts(&self, limit: usize) -> Result<Vec<crate::kad::KadContact>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT kad_id, ip, udp_port, tcp_port, version, verified, fail_count, first_seen_unix_ms, last_seen_unix_ms
             FROM kad_contacts
             ORDER BY last_seen_unix_ms DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let kad_id_str: String = row.get(0)?;
            let kad_id = crate::kad::KadId::from_hex(&kad_id_str).unwrap_or(crate::kad::KadId::ZERO);
            let ip_str: String = row.get(1)?;
            let ip: std::net::Ipv4Addr = ip_str.parse().unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);
            let udp_port: i64 = row.get(2)?;
            let tcp_port: Option<i64> = row.get(3)?;
            let version: i64 = row.get(4)?;
            let verified: i64 = row.get(5)?;
            let fail_count: i64 = row.get(6)?;
            let first_seen: i64 = row.get(7)?;
            let last_seen: i64 = row.get(8)?;

            Ok(crate::kad::KadContact {
                kad_id,
                ip,
                udp_port: udp_port.max(0) as u16,
                tcp_port: tcp_port.map(|p| p.max(0) as u16),
                version: version.max(0) as u8,
                verified: verified != 0,
                fail_count: fail_count.max(0) as u32,
                first_seen_unix_ms: first_seen.max(0) as u64,
                last_seen_unix_ms: last_seen.max(0) as u64,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Counts total Kad contacts stored in SQLite.
    pub fn count_kad_contacts(&self) -> Result<usize, CoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM kad_contacts",
            [],
            |r| r.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    /// Deletes all Kad contacts.
    pub fn clear_kad_contacts(&mut self) -> Result<usize, CoreError> {
        let rows = self.conn.execute("DELETE FROM kad_contacts", [])?;
        Ok(rows)
    }

    /// Inserts or replaces a batch of Kad indexed sources.
    pub fn insert_kad_indexed_sources(
        &mut self,
        sources: &[crate::kad::KadIndexedSource],
    ) -> Result<usize, CoreError> {
        if sources.is_empty() {
            return Ok(0);
        }
        let tx = self.conn.transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO kad_sources_index (
                    file_hash, source_kad_id, ip, udp_port, tcp_port, file_size,
                    encryption, expires_at_unix_ms, updated_at_unix_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(file_hash, source_kad_id, ip, tcp_port) DO UPDATE SET
                    udp_port = ?4,
                    file_size = ?6,
                    encryption = ?7,
                    expires_at_unix_ms = ?8,
                    updated_at_unix_ms = ?9",
            )?;

            for s in sources {
                let file_hash_hex = crate::kad::KadId(s.file_hash).to_hex();
                stmt.execute(params![
                    file_hash_hex,
                    s.source_kad_id.to_hex(),
                    s.ip.to_string(),
                    s.udp_port as i64,
                    s.tcp_port as i64,
                    s.file_size as i64,
                    s.encryption as i64,
                    s.expires_at_unix_ms as i64,
                    s.updated_at_unix_ms as i64,
                ])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Retrieves unexpired indexed Kad sources for a given file hash.
    pub fn get_kad_indexed_sources(
        &self,
        file_hash: &[u8; 16],
        max_results: usize,
        now_unix_ms: u64,
    ) -> Result<Vec<crate::kad::KadIndexedSource>, CoreError> {
        let file_hash_hex = crate::kad::KadId(*file_hash).to_hex();
        let mut stmt = self.conn.prepare(
            "SELECT file_hash, source_kad_id, ip, udp_port, tcp_port, file_size, encryption, expires_at_unix_ms, updated_at_unix_ms
             FROM kad_sources_index
             WHERE file_hash = ?1 AND expires_at_unix_ms > ?2
             ORDER BY updated_at_unix_ms DESC
             LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![file_hash_hex, now_unix_ms as i64, max_results as i64], |row| {
            let f_hash_str: String = row.get(0)?;
            let f_hash = crate::kad::KadId::from_hex(&f_hash_str)
                .map(|k| k.0)
                .unwrap_or([0u8; 16]);

            let kad_id_str: String = row.get(1)?;
            let source_kad_id = crate::kad::KadId::from_hex(&kad_id_str).unwrap_or(crate::kad::KadId::ZERO);

            let ip_str: String = row.get(2)?;
            let ip: std::net::Ipv4Addr = ip_str.parse().unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);

            let udp_port: i64 = row.get(3)?;
            let tcp_port: i64 = row.get(4)?;
            let file_size: i64 = row.get(5)?;
            let encryption: i64 = row.get(6)?;
            let expires_at: i64 = row.get(7)?;
            let updated_at: i64 = row.get(8)?;

            Ok(crate::kad::KadIndexedSource {
                file_hash: f_hash,
                source_kad_id,
                ip,
                udp_port: udp_port.max(0) as u16,
                tcp_port: tcp_port.max(0) as u16,
                file_size: file_size.max(0) as u64,
                encryption: encryption.max(0) as u8,
                expires_at_unix_ms: expires_at.max(0) as u64,
                updated_at_unix_ms: updated_at.max(0) as u64,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Purges expired Kad indexed sources.
    pub fn purge_expired_kad_sources(&mut self, now_unix_ms: u64) -> Result<usize, CoreError> {
        let count = self.conn.execute(
            "DELETE FROM kad_sources_index WHERE expires_at_unix_ms <= ?1",
            params![now_unix_ms as i64],
        )?;
        Ok(count)
    }

    /// Counts total indexed Kad sources.
    pub fn count_kad_indexed_sources(&self) -> Result<usize, CoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM kad_sources_index",
            [],
            |r| r.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    /// Inserts or updates Kad keyword index entries in batch.
    pub fn insert_kad_keyword_entries(
        &mut self,
        entries: &[crate::kad::KadKeywordEntry],
    ) -> Result<usize, CoreError> {
        let tx = self.conn.transaction()?;
        let mut inserted = 0;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO kad_keywords_index (
                    keyword_hash, file_hash, file_name, file_size, source_count,
                    discovered_at_unix_ms, expires_at_unix_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(keyword_hash, file_hash) DO UPDATE SET
                    file_name = excluded.file_name,
                    file_size = excluded.file_size,
                    source_count = MAX(kad_keywords_index.source_count, excluded.source_count),
                    expires_at_unix_ms = excluded.expires_at_unix_ms",
            )?;

            for e in entries {
                stmt.execute(params![
                    e.keyword_hash.to_lowercase(),
                    e.file_hash.to_lowercase(),
                    e.file_name,
                    e.file_size as i64,
                    e.source_count as i64,
                    e.discovered_at_unix_ms,
                    e.expires_at_unix_ms,
                ])?;
                inserted += 1;
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    /// Retrieves all active Kad keyword index entries matching a keyword hash.
    pub fn get_kad_keyword_entries(
        &self,
        keyword_hash: &str,
        now_unix_ms: u64,
    ) -> Result<Vec<crate::kad::KadKeywordEntry>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT keyword_hash, file_hash, file_name, file_size, source_count,
                    discovered_at_unix_ms, expires_at_unix_ms
             FROM kad_keywords_index
             WHERE keyword_hash = ?1 AND expires_at_unix_ms > ?2
             ORDER BY source_count DESC
             LIMIT 100",
        )?;

        let rows = stmt.query_map(params![keyword_hash.to_lowercase(), now_unix_ms as i64], |row| {
            let keyword_hash: String = row.get(0)?;
            let file_hash: String = row.get(1)?;
            let file_name: String = row.get(2)?;
            let file_size: i64 = row.get(3)?;
            let source_count: i64 = row.get(4)?;
            let discovered_at: i64 = row.get(5)?;
            let expires_at: i64 = row.get(6)?;

            Ok(crate::kad::KadKeywordEntry {
                keyword_hash,
                file_hash,
                file_name,
                file_size: file_size.max(0) as u64,
                source_count: source_count.max(1) as u32,
                discovered_at_unix_ms: discovered_at,
                expires_at_unix_ms: expires_at,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Purges expired Kad keyword entries.
    pub fn purge_expired_kad_keywords(&mut self, now_unix_ms: u64) -> Result<usize, CoreError> {
        let count = self.conn.execute(
            "DELETE FROM kad_keywords_index WHERE expires_at_unix_ms <= ?1",
            params![now_unix_ms as i64],
        )?;
        Ok(count)
    }

    /// Counts total indexed Kad keyword entries.
    pub fn count_kad_keyword_entries(&self) -> Result<usize, CoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM kad_keywords_index",
            [],
            |r| r.get(0),
        )?;
        Ok(count.max(0) as usize)
    }
}
