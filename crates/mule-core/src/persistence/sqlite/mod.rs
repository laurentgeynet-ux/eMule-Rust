// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use rusqlite::{params, Connection};
use std::path::Path;
use uuid::Uuid;

use crate::error::CoreError;
use crate::kad::{KadContact, KadIndexedNote, KadIndexedSource, KadKeywordEntry};
use crate::model::{
    Download, DownloadCategory, DownloadStatus, Ed2kServer, Friend, HistoryEntry, HistoryFilter,
    HistoryStats, PeerCredit, PeerSource, PeerSourceStatus, SharedFile,
};
use crate::persistence::traits::{
    DownloadRepository, KadRepository, PeerSourceRepository, ServerRepository,
};
use crate::ranges::RangeSet;

pub mod categories;
pub mod credits;
pub mod downloads;
pub mod friends;
pub mod history;
pub mod kad;
pub mod schema;
pub mod servers;
pub mod shares;
pub mod sources;

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
        schema::init_schema(&self.conn)
    }

    // ── Settings ────────────────────────────────────────────────────────────

    /// Gets a setting value by key.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?1")?;
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
        let rows = self
            .conn
            .execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(rows > 0)
    }

    // ── Downloads ───────────────────────────────────────────────────────────

    /// Saves a new download record.
    pub fn insert_download(&mut self, dl: &Download) -> Result<(), CoreError> {
        downloads::insert_download(&mut self.conn, dl)
    }

    /// Persists the trusted AICH root hash for a download.
    pub fn update_aich_root(
        &mut self,
        download_id: &Uuid,
        aich_root: &[u8; 20],
    ) -> Result<(), CoreError> {
        downloads::update_aich_root(&mut self.conn, download_id, aich_root)
    }

    /// Atomically replaces received ranges for a download in a single transaction.
    pub fn sync_received_ranges(
        &mut self,
        download_id: &Uuid,
        ranges: &RangeSet,
    ) -> Result<(), CoreError> {
        downloads::sync_received_ranges(&mut self.conn, download_id, ranges)
    }

    /// Loads the stored RangeSet for a download.
    pub fn load_received_ranges(&self, download_id: &Uuid) -> Result<RangeSet, CoreError> {
        downloads::load_received_ranges(&self.conn, download_id)
    }

    /// Records that a part has been verified.
    pub fn mark_part_verified(
        &mut self,
        download_id: &Uuid,
        part_index: u32,
    ) -> Result<(), CoreError> {
        downloads::mark_part_verified(&mut self.conn, download_id, part_index)
    }

    /// Unmarks a part upon corruption detection.
    pub fn unmark_part_verified(
        &mut self,
        download_id: &Uuid,
        part_index: u32,
    ) -> Result<(), CoreError> {
        downloads::unmark_part_verified(&mut self.conn, download_id, part_index)
    }

    /// Retrieves all verified part indexes.
    pub fn get_verified_parts(&self, download_id: &Uuid) -> Result<Vec<u32>, CoreError> {
        downloads::get_verified_parts(&self.conn, download_id)
    }

    /// Stores the per-part MD4 hashset for a file (keyed by lowercase hex ed2k hash).
    pub fn store_part_hashes(
        &mut self,
        file_hash: &str,
        part_hashes: &[[u8; 16]],
    ) -> Result<(), CoreError> {
        downloads::store_part_hashes(&mut self.conn, file_hash, part_hashes)
    }

    /// Loads the per-part MD4 hashset for a file, ordered by part index.
    pub fn get_part_hashes(&self, file_hash: &str) -> Result<Vec<[u8; 16]>, CoreError> {
        downloads::get_part_hashes(&self.conn, file_hash)
    }

    /// Deletes the stored per-part hashset for a file.
    pub fn delete_part_hashes(&mut self, file_hash: &str) -> Result<(), CoreError> {
        downloads::delete_part_hashes(&mut self.conn, file_hash)
    }

    /// Updates status of a download.
    pub fn update_status(
        &mut self,
        download_id: &Uuid,
        status: DownloadStatus,
    ) -> Result<(), CoreError> {
        downloads::update_status(&mut self.conn, download_id, status)
    }

    /// Persists the per-download priority ("low" | "normal" | "high").
    pub fn update_priority(
        &mut self,
        download_id: &Uuid,
        priority: &str,
    ) -> Result<(), CoreError> {
        downloads::update_priority(&mut self.conn, download_id, priority)
    }

    /// Persists the per-file source limit (eMule FT_MAXSOURCES).
    /// `None` restores the global default limit.
    pub fn update_max_sources(
        &mut self,
        download_id: &Uuid,
        max_sources: Option<u32>,
    ) -> Result<(), CoreError> {
        downloads::update_max_sources(&mut self.conn, download_id, max_sources)
    }

    /// Retrieves all persisted downloads from the database.
    pub fn get_all_downloads(&self) -> Result<Vec<Download>, CoreError> {
        downloads::get_all_downloads(&self.conn)
    }

    /// Deletes a download record and its associated ranges and verified parts.
    pub fn delete_download(&mut self, download_id: &Uuid) -> Result<bool, CoreError> {
        downloads::delete_download(&mut self.conn, download_id)
    }

    /// Updates the target finalization path of a download.
    pub fn update_target_path(
        &mut self,
        download_id: &Uuid,
        target_path: &Path,
    ) -> Result<(), CoreError> {
        downloads::update_target_path(&mut self.conn, download_id, target_path)
    }

    /// Renames a download: updates `display_name` and the derived
    /// `target_path` (finalization writes under the new name).
    pub fn update_download_name_and_target(
        &mut self,
        download_id: &Uuid,
        display_name: &str,
        target_path: &Path,
    ) -> Result<(), CoreError> {
        downloads::update_download_name_and_target(
            &mut self.conn,
            download_id,
            display_name,
            target_path,
        )
    }

    // ── Download History ────────────────────────────────────────────────────

    /// Inserts a new history record and returns the generated row ID.
    pub fn insert_history_entry(&mut self, entry: &HistoryEntry) -> Result<i64, CoreError> {
        history::insert_history_entry(&mut self.conn, entry)
    }

    /// Queries history entries matching the given filter.
    pub fn query_history(&self, filter: &HistoryFilter) -> Result<Vec<HistoryEntry>, CoreError> {
        history::query_history(&self.conn, filter)
    }

    /// Aggregates historical statistics across time windows (Session, 24h, 7d, Lifetime).
    pub fn get_history_stats(&self, session_start_unix_ms: i64) -> Result<HistoryStats, CoreError> {
        history::get_history_stats(&self.conn, session_start_unix_ms)
    }

    /// Deletes all history records.
    pub fn clear_history(&mut self) -> Result<usize, CoreError> {
        history::clear_history(&mut self.conn)
    }

    /// Prunes history records older than the specified retention days.
    pub fn apply_retention_policy(&mut self, retention_days: u32) -> Result<usize, CoreError> {
        history::apply_retention_policy(&mut self.conn, retention_days)
    }

    // ── Categories ──────────────────────────────────────────────────────────

    /// Retrieves all categories ordered by sort_order ascending, then created_at ascending.
    pub fn get_categories(&self) -> Result<Vec<DownloadCategory>, CoreError> {
        categories::get_categories(&self.conn)
    }

    /// Retrieves a category by its ID.
    pub fn get_category(&self, id: &str) -> Result<Option<DownloadCategory>, CoreError> {
        categories::get_category(&self.conn, id)
    }

    /// Inserts a new category.
    pub fn insert_category(&mut self, cat: &DownloadCategory) -> Result<(), CoreError> {
        categories::insert_category(&mut self.conn, cat)
    }

    /// Updates an existing category.
    pub fn update_category(&mut self, cat: &DownloadCategory) -> Result<bool, CoreError> {
        categories::update_category(&mut self.conn, cat)
    }

    /// Deletes a category and atomically reassigns all affected downloads to NULL.
    pub fn delete_category(&mut self, id: &str) -> Result<usize, CoreError> {
        categories::delete_category(&mut self.conn, id)
    }

    /// Reorders categories based on the provided ordered list of category IDs.
    pub fn reorder_categories(&mut self, category_ids: &[String]) -> Result<(), CoreError> {
        categories::reorder_categories(&mut self.conn, category_ids)
    }

    /// Sets or clears the category assigned to a download.
    pub fn set_download_category(
        &mut self,
        download_id: &Uuid,
        category_id: Option<&str>,
    ) -> Result<bool, CoreError> {
        categories::set_download_category(&mut self.conn, download_id, category_id)
    }

    // ── Servers ─────────────────────────────────────────────────────────────

    /// Upserts a batch of eD2K servers.
    pub fn upsert_servers(&mut self, servers: &[Ed2kServer]) -> Result<usize, CoreError> {
        servers::upsert_servers(&mut self.conn, servers)
    }

    /// Lists all persisted eD2K servers ordered by preference DESC, name ASC.
    pub fn list_servers(&self) -> Result<Vec<Ed2kServer>, CoreError> {
        servers::list_servers(&self.conn)
    }

    /// Updates the enabled/disabled state of a server.
    pub fn set_server_enabled(&mut self, address: &str, enabled: bool) -> Result<bool, CoreError> {
        servers::set_server_enabled(&mut self.conn, address, enabled)
    }

    /// Deletes a server from SQLite by address ("ip:port").
    pub fn delete_server(&mut self, address: &str) -> Result<bool, CoreError> {
        servers::delete_server(&mut self.conn, address)
    }

    /// Removes all servers from SQLite.
    pub fn clear_servers(&mut self) -> Result<usize, CoreError> {
        servers::clear_servers(&mut self.conn)
    }

    // ── Peer Sources Registry ───────────────────────────────────────────────

    /// Upserts a batch of peer sources for a given file hash.
    pub fn upsert_peer_sources(
        &mut self,
        file_hash: &str,
        sources: &[PeerSource],
    ) -> Result<usize, CoreError> {
        sources::upsert_peer_sources(&mut self.conn, file_hash, sources)
    }

    /// Lists all persisted peer sources for a given file hash.
    pub fn list_peer_sources(&self, file_hash: &str) -> Result<Vec<PeerSource>, CoreError> {
        sources::list_peer_sources(&self.conn, file_hash)
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
        sources::update_peer_source_status(
            &mut self.conn,
            file_hash,
            address,
            status,
            error,
            cooldown_until_unix_ms,
        )
    }

    /// Updates the learned software name and/or availability bitmask of a peer source.
    pub fn update_peer_source_details(
        &mut self,
        file_hash: &str,
        address: &str,
        software: Option<&str>,
        availability: Option<&[u8]>,
    ) -> Result<bool, CoreError> {
        sources::update_peer_source_details(
            &mut self.conn,
            file_hash,
            address,
            software,
            availability,
        )
    }

    /// Resets the cooldown and fail count for a peer source.
    pub fn reset_peer_source_cooldown(
        &mut self,
        file_hash: &str,
        address: &str,
    ) -> Result<bool, CoreError> {
        sources::reset_peer_source_cooldown(&mut self.conn, file_hash, address)
    }

    /// Deletes a specific peer source from SQLite.
    pub fn delete_peer_source(
        &mut self,
        file_hash: &str,
        address: &str,
    ) -> Result<bool, CoreError> {
        sources::delete_peer_source(&mut self.conn, file_hash, address)
    }

    /// Clears all peer sources for a file hash.
    pub fn clear_peer_sources(&mut self, file_hash: &str) -> Result<usize, CoreError> {
        sources::clear_peer_sources(&mut self.conn, file_hash)
    }

    // ── Shared Files ────────────────────────────────────────────────────────

    /// Upserts a shared file in SQLite.
    pub fn upsert_shared_file(&mut self, file: &SharedFile) -> Result<(), CoreError> {
        shares::upsert_shared_file(&mut self.conn, file)
    }

    /// Lists all shared files in SQLite.
    pub fn list_shared_files(&self) -> Result<Vec<SharedFile>, CoreError> {
        shares::list_shared_files(&self.conn)
    }

    /// Retrieves a specific shared file by hash.
    pub fn get_shared_file(&self, file_hash: &str) -> Result<Option<SharedFile>, CoreError> {
        shares::get_shared_file(&self.conn, file_hash)
    }

    /// Deletes a shared file entry from SQLite.
    pub fn delete_shared_file(&mut self, file_hash: &str) -> Result<bool, CoreError> {
        shares::delete_shared_file(&mut self.conn, file_hash)
    }

    /// Toggles the enabled state of a shared file.
    pub fn set_shared_file_enabled(
        &mut self,
        file_hash: &str,
        enabled: bool,
    ) -> Result<bool, CoreError> {
        shares::set_shared_file_enabled(&mut self.conn, file_hash, enabled)
    }

    /// Ajoute `delta` octets au total uploadé persisté d'un fichier partagé.
    pub fn add_share_uploaded_bytes(
        &mut self,
        file_hash: &str,
        delta: u64,
    ) -> Result<(), CoreError> {
        shares::add_share_uploaded_bytes(&mut self.conn, file_hash, delta)
    }

    /// Totaux uploadés persistés par hash de fichier (clés en minuscules).
    pub fn list_share_uploaded_bytes(
        &self,
    ) -> Result<std::collections::HashMap<String, u64>, CoreError> {
        shares::list_share_uploaded_bytes(&self.conn)
    }

    /// Associe (ou retire, `None`) une catégorie à un partage persisté.
    pub fn set_shared_file_category(
        &mut self,
        file_hash: &str,
        category_id: Option<&str>,
    ) -> Result<bool, CoreError> {
        shares::set_shared_file_category(&mut self.conn, file_hash, category_id)
    }

    /// Sets the upload priority (`up_priority`) of a shared file.
    pub fn set_shared_up_priority(
        &mut self,
        file_hash: &str,
        up_priority: &str,
    ) -> Result<bool, CoreError> {
        shares::set_shared_up_priority(&mut self.conn, file_hash, up_priority)
    }

    /// Caches the computed AICH root hash of a shared file.
    pub fn set_shared_aich_root(
        &mut self,
        file_hash: &str,
        aich_root: &[u8; 20],
    ) -> Result<bool, CoreError> {
        shares::set_shared_aich_root(&mut self.conn, file_hash, aich_root)
    }

    // ── Peer Credits ────────────────────────────────────────────────────────

    /// Upserts a peer credit record into SQLite.
    pub fn upsert_peer_credit(&mut self, credit: &PeerCredit) -> Result<(), CoreError> {
        credits::upsert_peer_credit(&mut self.conn, credit)
    }

    /// Retrieves a peer credit record by public key hash.
    pub fn get_peer_credit(&self, pubkey_hash: &str) -> Result<Option<PeerCredit>, CoreError> {
        credits::get_peer_credit(&self.conn, pubkey_hash)
    }

    /// Increments uploaded bytes counter for a peer credit record.
    pub fn record_credit_upload(
        &mut self,
        pubkey_hash: &str,
        pubkey_hex: &str,
        bytes: u64,
    ) -> Result<(), CoreError> {
        credits::record_credit_upload(&mut self.conn, pubkey_hash, pubkey_hex, bytes)
    }

    /// Increments downloaded bytes counter for a peer credit record.
    pub fn record_credit_download(
        &mut self,
        pubkey_hash: &str,
        pubkey_hex: &str,
        bytes: u64,
    ) -> Result<(), CoreError> {
        credits::record_credit_download(&mut self.conn, pubkey_hash, pubkey_hex, bytes)
    }

    /// Lists top peer credits.
    pub fn list_peer_credits(&self, limit: usize) -> Result<Vec<PeerCredit>, CoreError> {
        credits::list_peer_credits(&self.conn, limit)
    }

    // ── Friends ─────────────────────────────────────────────────────────────

    /// Adds or updates a friend (eMule `friends.met` equivalent).
    pub fn add_friend(&mut self, user_hash: &str, name: &str) -> Result<(), CoreError> {
        friends::add_friend(&mut self.conn, user_hash, name)
    }

    /// Removes a friend by user hash.
    pub fn remove_friend(&mut self, user_hash: &str) -> Result<(), CoreError> {
        friends::remove_friend(&mut self.conn, user_hash)
    }

    /// Lists all friends.
    pub fn list_friends(&self) -> Result<Vec<Friend>, CoreError> {
        friends::list_friends(&self.conn)
    }

    // ── Kad Contacts & Routing ──────────────────────────────────────────────

    /// Upserts a Kad contact record.
    pub fn upsert_kad_contact(&mut self, contact: &KadContact) -> Result<(), CoreError> {
        kad::upsert_kad_contact(&mut self.conn, contact)
    }

    /// Batch upserts Kad contacts in a single transaction.
    pub fn batch_upsert_kad_contacts(
        &mut self,
        contacts: &[KadContact],
    ) -> Result<usize, CoreError> {
        kad::batch_upsert_kad_contacts(&mut self.conn, contacts)
    }

    /// Retrieves all Kad contacts up to a given limit.
    pub fn list_kad_contacts(&self, limit: usize) -> Result<Vec<KadContact>, CoreError> {
        kad::list_kad_contacts(&self.conn, limit)
    }

    /// Counts total Kad contacts stored in SQLite.
    pub fn count_kad_contacts(&self) -> Result<usize, CoreError> {
        kad::count_kad_contacts(&self.conn)
    }

    /// Deletes all Kad contacts.
    pub fn clear_kad_contacts(&mut self) -> Result<usize, CoreError> {
        kad::clear_kad_contacts(&mut self.conn)
    }

    // ── Kad Indexed Sources ─────────────────────────────────────────────────

    /// Inserts or replaces a batch of Kad indexed sources.
    pub fn insert_kad_indexed_sources(
        &mut self,
        sources: &[KadIndexedSource],
    ) -> Result<usize, CoreError> {
        kad::insert_kad_indexed_sources(&mut self.conn, sources)
    }

    /// Retrieves unexpired indexed Kad sources for a given file hash.
    pub fn get_kad_indexed_sources(
        &self,
        file_hash: &[u8; 16],
        max_results: usize,
        now_unix_ms: u64,
    ) -> Result<Vec<KadIndexedSource>, CoreError> {
        kad::get_kad_indexed_sources(&self.conn, file_hash, max_results, now_unix_ms)
    }

    /// Loads every live indexed Kad source (storage-node rehydration).
    pub fn list_all_kad_indexed_sources(
        &self,
        now_unix_ms: u64,
        limit: usize,
    ) -> Result<Vec<KadIndexedSource>, CoreError> {
        kad::list_all_kad_indexed_sources(&self.conn, now_unix_ms, limit)
    }

    /// Purges expired Kad indexed sources.
    pub fn purge_expired_kad_sources(&mut self, now_unix_ms: u64) -> Result<usize, CoreError> {
        kad::purge_expired_kad_sources(&mut self.conn, now_unix_ms)
    }

    /// Counts total indexed Kad sources.
    pub fn count_kad_indexed_sources(&self) -> Result<usize, CoreError> {
        kad::count_kad_indexed_sources(&self.conn)
    }

    // ── Kad Keyword Index ───────────────────────────────────────────────────

    /// Inserts or updates Kad keyword index entries in batch.
    pub fn insert_kad_keyword_entries(
        &mut self,
        entries: &[KadKeywordEntry],
    ) -> Result<usize, CoreError> {
        kad::insert_kad_keyword_entries(&mut self.conn, entries)
    }

    /// Retrieves all active Kad keyword index entries matching a keyword hash.
    pub fn get_kad_keyword_entries(
        &self,
        keyword_hash: &str,
        now_unix_ms: u64,
    ) -> Result<Vec<KadKeywordEntry>, CoreError> {
        kad::get_kad_keyword_entries(&self.conn, keyword_hash, now_unix_ms)
    }

    /// Loads every live Kad keyword entry (storage-node rehydration).
    pub fn list_all_kad_keyword_entries(
        &self,
        now_unix_ms: u64,
        limit: usize,
    ) -> Result<Vec<KadKeywordEntry>, CoreError> {
        kad::list_all_kad_keyword_entries(&self.conn, now_unix_ms, limit)
    }

    /// Purges expired Kad keyword entries.
    pub fn purge_expired_kad_keywords(&mut self, now_unix_ms: u64) -> Result<usize, CoreError> {
        kad::purge_expired_kad_keywords(&mut self.conn, now_unix_ms)
    }

    /// Counts total indexed Kad keyword entries.
    pub fn count_kad_keyword_entries(&self) -> Result<usize, CoreError> {
        kad::count_kad_keyword_entries(&self.conn)
    }

    /// Upserts Kad note entries (storage-node role).
    pub fn insert_kad_notes(&mut self, notes: &[KadIndexedNote]) -> Result<usize, CoreError> {
        kad::insert_kad_notes(&mut self.conn, notes)
    }

    /// Loads every live Kad note entry for store rehydration.
    pub fn list_all_kad_notes(
        &self,
        now_unix_ms: u64,
        limit: usize,
    ) -> Result<Vec<KadIndexedNote>, CoreError> {
        kad::list_all_kad_notes(&self.conn, now_unix_ms, limit)
    }

    /// Purges expired Kad note entries.
    pub fn purge_expired_kad_notes(&mut self, now_unix_ms: u64) -> Result<usize, CoreError> {
        kad::purge_expired_kad_notes(&mut self.conn, now_unix_ms)
    }

    /// Counts total indexed Kad note entries.
    pub fn count_kad_notes(&self) -> Result<usize, CoreError> {
        kad::count_kad_notes(&self.conn)
    }
}

impl DownloadRepository for Database {
    fn insert_download(&mut self, download: &Download) -> Result<(), CoreError> {
        downloads::insert_download(&mut self.conn, download)
    }

    fn get_download(&self, id: &Uuid) -> Result<Option<Download>, CoreError> {
        downloads::get_download(&self.conn, id)
    }

    fn get_all_downloads(&self) -> Result<Vec<Download>, CoreError> {
        downloads::get_all_downloads(&self.conn)
    }

    fn update_status(&mut self, id: &Uuid, status: DownloadStatus) -> Result<(), CoreError> {
        downloads::update_status(&mut self.conn, id, status)
    }

    fn delete_download(&mut self, id: &Uuid) -> Result<bool, CoreError> {
        downloads::delete_download(&mut self.conn, id)
    }

    fn save_received_ranges(&mut self, id: &Uuid, ranges: &RangeSet) -> Result<(), CoreError> {
        downloads::sync_received_ranges(&mut self.conn, id, ranges)
    }

    fn load_received_ranges(&self, id: &Uuid) -> Result<RangeSet, CoreError> {
        downloads::load_received_ranges(&self.conn, id)
    }
}

impl PeerSourceRepository for Database {
    fn upsert_peer_sources(
        &mut self,
        file_hash_hex: &str,
        sources: &[PeerSource],
    ) -> Result<usize, CoreError> {
        sources::upsert_peer_sources(&mut self.conn, file_hash_hex, sources)
    }

    fn list_peer_sources(&self, file_hash_hex: &str) -> Result<Vec<PeerSource>, CoreError> {
        sources::list_peer_sources(&self.conn, file_hash_hex)
    }

    fn update_peer_source_status(
        &mut self,
        file_hash_hex: &str,
        address: &str,
        status: PeerSourceStatus,
        error: Option<&str>,
        cooldown_until_unix_ms: Option<i64>,
    ) -> Result<bool, CoreError> {
        sources::update_peer_source_status(
            &mut self.conn,
            file_hash_hex,
            address,
            status,
            error,
            cooldown_until_unix_ms,
        )
    }

    fn delete_peer_source(
        &mut self,
        file_hash_hex: &str,
        address: &str,
    ) -> Result<bool, CoreError> {
        sources::delete_peer_source(&mut self.conn, file_hash_hex, address)
    }
}

impl ServerRepository for Database {
    fn list_servers(&self) -> Result<Vec<Ed2kServer>, CoreError> {
        servers::list_servers(&self.conn)
    }

    fn upsert_servers(&mut self, servers: &[Ed2kServer]) -> Result<usize, CoreError> {
        servers::upsert_servers(&mut self.conn, servers)
    }

    fn delete_server(&mut self, address: &str) -> Result<bool, CoreError> {
        servers::delete_server(&mut self.conn, address)
    }

    fn clear_servers(&mut self) -> Result<usize, CoreError> {
        servers::clear_servers(&mut self.conn)
    }
}

impl KadRepository for Database {
    fn upsert_kad_contact(&mut self, contact: &KadContact) -> Result<(), CoreError> {
        kad::upsert_kad_contact(&mut self.conn, contact)
    }

    fn list_kad_contacts(&self, limit: usize) -> Result<Vec<KadContact>, CoreError> {
        kad::list_kad_contacts(&self.conn, limit)
    }

    fn insert_kad_sources(&mut self, sources: &[KadIndexedSource]) -> Result<usize, CoreError> {
        kad::insert_kad_indexed_sources(&mut self.conn, sources)
    }

    fn get_kad_sources(
        &self,
        target_kad_id: &[u8; 16],
        limit: usize,
        now_ms: u64,
    ) -> Result<Vec<KadIndexedSource>, CoreError> {
        kad::get_kad_indexed_sources(&self.conn, target_kad_id, limit, now_ms)
    }
}
