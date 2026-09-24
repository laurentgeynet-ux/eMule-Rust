// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! In-memory Kad indexed-entry storage (DHT storage node role).
//!
//! Mirrors eMule `CKademlia` indexed data: remote nodes publish source and
//! keyword entries targeting keys close to our Kad ID; we keep them with a
//! TTL and serve them back on `SearchSourceReq` / `SearchKeywordReq`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use mule_protocol::kad::{KadIdBytes, KadKeywordEntryWire, KadNoteWire, KadSourceWire};

/// Lifetime of a stored **source** entry — eMule `KADEMLIAREPUBLISHTIMES`
/// (`Opcodes.h:78`, 5 h); sources are republished each cycle.
pub const KAD_SOURCE_TTL: Duration = Duration::from_secs(5 * 60 * 60);
/// Lifetime of a stored **keyword** entry — eMule `KADEMLIAREPUBLISHTIMEK`
/// (`Opcodes.h:80`, 24 h).
pub const KAD_KEYWORD_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// Lifetime of a stored **note** entry — eMule `KADEMLIAREPUBLISHTIMEN`
/// (`Opcodes.h:79`, 24 h).
pub const KAD_NOTE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// Maximum distinct keys indexed — eMule `KADEMLIAMAXINDEX` (`Opcodes.h:82`).
pub const MAX_STORE_KEYS: usize = 50_000;
/// Maximum source/keyword entries stored per key — eMule
/// `KADEMLIAMAXSOURCEPERFILE` (`Opcodes.h:84`).
pub const MAX_ENTRIES_PER_KEY: usize = 1_000;
/// Maximum results served per search page — eMule `iMaxResults`
/// (`Indexed.cpp:619/707`, fragmented over `UDP_KAD_MAXFRAGMENT`).
pub const MAX_SEARCH_RESULT_PAGE: usize = 300;
/// Hard cap on total stored entries — eMule `KADEMLIAMAXENTRIES`
/// (`Opcodes.h:83`), also an anti-flood bound.
pub const MAX_TOTAL_ENTRIES: usize = 60_000;
/// Maximum note entries stored per target file — eMule
/// `KADEMLIAMAXNOTESPERFILE` (`Opcodes.h:85`).
pub const MAX_NOTES_PER_TARGET: usize = 150;

#[derive(Debug, Clone)]
struct StoredSource {
    source: KadSourceWire,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct StoredKeywordEntry {
    entry: KadKeywordEntryWire,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct StoredNote {
    note: KadNoteWire,
    publisher_ip: [u8; 4],
    publisher_udp_port: u16,
    expires_at: Instant,
}

/// A note entry rehydrated from persistence (tags already decoded).
pub struct HydratedNote {
    pub target_hash: KadIdBytes,
    pub note: KadNoteWire,
    pub publisher_ip: [u8; 4],
    pub publisher_udp_port: u16,
    /// Remaining TTL in milliseconds (0 = already expired, skipped).
    pub remaining_ms: u64,
}

/// Bounded in-memory store for Kad published entries.
#[derive(Default)]
pub struct KadStorage {
    sources: HashMap<KadIdBytes, Vec<StoredSource>>,
    keywords: HashMap<KadIdBytes, Vec<StoredKeywordEntry>>,
    /// target file hash → note entries (`CEntry` with `m_bSource = false`),
    /// deduplicated by `source_id` (the publisher's Kad ID).
    notes: HashMap<KadIdBytes, Vec<StoredNote>>,
    total_entries: usize,
}

impl KadStorage {
    /// Stores a source contact for `file_hash`. One entry per publisher
    /// (keyed by `kad_id`, falling back to `ip:port` when the id is null).
    /// Returns `true` when the entry was accepted.
    pub fn store_source(&mut self, file_hash: KadIdBytes, source: KadSourceWire) -> bool {
        let now = Instant::now();
        let expires_at = now + KAD_SOURCE_TTL;

        let dedup_key: [u8; 22] = if source.kad_id != [0u8; 16] {
            let mut k = [0u8; 22];
            k[..16].copy_from_slice(&source.kad_id);
            k
        } else {
            let mut k = [0u8; 22];
            k[..4].copy_from_slice(&source.ip);
            k[4..6].copy_from_slice(&source.tcp_port.to_be_bytes());
            k
        };

        let is_new_key = !self.sources.contains_key(&file_hash);
        if is_new_key && self.sources.len() >= MAX_STORE_KEYS {
            return false;
        }

        let entries = self.sources.entry(file_hash).or_default();
        let before = entries.len();
        entries.retain(|e| e.expires_at > now);
        self.total_entries = self.total_entries.saturating_sub(before - entries.len());

        let key_of = |s: &KadSourceWire| -> [u8; 22] {
            if s.kad_id != [0u8; 16] {
                let mut k = [0u8; 22];
                k[..16].copy_from_slice(&s.kad_id);
                k
            } else {
                let mut k = [0u8; 22];
                k[..4].copy_from_slice(&s.ip);
                k[4..6].copy_from_slice(&s.tcp_port.to_be_bytes());
                k
            }
        };

        if let Some(existing) = entries.iter_mut().find(|e| key_of(&e.source) == dedup_key) {
            existing.source = source;
            existing.expires_at = expires_at;
            return true;
        }
        if entries.len() >= MAX_ENTRIES_PER_KEY || self.total_entries >= MAX_TOTAL_ENTRIES {
            return false;
        }
        entries.push(StoredSource { source, expires_at });
        self.total_entries += 1;
        true
    }

    /// Returns live sources for `file_hash`, honoring `start_pos` pagination.
    pub fn get_sources(&mut self, file_hash: &KadIdBytes, start_pos: u16) -> Vec<KadSourceWire> {
        let now = Instant::now();
        let Some(entries) = self.sources.get_mut(file_hash) else {
            return Vec::new();
        };
        let before = entries.len();
        entries.retain(|e| e.expires_at > now);
        self.total_entries = self.total_entries.saturating_sub(before - entries.len());
        if entries.is_empty() {
            self.sources.remove(file_hash);
            return Vec::new();
        }
        entries
            .iter()
            .skip(start_pos as usize)
            .take(MAX_SEARCH_RESULT_PAGE)
            .map(|e| e.source.clone())
            .collect()
    }

    /// Stores published keyword entries under `keyword_hash`, deduplicated by
    /// file hash. Returns `true` when at least one entry was accepted.
    pub fn store_keyword_entries(
        &mut self,
        keyword_hash: KadIdBytes,
        new_entries: Vec<KadKeywordEntryWire>,
    ) -> bool {
        if new_entries.is_empty() {
            return false;
        }
        let now = Instant::now();
        let expires_at = now + KAD_KEYWORD_TTL;

        let is_new_key = !self.keywords.contains_key(&keyword_hash);
        if is_new_key && self.keywords.len() >= MAX_STORE_KEYS {
            return false;
        }

        let entries = self.keywords.entry(keyword_hash).or_default();
        let before = entries.len();
        entries.retain(|e| e.expires_at > now);
        self.total_entries = self.total_entries.saturating_sub(before - entries.len());

        let mut accepted = false;
        for entry in new_entries {
            if let Some(existing) = entries
                .iter_mut()
                .find(|e| e.entry.file_hash == entry.file_hash)
            {
                existing.entry = entry;
                existing.expires_at = expires_at;
                accepted = true;
                continue;
            }
            if entries.len() >= MAX_ENTRIES_PER_KEY || self.total_entries >= MAX_TOTAL_ENTRIES {
                break;
            }
            entries.push(StoredKeywordEntry { entry, expires_at });
            self.total_entries += 1;
            accepted = true;
        }
        accepted
    }

    /// Returns live keyword entries for `keyword_hash`, honoring `start_pos`.
    pub fn get_keyword_entries(
        &mut self,
        keyword_hash: &KadIdBytes,
        start_pos: u16,
    ) -> Vec<KadKeywordEntryWire> {
        let now = Instant::now();
        let Some(entries) = self.keywords.get_mut(keyword_hash) else {
            return Vec::new();
        };
        let before = entries.len();
        entries.retain(|e| e.expires_at > now);
        self.total_entries = self.total_entries.saturating_sub(before - entries.len());
        if entries.is_empty() {
            self.keywords.remove(keyword_hash);
            return Vec::new();
        }
        entries
            .iter()
            .skip(start_pos as usize)
            .take(MAX_SEARCH_RESULT_PAGE)
            .map(|e| e.entry.clone())
            .collect()
    }

    /// Stores a published note entry for `target_hash`, upserted by
    /// `source_id` (one note per publisher per file, like eMule `CEntry`).
    /// Returns `true` when accepted.
    pub fn store_note(
        &mut self,
        target_hash: KadIdBytes,
        note: KadNoteWire,
        publisher_ip: [u8; 4],
        publisher_udp_port: u16,
    ) -> bool {
        if note.source_id == [0u8; 16] {
            return false;
        }
        let now = Instant::now();
        let expires_at = now + KAD_NOTE_TTL;

        let is_new_key = !self.notes.contains_key(&target_hash);
        if is_new_key && self.notes.len() >= MAX_STORE_KEYS {
            return false;
        }

        let entries = self.notes.entry(target_hash).or_default();
        let before = entries.len();
        entries.retain(|e| e.expires_at > now);
        self.total_entries = self.total_entries.saturating_sub(before - entries.len());

        if let Some(existing) = entries
            .iter_mut()
            .find(|e| e.note.source_id == note.source_id)
        {
            existing.note = note;
            existing.publisher_ip = publisher_ip;
            existing.publisher_udp_port = publisher_udp_port;
            existing.expires_at = expires_at;
            return true;
        }
        if entries.len() >= MAX_NOTES_PER_TARGET || self.total_entries >= MAX_TOTAL_ENTRIES {
            return false;
        }
        entries.push(StoredNote {
            note,
            publisher_ip,
            publisher_udp_port,
            expires_at,
        });
        self.total_entries += 1;
        true
    }

    /// Returns live notes for `target_hash`. Mirrors
    /// `CIndexed::SendValidNoteResult`: entries whose declared file size
    /// disagrees with the requested size are filtered out.
    pub fn get_notes(
        &mut self,
        target_hash: &KadIdBytes,
        file_size: u64,
        max_results: usize,
    ) -> Vec<KadNoteWire> {
        let now = Instant::now();
        let Some(entries) = self.notes.get_mut(target_hash) else {
            return Vec::new();
        };
        let before = entries.len();
        entries.retain(|e| e.expires_at > now);
        self.total_entries = self.total_entries.saturating_sub(before - entries.len());
        if entries.is_empty() {
            self.notes.remove(target_hash);
            return Vec::new();
        }
        entries
            .iter()
            .filter(|e| e.note.file_size().is_none_or(|s| s == file_size))
            .take(max_results)
            .map(|e| e.note.clone())
            .collect()
    }

    /// Indexer load for `target_hash` expressed as a percentage (0-100),
    /// reported back in `KADEMLIA2_PUBLISH_RES` (eMule `uLoad`).
    pub fn note_load(&self, target_hash: &KadIdBytes) -> u8 {
        let count = self.notes.get(target_hash).map_or(0, |e| e.len());
        ((count * 100) / MAX_NOTES_PER_TARGET).min(100) as u8
    }

    /// (stored source keys, stored keyword keys, stored note keys, total live entries) — for diagnostics.
    pub fn stats(&mut self) -> (usize, usize, usize, usize) {
        (
            self.sources.len(),
            self.keywords.len(),
            self.notes.len(),
            self.total_entries,
        )
    }

    /// Rehydrates persisted note entries, preserving each entry's remaining
    /// TTL (expired or malformed entries are skipped by the caller).
    pub fn hydrate_notes(&mut self, notes: Vec<HydratedNote>) {
        for n in notes {
            if n.remaining_ms == 0 || self.total_entries >= MAX_TOTAL_ENTRIES {
                continue;
            }
            let entries = self.notes.entry(n.target_hash).or_default();
            if entries.len() >= MAX_NOTES_PER_TARGET {
                continue;
            }
            if let Some(existing) = entries
                .iter_mut()
                .find(|e| e.note.source_id == n.note.source_id)
            {
                existing.note = n.note;
                existing.expires_at = Instant::now() + Duration::from_millis(n.remaining_ms);
                continue;
            }
            entries.push(StoredNote {
                note: n.note,
                publisher_ip: n.publisher_ip,
                publisher_udp_port: n.publisher_udp_port,
                expires_at: Instant::now() + Duration::from_millis(n.remaining_ms),
            });
            self.total_entries += 1;
        }
    }

    /// Rehydrates the store from persisted index rows, preserving each
    /// entry's remaining TTL (entries already expired are skipped).
    pub fn hydrate(
        &mut self,
        sources: Vec<mule_core::kad::KadIndexedSource>,
        keywords: Vec<mule_core::kad::KadKeywordEntry>,
        now_unix_ms: u64,
    ) {
        for s in sources {
            let remaining = s.expires_at_unix_ms.saturating_sub(now_unix_ms);
            if remaining == 0 || self.total_entries >= MAX_TOTAL_ENTRIES {
                continue;
            }
            let entries = self.sources.entry(s.file_hash).or_default();
            if entries.len() >= MAX_ENTRIES_PER_KEY {
                continue;
            }
            entries.push(StoredSource {
                source: KadSourceWire {
                    kad_id: s.source_kad_id.0,
                    ip: s.ip.octets(),
                    udp_port: s.udp_port,
                    tcp_port: s.tcp_port,
                    encryption: s.encryption,
                    buddy_id: None,
                },
                expires_at: Instant::now() + Duration::from_millis(remaining),
            });
            self.total_entries += 1;
        }

        let mut grouped: HashMap<KadIdBytes, Vec<mule_core::kad::KadKeywordEntry>> = HashMap::new();
        for e in keywords {
            if self.total_entries >= MAX_TOTAL_ENTRIES {
                break;
            }
            let remaining = (e.expires_at_unix_ms.max(0) as u64).saturating_sub(now_unix_ms);
            if remaining == 0 {
                continue;
            }
            let Ok(hash_bytes) =
                <[u8; 16]>::try_from(hex::decode(&e.keyword_hash).unwrap_or_default().as_slice())
            else {
                continue;
            };
            grouped.entry(hash_bytes).or_default().push(e);
        }
        for (kw_hash, group) in grouped {
            let entries = self.keywords.entry(kw_hash).or_default();
            for e in group {
                if entries.len() >= MAX_ENTRIES_PER_KEY || self.total_entries >= MAX_TOTAL_ENTRIES {
                    break;
                }
                let remaining = (e.expires_at_unix_ms.max(0) as u64).saturating_sub(now_unix_ms);
                let Ok(file_hash) =
                    <[u8; 16]>::try_from(hex::decode(&e.file_hash).unwrap_or_default().as_slice())
                else {
                    continue;
                };
                entries.push(StoredKeywordEntry {
                    entry: KadKeywordEntryWire {
                        file_hash,
                        file_name: e.file_name,
                        file_size: e.file_size,
                        source_count: e.source_count,
                    },
                    expires_at: Instant::now() + Duration::from_millis(remaining),
                });
                self.total_entries += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(id: u16, ip: [u8; 4]) -> KadSourceWire {
        let mut kad_id = [0u8; 16];
        kad_id[..2].copy_from_slice(&id.to_le_bytes());
        KadSourceWire {
            kad_id,
            ip,
            udp_port: 4672,
            tcp_port: 4662,
            encryption: 0,
            buddy_id: None,
        }
    }

    fn kw_entry(hash_id: u16) -> KadKeywordEntryWire {
        let mut file_hash = [0u8; 16];
        file_hash[..2].copy_from_slice(&hash_id.to_le_bytes());
        KadKeywordEntryWire {
            file_hash,
            file_name: "file.bin".to_string(),
            file_size: 1234,
            source_count: 1,
        }
    }

    #[test]
    fn source_store_lookup_and_dedup() {
        let mut st = KadStorage::default();
        let key = [7u8; 16];
        assert!(st.store_source(key, source(1, [1, 2, 3, 4])));
        assert!(st.store_source(key, source(2, [5, 6, 7, 8])));
        assert_eq!(st.get_sources(&key, 0).len(), 2);
        // Same publisher kad_id refreshes instead of duplicating.
        let mut updated = source(1, [9, 9, 9, 9]);
        updated.tcp_port = 9999;
        assert!(st.store_source(key, updated));
        let got = st.get_sources(&key, 0);
        assert_eq!(got.len(), 2);
        assert_eq!(
            got.iter().find(|s| s.kad_id[0] == 1).unwrap().tcp_port,
            9999
        );
        // Pagination.
        assert_eq!(st.get_sources(&key, 1).len(), 1);
        assert!(st.get_sources(&key, 2).is_empty());
    }

    #[test]
    fn source_per_key_cap_enforced() {
        let mut st = KadStorage::default();
        let key = [3u8; 16];
        for i in 0..MAX_ENTRIES_PER_KEY {
            assert!(st.store_source(key, source(i as u16, [1, 1, 1, 1])));
        }
        assert!(!st.store_source(key, source(2000, [2, 2, 2, 2])));
        // Serving is paged to `MAX_SEARCH_RESULT_PAGE` (eMule iMaxResults).
        assert_eq!(st.get_sources(&key, 0).len(), MAX_SEARCH_RESULT_PAGE);
    }

    #[test]
    fn keyword_store_lookup_and_dedup() {
        let mut st = KadStorage::default();
        let kw = [9u8; 16];
        assert!(st.store_keyword_entries(kw, vec![kw_entry(1), kw_entry(2)]));
        assert_eq!(st.get_keyword_entries(&kw, 0).len(), 2);
        // Re-publish same file hash updates rather than duplicating.
        let mut upd = kw_entry(1);
        upd.file_name = "renamed.bin".to_string();
        assert!(st.store_keyword_entries(kw, vec![upd]));
        let got = st.get_keyword_entries(&kw, 0);
        assert_eq!(got.len(), 2);
        assert_eq!(
            got.iter().find(|e| e.file_hash[0] == 1).unwrap().file_name,
            "renamed.bin"
        );
        // Empty entry list is rejected.
        assert!(!st.store_keyword_entries([4u8; 16], Vec::new()));
    }

    #[test]
    fn keyword_per_key_cap_enforced() {
        let mut st = KadStorage::default();
        let kw = [5u8; 16];
        let batch: Vec<_> = (0..MAX_ENTRIES_PER_KEY)
            .map(|i| kw_entry(i as u16))
            .collect();
        assert!(st.store_keyword_entries(kw, batch));
        assert!(!st.store_keyword_entries(kw, vec![kw_entry(2000)]));
    }

    #[test]
    fn hydrate_restores_entries_and_skips_expired() {
        use mule_core::kad::{KadId, KadIndexedSource, KadKeywordEntry};
        let now_ms: u64 = 1_000_000;
        let mut st = KadStorage::default();
        let src = KadIndexedSource {
            file_hash: [9u8; 16],
            source_kad_id: KadId([5u8; 16]),
            ip: std::net::Ipv4Addr::new(1, 2, 3, 4),
            udp_port: 4672,
            tcp_port: 4662,
            file_size: 42,
            encryption: 0,
            expires_at_unix_ms: now_ms + 60_000,
            updated_at_unix_ms: now_ms,
        };
        let expired = KadIndexedSource {
            expires_at_unix_ms: now_ms - 1,
            ..src.clone()
        };
        let kw = KadKeywordEntry {
            keyword_hash: hex::encode([7u8; 16]),
            file_hash: hex::encode([9u8; 16]),
            file_name: "file.bin".into(),
            file_size: 42,
            source_count: 3,
            discovered_at_unix_ms: now_ms as i64,
            expires_at_unix_ms: (now_ms + 60_000) as i64,
        };
        st.hydrate(vec![src, expired], vec![kw], now_ms);
        assert_eq!(st.get_sources(&[9u8; 16], 0).len(), 1);
        assert_eq!(st.get_keyword_entries(&[7u8; 16], 0).len(), 1);
        let (_, _, _, total) = st.stats();
        assert_eq!(total, 2);
    }

    #[test]
    fn stats_track_entries() {
        let mut st = KadStorage::default();
        st.store_source([1u8; 16], source(1, [1, 1, 1, 1]));
        st.store_keyword_entries([2u8; 16], vec![kw_entry(1), kw_entry(2)]);
        let (s, k, n, total) = st.stats();
        assert_eq!((s, k, n, total), (1, 1, 0, 3));
    }

    fn note(src_byte: u8, size: u64) -> KadNoteWire {
        use mule_protocol::ed2k::tags::{Ed2kTag, TagName, TagValue};
        let mut source_id = [0u8; 16];
        source_id[0] = src_byte;
        KadNoteWire {
            source_id,
            tags: vec![
                Ed2kTag {
                    name: TagName::Id(0x02),
                    value: TagValue::UInt64(size),
                },
                Ed2kTag {
                    name: TagName::Id(0x0B),
                    value: TagValue::String("note text".to_string()),
                },
            ],
            unknown_tags: vec![],
        }
    }

    #[test]
    fn note_store_lookup_upsert_and_size_filter() {
        let mut st = KadStorage::default();
        let target = [8u8; 16];
        assert!(st.store_note(target, note(1, 1000), [1, 2, 3, 4], 4672));
        assert!(st.store_note(target, note(2, 1000), [5, 6, 7, 8], 4672));
        // Same publisher re-publishes → upsert, not duplicate.
        assert!(st.store_note(target, note(1, 1000), [9, 9, 9, 9], 1234));
        assert_eq!(st.get_notes(&target, 1000, 50).len(), 2);
        // Wrong file size filters the entries out (SendValidNoteResult).
        assert!(st.get_notes(&target, 999, 50).is_empty());
        // Null source_id is rejected.
        assert!(!st.store_note(target, note(0, 1000), [1, 1, 1, 1], 1));
        // Load reflects occupancy as a percentage of MAX_NOTES_PER_TARGET.
        assert_eq!(st.note_load(&target), (2 * 100 / MAX_NOTES_PER_TARGET) as u8);
        assert_eq!(st.note_load(&[0xEEu8; 16]), 0);
    }

    #[test]
    fn note_per_target_cap_enforced() {
        let mut st = KadStorage::default();
        let target = [6u8; 16];
        for i in 0..MAX_NOTES_PER_TARGET {
            assert!(st.store_note(target, note(i as u8 + 1, 5), [1, 1, 1, 1], 1));
        }
        assert!(!st.store_note(target, note(250, 5), [1, 1, 1, 1], 1));
        assert_eq!(st.note_load(&target), 100);
    }
}
