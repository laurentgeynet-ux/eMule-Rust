//! In-memory Kad indexed-entry storage (DHT storage node role).
//!
//! Mirrors eMule `CKademlia` indexed data: remote nodes publish source and
//! keyword entries targeting keys close to our Kad ID; we keep them with a
//! TTL and serve them back on `SearchSourceReq` / `SearchKeywordReq`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use mule_protocol::kad::{KadIdBytes, KadKeywordEntryWire, KadSourceWire};

/// Lifetime of a stored entry. eMule republishes sources/keywords roughly
/// every 5 hours; entries not refreshed by then are dropped.
pub const KAD_STORE_TTL: Duration = Duration::from_secs(5 * 60 * 60);
/// Maximum distinct keys indexed.
pub const MAX_STORE_KEYS: usize = 5_000;
/// Maximum entries stored per key (also the max page served to searchers).
pub const MAX_ENTRIES_PER_KEY: usize = 50;
/// Hard cap on total stored entries (anti-flood).
pub const MAX_TOTAL_ENTRIES: usize = 20_000;

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

/// Bounded in-memory store for Kad published entries.
#[derive(Default)]
pub struct KadStorage {
    sources: HashMap<KadIdBytes, Vec<StoredSource>>,
    keywords: HashMap<KadIdBytes, Vec<StoredKeywordEntry>>,
    total_entries: usize,
}

impl KadStorage {
    /// Stores a source contact for `file_hash`. One entry per publisher
    /// (keyed by `kad_id`, falling back to `ip:port` when the id is null).
    /// Returns `true` when the entry was accepted.
    pub fn store_source(&mut self, file_hash: KadIdBytes, source: KadSourceWire) -> bool {
        let now = Instant::now();
        let expires_at = now + KAD_STORE_TTL;

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
            .take(MAX_ENTRIES_PER_KEY)
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
        let expires_at = now + KAD_STORE_TTL;

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
            .take(MAX_ENTRIES_PER_KEY)
            .map(|e| e.entry.clone())
            .collect()
    }

    /// (stored source keys, stored keyword keys, total live entries) — for diagnostics.
    pub fn stats(&mut self) -> (usize, usize, usize) {
        (self.sources.len(), self.keywords.len(), self.total_entries)
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

    fn source(id_byte: u8, ip: [u8; 4]) -> KadSourceWire {
        let mut kad_id = [0u8; 16];
        kad_id[0] = id_byte;
        KadSourceWire {
            kad_id,
            ip,
            udp_port: 4672,
            tcp_port: 4662,
            encryption: 0,
        }
    }

    fn kw_entry(hash_byte: u8) -> KadKeywordEntryWire {
        let mut file_hash = [0u8; 16];
        file_hash[0] = hash_byte;
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
            assert!(st.store_source(key, source(i as u8, [1, 1, 1, 1])));
        }
        assert!(!st.store_source(key, source(200, [2, 2, 2, 2])));
        assert_eq!(st.get_sources(&key, 0).len(), MAX_ENTRIES_PER_KEY);
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
            .map(|i| kw_entry(i as u8))
            .collect();
        assert!(st.store_keyword_entries(kw, batch));
        assert!(!st.store_keyword_entries(kw, vec![kw_entry(250)]));
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
        let (_, _, total) = st.stats();
        assert_eq!(total, 2);
    }

    #[test]
    fn stats_track_entries() {
        let mut st = KadStorage::default();
        st.store_source([1u8; 16], source(1, [1, 1, 1, 1]));
        st.store_keyword_entries([2u8; 16], vec![kw_entry(1), kw_entry(2)]);
        let (s, k, total) = st.stats();
        assert_eq!((s, k, total), (1, 1, 3));
    }
}
