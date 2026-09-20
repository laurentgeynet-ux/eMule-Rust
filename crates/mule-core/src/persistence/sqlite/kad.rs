use rusqlite::{params, Connection};

use crate::error::CoreError;
use crate::kad::{KadContact, KadId, KadIndexedNote, KadIndexedSource, KadKeywordEntry};

pub fn upsert_kad_contact(conn: &mut Connection, contact: &KadContact) -> Result<(), CoreError> {
    conn.execute(
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

pub fn batch_upsert_kad_contacts(
    conn: &mut Connection,
    contacts: &[KadContact],
) -> Result<usize, CoreError> {
    let tx = conn.transaction()?;
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

pub fn list_kad_contacts(conn: &Connection, limit: usize) -> Result<Vec<KadContact>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT kad_id, ip, udp_port, tcp_port, version, verified, fail_count, first_seen_unix_ms, last_seen_unix_ms
         FROM kad_contacts
         ORDER BY last_seen_unix_ms DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        let kad_id_str: String = row.get(0)?;
        let kad_id = KadId::from_hex(&kad_id_str).unwrap_or(KadId::ZERO);
        let ip_str: String = row.get(1)?;
        let ip: std::net::Ipv4Addr = ip_str.parse().unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);
        let udp_port: i64 = row.get(2)?;
        let tcp_port: Option<i64> = row.get(3)?;
        let version: i64 = row.get(4)?;
        let verified: i64 = row.get(5)?;
        let fail_count: i64 = row.get(6)?;
        let first_seen: i64 = row.get(7)?;
        let last_seen: i64 = row.get(8)?;

        Ok(KadContact {
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

pub fn count_kad_contacts(conn: &Connection) -> Result<usize, CoreError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM kad_contacts", [], |r| r.get(0))?;
    Ok(count.max(0) as usize)
}

pub fn clear_kad_contacts(conn: &mut Connection) -> Result<usize, CoreError> {
    let rows = conn.execute("DELETE FROM kad_contacts", [])?;
    Ok(rows)
}

pub fn insert_kad_indexed_sources(
    conn: &mut Connection,
    sources: &[KadIndexedSource],
) -> Result<usize, CoreError> {
    if sources.is_empty() {
        return Ok(0);
    }
    let tx = conn.transaction()?;
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
            let file_hash_hex = KadId(s.file_hash).to_hex();
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

pub fn get_kad_indexed_sources(
    conn: &Connection,
    file_hash: &[u8; 16],
    max_results: usize,
    now_unix_ms: u64,
) -> Result<Vec<KadIndexedSource>, CoreError> {
    let file_hash_hex = KadId(*file_hash).to_hex();
    let mut stmt = conn.prepare(
        "SELECT file_hash, source_kad_id, ip, udp_port, tcp_port, file_size, encryption, expires_at_unix_ms, updated_at_unix_ms
         FROM kad_sources_index
         WHERE file_hash = ?1 AND expires_at_unix_ms > ?2
         ORDER BY updated_at_unix_ms DESC
         LIMIT ?3",
    )?;

    let rows = stmt.query_map(
        params![file_hash_hex, now_unix_ms as i64, max_results as i64],
        |row| {
            let f_hash_str: String = row.get(0)?;
            let f_hash = KadId::from_hex(&f_hash_str)
                .map(|k| k.0)
                .unwrap_or([0u8; 16]);

            let kad_id_str: String = row.get(1)?;
            let source_kad_id = KadId::from_hex(&kad_id_str).unwrap_or(KadId::ZERO);

            let ip_str: String = row.get(2)?;
            let ip: std::net::Ipv4Addr = ip_str.parse().unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);

            let udp_port: i64 = row.get(3)?;
            let tcp_port: i64 = row.get(4)?;
            let file_size: i64 = row.get(5)?;
            let encryption: i64 = row.get(6)?;
            let expires_at: i64 = row.get(7)?;
            let updated_at: i64 = row.get(8)?;

            Ok(KadIndexedSource {
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
        },
    )?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

/// Loads every live indexed source (storage-node rehydration at startup).
/// Bounded by `limit` to stay under the in-memory store cap.
pub fn list_all_kad_indexed_sources(
    conn: &Connection,
    now_unix_ms: u64,
    limit: usize,
) -> Result<Vec<KadIndexedSource>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT file_hash, source_kad_id, ip, udp_port, tcp_port, file_size, encryption, expires_at_unix_ms, updated_at_unix_ms
         FROM kad_sources_index
         WHERE expires_at_unix_ms > ?1
         ORDER BY updated_at_unix_ms DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![now_unix_ms as i64, limit as i64], |row| {
        let f_hash_str: String = row.get(0)?;
        let f_hash = KadId::from_hex(&f_hash_str)
            .map(|k| k.0)
            .unwrap_or([0u8; 16]);
        let kad_id_str: String = row.get(1)?;
        let source_kad_id = KadId::from_hex(&kad_id_str).unwrap_or(KadId::ZERO);
        let ip_str: String = row.get(2)?;
        let ip: std::net::Ipv4Addr = ip_str.parse().unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);
        Ok(KadIndexedSource {
            file_hash: f_hash,
            source_kad_id,
            ip,
            udp_port: (row.get::<_, i64>(3)?).max(0) as u16,
            tcp_port: (row.get::<_, i64>(4)?).max(0) as u16,
            file_size: (row.get::<_, i64>(5)?).max(0) as u64,
            encryption: (row.get::<_, i64>(6)?).max(0) as u8,
            expires_at_unix_ms: (row.get::<_, i64>(7)?).max(0) as u64,
            updated_at_unix_ms: (row.get::<_, i64>(8)?).max(0) as u64,
        })
    })?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

/// Loads every live keyword entry (storage-node rehydration at startup).
pub fn list_all_kad_keyword_entries(
    conn: &Connection,
    now_unix_ms: u64,
    limit: usize,
) -> Result<Vec<KadKeywordEntry>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT keyword_hash, file_hash, file_name, file_size, source_count,
                discovered_at_unix_ms, expires_at_unix_ms
         FROM kad_keywords_index
         WHERE expires_at_unix_ms > ?1
         ORDER BY discovered_at_unix_ms DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![now_unix_ms as i64, limit as i64], |row| {
        Ok(KadKeywordEntry {
            keyword_hash: row.get(0)?,
            file_hash: row.get(1)?,
            file_name: row.get(2)?,
            file_size: (row.get::<_, i64>(3)?).max(0) as u64,
            source_count: (row.get::<_, i64>(4)?).max(1) as u32,
            discovered_at_unix_ms: row.get(5)?,
            expires_at_unix_ms: row.get(6)?,
        })
    })?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn purge_expired_kad_sources(
    conn: &mut Connection,
    now_unix_ms: u64,
) -> Result<usize, CoreError> {
    let count = conn.execute(
        "DELETE FROM kad_sources_index WHERE expires_at_unix_ms <= ?1",
        params![now_unix_ms as i64],
    )?;
    Ok(count)
}

pub fn count_kad_indexed_sources(conn: &Connection) -> Result<usize, CoreError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM kad_sources_index", [], |r| r.get(0))?;
    Ok(count.max(0) as usize)
}

pub fn insert_kad_keyword_entries(
    conn: &mut Connection,
    entries: &[KadKeywordEntry],
) -> Result<usize, CoreError> {
    let tx = conn.transaction()?;
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

pub fn get_kad_keyword_entries(
    conn: &Connection,
    keyword_hash: &str,
    now_unix_ms: u64,
) -> Result<Vec<KadKeywordEntry>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT keyword_hash, file_hash, file_name, file_size, source_count,
                discovered_at_unix_ms, expires_at_unix_ms
         FROM kad_keywords_index
         WHERE keyword_hash = ?1 AND expires_at_unix_ms > ?2
         ORDER BY source_count DESC
         LIMIT 100",
    )?;

    let rows = stmt.query_map(
        params![keyword_hash.to_lowercase(), now_unix_ms as i64],
        |row| {
            let keyword_hash: String = row.get(0)?;
            let file_hash: String = row.get(1)?;
            let file_name: String = row.get(2)?;
            let file_size: i64 = row.get(3)?;
            let source_count: i64 = row.get(4)?;
            let discovered_at: i64 = row.get(5)?;
            let expires_at: i64 = row.get(6)?;

            Ok(KadKeywordEntry {
                keyword_hash,
                file_hash,
                file_name,
                file_size: file_size.max(0) as u64,
                source_count: source_count.max(1) as u32,
                discovered_at_unix_ms: discovered_at,
                expires_at_unix_ms: expires_at,
            })
        },
    )?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn purge_expired_kad_keywords(
    conn: &mut Connection,
    now_unix_ms: u64,
) -> Result<usize, CoreError> {
    let count = conn.execute(
        "DELETE FROM kad_keywords_index WHERE expires_at_unix_ms <= ?1",
        params![now_unix_ms as i64],
    )?;
    Ok(count)
}

pub fn count_kad_keyword_entries(conn: &Connection) -> Result<usize, CoreError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM kad_keywords_index", [], |r| r.get(0))?;
    Ok(count.max(0) as usize)
}

// ---------------------------------------------------------------------------
// Kad notes index (file comments / ratings — storage-node role)
// ---------------------------------------------------------------------------

/// Upserts note entries keyed by (file_hash, source_id) — one note per
/// publisher per file, like eMule `CEntry` with `m_bSource = false`.
pub fn insert_kad_notes(
    conn: &mut Connection,
    notes: &[KadIndexedNote],
) -> Result<usize, CoreError> {
    let tx = conn.transaction()?;
    let mut inserted = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO kad_notes_index (
                file_hash, source_id, publisher_ip, publisher_udp_port,
                tags_blob, discovered_at_unix_ms, expires_at_unix_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(file_hash, source_id) DO UPDATE SET
                publisher_ip = excluded.publisher_ip,
                publisher_udp_port = excluded.publisher_udp_port,
                tags_blob = excluded.tags_blob,
                expires_at_unix_ms = excluded.expires_at_unix_ms",
        )?;

        for n in notes {
            stmt.execute(params![
                n.file_hash.to_lowercase(),
                n.source_id.to_lowercase(),
                n.publisher_ip,
                n.publisher_udp_port as i64,
                n.tags_blob,
                n.discovered_at_unix_ms,
                n.expires_at_unix_ms,
            ])?;
            inserted += 1;
        }
    }
    tx.commit()?;
    Ok(inserted)
}

/// Loads every live note entry (storage-node rehydration at startup).
pub fn list_all_kad_notes(
    conn: &Connection,
    now_unix_ms: u64,
    limit: usize,
) -> Result<Vec<KadIndexedNote>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT file_hash, source_id, publisher_ip, publisher_udp_port,
                tags_blob, discovered_at_unix_ms, expires_at_unix_ms
         FROM kad_notes_index
         WHERE expires_at_unix_ms > ?1
         ORDER BY discovered_at_unix_ms DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![now_unix_ms as i64, limit as i64], |row| {
        Ok(KadIndexedNote {
            file_hash: row.get(0)?,
            source_id: row.get(1)?,
            publisher_ip: row.get(2)?,
            publisher_udp_port: (row.get::<_, i64>(3)?).max(0) as u16,
            tags_blob: row.get(4)?,
            discovered_at_unix_ms: row.get(5)?,
            expires_at_unix_ms: row.get(6)?,
        })
    })?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn purge_expired_kad_notes(
    conn: &mut Connection,
    now_unix_ms: u64,
) -> Result<usize, CoreError> {
    let count = conn.execute(
        "DELETE FROM kad_notes_index WHERE expires_at_unix_ms <= ?1",
        params![now_unix_ms as i64],
    )?;
    Ok(count)
}

pub fn count_kad_notes(conn: &Connection) -> Result<usize, CoreError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM kad_notes_index", [], |r| r.get(0))?;
    Ok(count.max(0) as usize)
}
