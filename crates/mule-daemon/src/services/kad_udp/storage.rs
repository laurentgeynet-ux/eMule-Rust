// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! In-memory Kad indexed-entry storage (DHT storage node role).
//!
//! Mirrors eMule `CKademlia` indexed data: remote nodes publish source and
//! keyword entries targeting keys close to our Kad ID; we keep them with a
//! TTL and serve them back on `SearchSourceReq` / `SearchKeywordReq`.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use mule_protocol::kad::{
    AichHashVote, KadIdBytes, KadKeywordEntryWire, KadNoteWire, KadSourceWire,
    AICH_ROOT_HASH_SIZE,
};

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
/// Maximum tracked publishers per keyword entry — eMule keeps at most 100
/// `structPublishingIP` records, evicting the oldest
/// (`Entry.cpp` `MergeIPsAndFilenames`).
const MAX_TRACKED_PUBLISHERS: usize = 100;
/// Maximum distinct filenames tracked per keyword entry — eMule "natural
/// limit" of `m_aFileNames` (`Entry.cpp:437`).
const MAX_TRACKED_FILENAMES: usize = 255;
/// `PUBLISHPOINTSSPERSUBNET` (`Entry.cpp:477`) — trust points a publisher
/// contributes, divided by its /24 subnet's global publish count.
const PUBLISH_POINTS_PER_SUBNET: f32 = 10.0;
/// Trust threshold for the first result pass — eMule serves entries with
/// `GetTrustValue() >= 1.0` before lower-trust ones (`Indexed.cpp:640`).
const TRUSTED_RESULT_THRESHOLD: f32 = 1.0;
/// Window for eMule `bFastRefresh`: a republish by the *same* publisher
/// sooner than `KADEMLIAREPUBLISHTIMES - HR2S(1)` (5 h - 1 h) does not
/// increase filename popularity.
const FAST_REFRESH_WINDOW: Duration = Duration::from_secs(4 * 60 * 60);
/// Publisher TTL inside one entry — eMule `KADEMLIAREPUBLISHTIMEK`
/// (`CleanUpTrackedPublishers` drops trackers older than 24 h).
const PUBLISHER_TTL: Duration = KAD_KEYWORD_TTL;

/// eMule `EqualKadTagStr` — Unicode case-insensitive filename comparison
/// used for name deduplication in `MergeIPsAndFilenames`.
fn kad_names_equal(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// `uIP & ~0xFF` — the /24 subnet key eMule `AdjustGlobalPublishTracking`
/// aggregates publish counts over (host-order u32 of the IPv4 octets).
fn publish_subnet_key(ip: [u8; 4]) -> u32 {
    u32::from_be_bytes(ip) & !0xFF
}

#[derive(Debug, Clone)]
struct StoredSource {
    source: KadSourceWire,
    expires_at: Instant,
}

/// One tracked publisher of a keyword entry — eMule `structPublishingIP`
/// (`Entry.h`), ordered oldest-first inside `publishers`.
#[derive(Debug, Clone)]
struct PublisherTrack {
    /// Publisher IPv4 octets (the `PUBLISH_KEY_REQ` sender).
    ip: [u8; 4],
    /// Last publish time from this publisher.
    last_publish: Instant,
    /// Index into `aich_hashes` of the AICH root this publisher last
    /// claimed — eMule `m_wAICHHashIdx`, `None` = `_UI16_MAX`.
    aich_idx: Option<usize>,
}

/// One distinct filename and its popularity — eMule
/// `structFileNameEntry` (`Entry.h:38`).
#[derive(Debug, Clone)]
struct FileNameTrack {
    name: String,
    /// Relative popularity index — not an exact publisher count
    /// (`GetCommonFileName` compares values only).
    popularity: u32,
}

#[derive(Debug, Clone)]
struct StoredKeywordEntry {
    entry: KadKeywordEntryWire,
    expires_at: Instant,
    /// Tracked publishers, oldest first — eMule `m_pliPublishingIPs`.
    publishers: VecDeque<PublisherTrack>,
    /// AICH roots indexed for this file — eMule `m_aAICHHashes`.
    aich_hashes: Vec<[u8; AICH_ROOT_HASH_SIZE]>,
    /// Per-hash publisher count — eMule `m_anAICHHashPopularity`.
    aich_popularity: Vec<u8>,
    /// Distinct filenames with popularity — eMule `m_aFileNames`.
    filenames: Vec<FileNameTrack>,
}

impl StoredKeywordEntry {
    fn new(entry: KadKeywordEntryWire, expires_at: Instant) -> Self {
        Self {
            filenames: Vec::new(),
            entry,
            expires_at,
            publishers: VecDeque::new(),
            aich_hashes: Vec::new(),
            aich_popularity: Vec::new(),
        }
    }

    /// eMule `CKeyEntry::AddRemoveAICHHash` — `add` increments (or appends)
    /// the popularity of `root` and returns its index; `!add` decrements
    /// and returns `None`. Zero-popularity hashes stay in the array to
    /// keep publisher `aich_idx` indices stable.
    fn add_remove_aich(&mut self, root: &[u8; AICH_ROOT_HASH_SIZE], add: bool) -> Option<usize> {
        let idx = self.aich_hashes.iter().position(|h| h == root);
        if add {
            match idx {
                Some(i) => {
                    self.aich_popularity[i] = self.aich_popularity[i].saturating_add(1);
                    Some(i)
                }
                None => {
                    self.aich_hashes.push(*root);
                    self.aich_popularity.push(1);
                    Some(self.aich_hashes.len() - 1)
                }
            }
        } else {
            idx.inspect(|&i| {
                self.aich_popularity[i] = self.aich_popularity[i].saturating_sub(1);
            })
        }
    }

    /// eMule `CKeyEntry::MergeIPsAndFilenames` filename merge: the new
    /// publication's name is appended with popularity 1 unless it
    /// case-insensitively matches a known name — then that name's
    /// popularity increases (except on a same-publisher fast refresh).
    fn merge_filenames(&mut self, new_name: &str, fast_refresh: bool) {
        if new_name.is_empty() {
            return;
        }
        if let Some(f) = self
            .filenames
            .iter_mut()
            .find(|f| kad_names_equal(&f.name, new_name))
        {
            if !fast_refresh {
                f.popularity = f.popularity.saturating_add(1);
            }
            return;
        }
        self.filenames.push(FileNameTrack {
            name: new_name.to_string(),
            popularity: 1,
        });
        if self.filenames.len() > MAX_TRACKED_FILENAMES {
            self.filenames.truncate(MAX_TRACKED_FILENAMES);
        }
    }

    /// eMule `CKeyEntry::GetCommonFileName` — the name most publishers
    /// agree on (first max popularity wins).
    fn common_file_name(&self) -> &str {
        let mut best: Option<&FileNameTrack> = None;
        for f in &self.filenames {
            if best.map_or(0, |b| b.popularity) < f.popularity {
                best = Some(f);
            }
        }
        best.map(|f| f.name.as_str())
            .unwrap_or(&self.entry.file_name)
    }

    /// eMule `CKeyEntry::CleanUpTrackedPublishers` — drops trackers older
    /// than `PUBLISHER_TTL` from the head of the ordered list. Like eMule,
    /// AICH popularity is NOT decremented for expired publishers (only
    /// for >100-publisher purges and republish changes).
    fn cleanup_publishers(&mut self, now: Instant, global: &mut HashMap<u32, u32>) {
        while let Some(front) = self.publishers.front() {
            if now < front.last_publish + PUBLISHER_TTL {
                break;
            }
            let old = self.publishers.pop_front().expect("front checked");
            adjust_publish_tracking(global, old.ip, false);
        }
    }

    /// eMule `CKeyEntry::GetTrustValue` — sum of
    /// `PUBLISH_POINTS_PER_SUBNET / subnet_count` over live publishers.
    fn trust_value(&self, global: &HashMap<u32, u32>) -> f32 {
        self.publishers
            .iter()
            .map(|p| {
                let n = global.get(&publish_subnet_key(p.ip)).copied().unwrap_or(0);
                if n > 0 {
                    PUBLISH_POINTS_PER_SUBNET / n as f32
                } else {
                    0.0
                }
            })
            .sum()
    }

    /// Builds the wire entry served in search results: common filename,
    /// AICH histogram (`TAG_KADAICHHASHRESULT` source) and
    /// `TAG_PUBLISHINFO` u32 — eMule `WriteTagListWithPublishInfo`.
    fn to_wire(&self, global: &HashMap<u32, u32>) -> KadKeywordEntryWire {
        let mut wire = self.entry.clone();
        wire.file_name = self.common_file_name().to_string();
        wire.aich_pub = None;
        wire.aich_votes = self
            .aich_hashes
            .iter()
            .zip(&self.aich_popularity)
            .filter(|(_, p)| **p > 0)
            .map(|(h, p)| AichHashVote {
                popularity: *p,
                root: *h,
            })
            .collect();
        wire.publish_info = (!self.publishers.is_empty()).then(|| {
            let names = self.filenames.len().min(255) as u32;
            let pubs = self.publishers.len().min(255) as u32;
            let trust = (100.0 * self.trust_value(global)) as u32 & 0xFFFF;
            (names << 24) | (pubs << 16) | trust
        });
        wire
    }
}

/// eMule `CKeyEntry::AdjustGlobalPublishTracking` — ±1 on the publisher's
/// /24 subnet count.
fn adjust_publish_tracking(global: &mut HashMap<u32, u32>, ip: [u8; 4], increase: bool) {
    let key = publish_subnet_key(ip);
    let count = global.entry(key).or_insert(0);
    if increase {
        *count += 1;
    } else {
        *count = count.saturating_sub(1);
    }
    if *count == 0 {
        global.remove(&key);
    }
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
    /// /24 subnet → count of keyword publishers registered from it —
    /// eMule `CKeyEntry::s_mapGlobalPublishIPs`, used by the trust value.
    publish_ip_counts: HashMap<u32, u32>,
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

    /// Stores published keyword entries under `keyword_hash`.
    ///
    /// Deduplicates by `(file_hash, file_size)` — eMule `AddKeyword` keeps
    /// separate entries when the same file id is published with a
    /// different size. `publisher_ip` feeds eMule's publish tracking
    /// (`MergeIPsAndFilenames`): the same publisher republishing refreshes
    /// its record (updating/removing its AICH contribution) instead of
    /// adding a second vote.
    ///
    /// Returns `true` when at least one entry was accepted.
    pub fn store_keyword_entries(
        &mut self,
        keyword_hash: KadIdBytes,
        new_entries: Vec<KadKeywordEntryWire>,
        publisher_ip: [u8; 4],
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
        for mut entry in new_entries {
            // The publisher's AICH claim is consumed by the merge; it is
            // never part of the stored tag list (eMule `AddTag` semantics).
            let aich_pub = entry.aich_pub.take();
            entry.aich_votes.clear();
            entry.publish_info = None;
            let pos = entries.iter().position(|e| {
                e.entry.file_hash == entry.file_hash && e.entry.file_size == entry.file_size
            });
            if let Some(i) = pos {
                Self::merge_keyword_entry(
                    &mut entries[i],
                    entry,
                    aich_pub,
                    publisher_ip,
                    now,
                    expires_at,
                    &mut self.publish_ip_counts,
                );
                accepted = true;
                continue;
            }
            if entries.len() >= MAX_ENTRIES_PER_KEY || self.total_entries >= MAX_TOTAL_ENTRIES {
                break;
            }
            let mut stored = StoredKeywordEntry::new(entry, expires_at);
            let new_name = stored.entry.file_name.clone();
            stored.merge_filenames(&new_name, false);
            let aich_idx = aich_pub.and_then(|root| stored.add_remove_aich(&root, true));
            stored.publishers.push_back(PublisherTrack {
                ip: publisher_ip,
                last_publish: now,
                aich_idx,
            });
            adjust_publish_tracking(&mut self.publish_ip_counts, publisher_ip, true);
            entries.push(stored);
            self.total_entries += 1;
            accepted = true;
        }
        accepted
    }

    /// eMule `CKeyEntry::MergeIPsAndFilenames` — merges the new
    /// publication's publisher/name/AICH tracking into `existing`, then
    /// replaces its wire data (the `*pOldEntry = *pEntry` assignment).
    fn merge_keyword_entry(
        existing: &mut StoredKeywordEntry,
        mut new_entry: KadKeywordEntryWire,
        aich_pub: Option<[u8; AICH_ROOT_HASH_SIZE]>,
        publisher_ip: [u8; 4],
        now: Instant,
        expires_at: Instant,
        global: &mut HashMap<u32, u32>,
    ) {
        // Publisher tracking.
        let mut fast_refresh = false;
        let pub_pos = existing.publishers.iter().position(|p| p.ip == publisher_ip);
        if let Some(pos) = pub_pos {
            // `bFastRefresh`: same publisher republishing before
            // KADEMLIAREPUBLISHTIMES - 1h does not bump name popularity.
            fast_refresh = existing.publishers[pos].last_publish > now - FAST_REFRESH_WINDOW;
            existing.publishers[pos].last_publish = now;
            // AICH contribution update (eMule: changed hash decrements
            // the old and increments the new; removed hash decrements).
            let prev_idx = existing.publishers[pos].aich_idx;
            match (aich_pub, prev_idx) {
                (Some(root), None) => {
                    existing.publishers[pos].aich_idx = existing.add_remove_aich(&root, true);
                }
                (Some(root), Some(idx)) if existing.aich_hashes[idx] != root => {
                    let old_root = existing.aich_hashes[idx];
                    existing.add_remove_aich(&old_root, false);
                    existing.publishers[pos].aich_idx = existing.add_remove_aich(&root, true);
                }
                (None, Some(idx)) => {
                    let old_root = existing.aich_hashes[idx];
                    existing.add_remove_aich(&old_root, false);
                    existing.publishers[pos].aich_idx = None;
                }
                _ => {}
            }
        } else {
            let aich_idx = aich_pub.and_then(|root| existing.add_remove_aich(&root, true));
            existing.publishers.push_back(PublisherTrack {
                ip: publisher_ip,
                last_publish: now,
                aich_idx,
            });
            adjust_publish_tracking(global, publisher_ip, true);
            // eMule keeps at most 100 tracked publishers per entry.
            if existing.publishers.len() > MAX_TRACKED_PUBLISHERS {
                if let Some(old) = existing.publishers.pop_front() {
                    if let Some(idx) = old.aich_idx {
                        let old_root = existing.aich_hashes[idx];
                        existing.add_remove_aich(&old_root, false);
                    }
                    adjust_publish_tracking(global, old.ip, false);
                }
            }
        }

        // Filename merge (new name appended or matching name popularity).
        let new_name = new_entry.file_name.clone();
        existing.merge_filenames(&new_name, fast_refresh);

        // `*pOldEntry = *pEntry`: the newest wire data wins; the node
        // computed tags are re-synthesized at serve time.
        new_entry.aich_pub = None;
        existing.entry = new_entry;
        existing.expires_at = expires_at;
    }

    /// Returns live keyword entries for `keyword_hash`, honoring `start_pos`.
    pub fn get_keyword_entries(
        &mut self,
        keyword_hash: &KadIdBytes,
        start_pos: u16,
    ) -> Vec<KadKeywordEntryWire> {
        self.get_keyword_entries_filtered(keyword_hash, start_pos, |_| true)
    }

    /// Returns live keyword entries matching `filter`, honoring `start_pos`
    /// over the *matching* set — eMule `SendValidKeywordResult` counts the
    /// start position after `StartSearchTermsMatch` filtering, not before
    /// (`Indexed.cpp:630`).
    ///
    /// Results are served in two passes like eMule: trusted entries
    /// (`trust >= 1.0`) first, then the rest — the `start_pos` offset
    /// skips matching entries across both passes.
    pub fn get_keyword_entries_filtered(
        &mut self,
        keyword_hash: &KadIdBytes,
        start_pos: u16,
        filter: impl Fn(&KadKeywordEntryWire) -> bool,
    ) -> Vec<KadKeywordEntryWire> {
        let now = Instant::now();
        let Some(entries) = self.keywords.get_mut(keyword_hash) else {
            return Vec::new();
        };
        let before = entries.len();
        entries.retain(|e| e.expires_at > now);
        self.total_entries = self.total_entries.saturating_sub(before - entries.len());
        for e in entries.iter_mut() {
            e.cleanup_publishers(now, &mut self.publish_ip_counts);
        }
        if entries.is_empty() {
            self.keywords.remove(keyword_hash);
            return Vec::new();
        }
        // eMule `SendValidKeywordResult`: trusted entries first, then the
        // rest — both passes share the same `start_pos` counter. The
        // filter sees the served wire (common filename — eMule matches
        // `StartSearchTermsMatch` on `GetCommonFileName`).
        let mut out = Vec::with_capacity(MAX_SEARCH_RESULT_PAGE.min(entries.len()));
        let mut skipped = 0usize;
        'passes: for only_trusted in [true, false] {
            for e in entries.iter() {
                let trusted = e.trust_value(&self.publish_ip_counts) >= TRUSTED_RESULT_THRESHOLD;
                if trusted != only_trusted {
                    continue;
                }
                let wire = e.to_wire(&self.publish_ip_counts);
                if !filter(&wire) {
                    continue;
                }
                if skipped < start_pos as usize {
                    skipped += 1;
                    continue;
                }
                if out.len() >= MAX_SEARCH_RESULT_PAGE {
                    break 'passes;
                }
                out.push(wire);
            }
        }
        out
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
                        file_name: e.file_name.clone(),
                        file_size: e.file_size,
                        source_count: e.source_count,
                        // Published metadata tags are kept in memory only
                        // (eMule does not persist its index either), so
                        // rehydrated entries answer filename/filesize
                        // restrictive terms only.
                        tags: Vec::new(),
                        aich_pub: None,
                        aich_votes: Vec::new(),
                        publish_info: None,
                    },
                    expires_at: Instant::now() + Duration::from_millis(remaining),
                    // Publisher/AICH tracking is not persisted (eMule does
                    // persist it via `WritePublishTrackingDataToFile` —
                    // documented gap): rehydrated entries serve at trust 0.
                    publishers: VecDeque::new(),
                    aich_hashes: Vec::new(),
                    aich_popularity: Vec::new(),
                    filenames: vec![FileNameTrack {
                        name: e.file_name,
                        popularity: 1,
                    }],
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
            tags: Vec::new(),
            aich_pub: None,
            aich_votes: Vec::new(),
            publish_info: None,
        }
    }

    const TEST_PUB_IP: [u8; 4] = [10, 0, 0, 1];

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
        assert!(st.store_keyword_entries(kw, vec![kw_entry(1), kw_entry(2)], TEST_PUB_IP));
        assert_eq!(st.get_keyword_entries(&kw, 0).len(), 2);
        // Re-publish same file hash updates rather than duplicating.
        let mut upd = kw_entry(1);
        upd.file_name = "renamed.bin".to_string();
        assert!(st.store_keyword_entries(kw, vec![upd], TEST_PUB_IP));
        let got = st.get_keyword_entries(&kw, 0);
        assert_eq!(got.len(), 2);
        // eMule `GetCommonFileName` serves the most-popular name — a
        // single rename does not outvote the name publishers agreed on
        // (both at popularity 1, oldest wins).
        assert_eq!(
            got.iter().find(|e| e.file_hash[0] == 1).unwrap().file_name,
            "file.bin"
        );
        // Empty entry list is rejected.
        assert!(!st.store_keyword_entries([4u8; 16], Vec::new(), TEST_PUB_IP));
    }

    #[test]
    fn keyword_filtered_query_paginates_after_filter() {
        // eMule `SendValidKeywordResult`: `start_pos` counts the entries
        // that pass `StartSearchTermsMatch`, not the raw stored set.
        let mut st = KadStorage::default();
        let kw = [6u8; 16];
        let mk = |id: u16, name: &str| {
            let mut e = kw_entry(id);
            e.file_name = name.to_string();
            e
        };
        assert!(st.store_keyword_entries(
            kw,
            vec![
                mk(1, "skip.bin"),
                mk(2, "keep-one.bin"),
                mk(3, "skip.bin"),
                mk(4, "keep-two.bin"),
                mk(5, "keep-three.bin"),
            ],
            TEST_PUB_IP,
        ));
        let got = st.get_keyword_entries_filtered(&kw, 0, |e| e.file_name.starts_with("keep"));
        assert_eq!(got.len(), 3);
        // start_pos=2 must skip two *matching* entries.
        let got = st.get_keyword_entries_filtered(&kw, 2, |e| e.file_name.starts_with("keep"));
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].file_name, "keep-three.bin");
        // No match -> empty, no panic.
        assert!(st
            .get_keyword_entries_filtered(&kw, 0, |e| e.file_name == "nope")
            .is_empty());
    }

    #[test]
    fn keyword_per_key_cap_enforced() {
        let mut st = KadStorage::default();
        let kw = [5u8; 16];
        let batch: Vec<_> = (0..MAX_ENTRIES_PER_KEY)
            .map(|i| kw_entry(i as u16))
            .collect();
        assert!(st.store_keyword_entries(kw, batch, TEST_PUB_IP));
        assert!(!st.store_keyword_entries(kw, vec![kw_entry(2000)], TEST_PUB_IP));
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
        st.store_keyword_entries([2u8; 16], vec![kw_entry(1), kw_entry(2)], TEST_PUB_IP);
        let (s, k, n, total) = st.stats();
        assert_eq!((s, k, n, total), (1, 1, 0, 3));
    }

    // ---- AICH keyword tracking (eMule `CKeyEntry`) ----

    fn kw_with_aich(hash_id: u16, root: [u8; 20]) -> KadKeywordEntryWire {
        let mut e = kw_entry(hash_id);
        e.aich_pub = Some(root);
        e
    }

    fn votes_of(e: &KadKeywordEntryWire) -> Vec<(u8, [u8; 20])> {
        e.aich_votes
            .iter()
            .map(|v| (v.popularity, v.root))
            .collect()
    }

    #[test]
    fn aich_two_publishers_same_root_popularity_two() {
        let mut st = KadStorage::default();
        let kw = [9u8; 16];
        let root = [0x42u8; 20];
        assert!(st.store_keyword_entries(
            kw,
            vec![kw_with_aich(1, root)],
            [10, 0, 0, 1]
        ));
        // Second publisher, different IP, same file+size, same root.
        assert!(st.store_keyword_entries(
            kw,
            vec![kw_with_aich(1, root)],
            [10, 0, 0, 2]
        ));
        let got = st.get_keyword_entries(&kw, 0);
        assert_eq!(got.len(), 1);
        assert_eq!(votes_of(&got[0]), vec![(2, root)]);
        // PUBLISHINFO: 1 name, 2 publishers.
        let info = got[0].publish_info.expect("publish info");
        assert_eq!((info >> 24) & 0xFF, 1);
        assert_eq!((info >> 16) & 0xFF, 2);
    }

    #[test]
    fn aich_distinct_roots_popularity_split() {
        let mut st = KadStorage::default();
        let kw = [9u8; 16];
        assert!(st.store_keyword_entries(
            kw,
            vec![kw_with_aich(1, [0x11; 20])],
            [10, 0, 0, 1]
        ));
        assert!(st.store_keyword_entries(
            kw,
            vec![kw_with_aich(1, [0x22; 20])],
            [10, 0, 0, 2]
        ));
        let got = st.get_keyword_entries(&kw, 0);
        let mut votes = votes_of(&got[0]);
        votes.sort();
        assert_eq!(votes, vec![(1, [0x11; 20]), (1, [0x22; 20])]);
    }

    #[test]
    fn aich_refresh_changes_root_old_vote_released() {
        // eMule `MergeIPsAndFilenames`: the same publisher republishing
        // with a different AICH root decrements its old vote.
        let mut st = KadStorage::default();
        let kw = [9u8; 16];
        let ip = [10, 0, 0, 1];
        assert!(st.store_keyword_entries(kw, vec![kw_with_aich(1, [0x11; 20])], ip));
        assert!(st.store_keyword_entries(kw, vec![kw_with_aich(1, [0x22; 20])], ip));
        let got = st.get_keyword_entries(&kw, 0);
        assert_eq!(votes_of(&got[0]), vec![(1, [0x22; 20])]);
        assert_eq!((got[0].publish_info.unwrap() >> 16) & 0xFF, 1);
    }

    #[test]
    fn aich_refresh_drops_root_vote_removed() {
        // Publisher republishing without AICH removes its contribution.
        let mut st = KadStorage::default();
        let kw = [9u8; 16];
        let ip = [10, 0, 0, 1];
        assert!(st.store_keyword_entries(kw, vec![kw_with_aich(1, [0x11; 20])], ip));
        assert!(st.store_keyword_entries(kw, vec![kw_entry(1)], ip));
        let got = st.get_keyword_entries(&kw, 0);
        assert!(got[0].aich_votes.is_empty());
    }

    #[test]
    fn aich_republish_same_root_no_double_vote() {
        let mut st = KadStorage::default();
        let kw = [9u8; 16];
        let ip = [10, 0, 0, 1];
        assert!(st.store_keyword_entries(kw, vec![kw_with_aich(1, [0x11; 20])], ip));
        assert!(st.store_keyword_entries(kw, vec![kw_with_aich(1, [0x11; 20])], ip));
        let got = st.get_keyword_entries(&kw, 0);
        assert_eq!(votes_of(&got[0]), vec![(1, [0x11; 20])]);
    }

    #[test]
    fn common_filename_prefers_popular_name() {
        // eMule `GetCommonFileName`: three publishers agreeing on "good"
        // outvote one "bad" name.
        let mut st = KadStorage::default();
        let kw = [9u8; 16];
        let mk = |name: &str| {
            let mut e = kw_entry(1);
            e.file_name = name.to_string();
            e
        };
        assert!(st.store_keyword_entries(kw, vec![mk("bad.bin")], [10, 0, 0, 1]));
        assert!(st.store_keyword_entries(kw, vec![mk("good.bin")], [10, 0, 0, 2]));
        assert!(st.store_keyword_entries(kw, vec![mk("good.bin")], [10, 0, 0, 3]));
        let got = st.get_keyword_entries(&kw, 0);
        assert_eq!(got[0].file_name, "good.bin");
        assert_eq!((got[0].publish_info.unwrap() >> 24) & 0xFF, 2);
    }

    #[test]
    fn trusted_entries_served_first() {
        // eMule `SendValidKeywordResult` two-pass serving: entries whose
        // trust value >= 1.0 precede lower-trust ones regardless of
        // storage order.
        let mut st = KadStorage::default();
        let kw = [9u8; 16];
        // 11 publishers from the same /24 on distinct files: each
        // entry's trust = 10/11 < 1.
        for i in 0..11u8 {
            assert!(st.store_keyword_entries(
                kw,
                vec![kw_entry(i as u16 + 1)],
                [10, 0, 0, i + 1]
            ));
        }
        // One publisher from a fresh /24: trust = 10.
        assert!(st.store_keyword_entries(kw, vec![kw_entry(100)], [11, 0, 0, 1]));
        let got = st.get_keyword_entries(&kw, 0);
        assert_eq!(got.len(), 12);
        assert_eq!(
            got[0].file_hash,
            kw_entry(100).file_hash,
            "fresh-subnet entry first"
        );
    }

    #[test]
    fn publisher_cleanup_keeps_aich_popularity() {
        // eMule `CleanUpTrackedPublishers`: expired publishers leave the
        // AICH histogram untouched (only >100-publisher purges and
        // republish changes decrement it).
        let mut st = KadStorage::default();
        let kw = [9u8; 16];
        let root = [0x33u8; 20];
        assert!(st.store_keyword_entries(kw, vec![kw_with_aich(1, root)], [10, 0, 0, 1]));
        // Force the publisher tracker's timestamp into the past.
        let entries = st.keywords.get_mut(&kw).unwrap();
        entries[0].publishers[0].last_publish =
            Instant::now() - PUBLISHER_TTL - Duration::from_secs(1);
        let got = st.get_keyword_entries(&kw, 0);
        assert_eq!(votes_of(&got[0]), vec![(1, root)], "AICH vote survives expiry");
        // …but the publisher itself is gone: no PUBLISHINFO tag.
        assert!(got[0].publish_info.is_none());
    }

    #[test]
    fn same_filehash_different_size_coexists() {
        // eMule `AddKeyword`: same source id published with a different
        // size produces a second entry, not a merge.
        let mut st = KadStorage::default();
        let kw = [9u8; 16];
        let mut big = kw_entry(1);
        big.file_size = 9999;
        assert!(st.store_keyword_entries(kw, vec![kw_entry(1)], TEST_PUB_IP));
        assert!(st.store_keyword_entries(kw, vec![big], TEST_PUB_IP));
        let got = st.get_keyword_entries(&kw, 0);
        assert_eq!(got.len(), 2);
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
