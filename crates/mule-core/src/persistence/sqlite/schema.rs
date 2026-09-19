use rusqlite::Connection;
use crate::error::CoreError;

pub fn init_schema(conn: &Connection) -> Result<(), CoreError> {
    conn.execute_batch(
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

    // Migrations
    let mut stmt = conn.prepare("PRAGMA table_info(downloads)")?;
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
        conn.execute_batch("ALTER TABLE downloads ADD COLUMN category_id TEXT;")?;
    }

    let mut stmt_peer = conn.prepare("PRAGMA table_info(peer_sources)")?;
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
        conn.execute_batch("ALTER TABLE peer_sources ADD COLUMN cooldown_until_unix_ms INTEGER;")?;
    }

    let mut stmt_peer2 = conn.prepare("PRAGMA table_info(peer_sources)")?;
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
        conn.execute_batch("ALTER TABLE peer_sources ADD COLUMN user_hash TEXT;")?;
    }
    if !has_crypt_options {
        conn.execute_batch("ALTER TABLE peer_sources ADD COLUMN crypt_options INTEGER DEFAULT 0;")?;
    }

    Ok(())
}
