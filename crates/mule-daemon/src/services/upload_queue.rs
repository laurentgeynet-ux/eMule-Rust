// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use crate::services::DaemonEvent;

/// Maximum number of queued upload requests.
/// eMule 0.72a `QueueSize` default (100 × QueueSizePref 50).
pub const MAX_UPLOAD_QUEUE: usize = 5000;
/// Maximum number of concurrent active uploads.
/// eMule 0.72a `MAX_UP_CLIENTS_ALLOWED` (Opcodes.h) — plafond absolu de
/// slots upload, quels que soient les autres réglages.
pub const MAX_ACTIVE_UPLOADS: usize = 100;
/// eMule `AcceptNewClient` : les `max(MIN_UP_CLIENTS_ALLOWED, 4)` = 4
/// premiers slots sont toujours accordés — le plancher de débit moyen
/// (`min_upload_rate`) ne peut pas geler l'upload quand des clients
/// attendent. Au-delà, chaque nouveau slot exige que la moyenne par slot
/// reste au-dessus du plancher.
const GUARANTEED_UPLOAD_SLOTS: usize = 4;

/// eMule trickle threshold: an active slot whose measured rate stays below
/// 1 KiB/s past the grace period is demoted to a trickle slot — it keeps a
/// residual share of bandwidth but no longer occupies a full upload slot
/// (`CUploadQueue` keeps slow clients alive while freeing the slot).
const TRICKLE_RATE_BPS: u64 = 1024;
/// Grace period before a slow slot may be demoted (60 s).
const TRICKLE_GRACE_MS: i64 = 60_000;

/// Fenêtre d'échantillonnage du débit upload (~2 s, cadence du statsTick) :
/// `upload_rate_bps` est recalculé à chaque fenêtre échue — même approche
/// que les débits de download mesurés par tranches de 500 ms dans
/// `peer_session/transfer.rs`.
const RATE_WINDOW_MS: i64 = 2_000;
/// Premier échantillon rapide (500 ms) : une session qui vient d'ouvrir sa
/// fenêtre n'attend pas 2 s avant d'afficher un débit réel.
const RATE_FIRST_SAMPLE_MS: i64 = 500;

/// State of an upload slot or queue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadSlotState {
    Waiting,
    Uploading,
    /// Slow client kept alive on residual bandwidth; does not count against
    /// `max_active` (eMule "trickle slot").
    Trickle,
    Completed,
    Rejected,
    Aborted,
}

/// Snapshot of an upload session / slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UploadSessionSnapshot {
    /// Identity key used for dedup: `uh:<userhash>` when the peer announced
    /// a user hash in OP_HELLO, otherwise its `ip:port` address.
    pub client_key: String,
    pub client_address: String,
    pub file_hash: String,
    pub file_name: String,
    pub transferred_bytes: u64,
    pub upload_rate_bps: u64,
    pub state: UploadSlotState,
    pub queue_rank: Option<u32>,
    pub started_at_unix_ms: i64,
    /// Début de la fenêtre de mesure courante (ms Unix, 0 = pas de mesure).
    /// État interne du `UploadManager` — non sérialisé.
    #[serde(skip)]
    pub rate_window_start_ms: i64,
    /// Octets envoyés dans la fenêtre courante. Non sérialisé.
    #[serde(skip)]
    pub rate_window_bytes: u64,
    /// `true` dès qu'une fenêtre de mesure a été refermée — requis pour la
    /// rétrogradation `Trickle` (un débit jamais mesuré n'est pas « lent »).
    /// Non sérialisé.
    #[serde(skip)]
    pub rate_sampled: bool,
}

impl UploadSessionSnapshot {
    /// Débit effectif à `now_ms` : le dernier débit échantillonné tant que
    /// la fenêtre courante reste fraîche ; au-delà de `RATE_WINDOW_MS` sans
    /// nouvel envoi, les octets résiduels de la fenêtre sont rapportés au
    /// temps écoulé — le débit décroît vers 0 quand le pair ne demande plus
    /// rien (même convention que `decay_stale_rates` côté downloads).
    pub fn effective_rate_bps(&self, now_ms: i64) -> u64 {
        let elapsed = now_ms.saturating_sub(self.rate_window_start_ms);
        if self.rate_window_start_ms == 0 || elapsed < RATE_WINDOW_MS {
            self.upload_rate_bps
        } else {
            self.rate_window_bytes.saturating_mul(1000) / elapsed as u64
        }
    }
}

/// Entry waiting in the upload queue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueuedUpload {
    /// Identity key (`uh:<userhash>` or `ip:port`) used for dedup/promotion.
    pub client_key: String,
    pub client_address: String,
    pub file_hash: String,
    pub file_name: String,
    pub enqueued_at_unix_ms: i64,
    pub credit_multiplier_bps: u32, // Multiplier * 100 (e.g. 100 for 1.0x, 1000 for 10.0x)
    /// eMule `GetFilePrioAsNumber` of the requested shared file:
    /// veryhigh=18, high=9, normal=7, low=6, verylow=2.
    #[serde(default = "default_file_prio_num")]
    pub file_prio_num: u32,
    /// eMule `IS_IDBADGUY`/`IsBanned` semantics: score is always 0, the entry
    /// waits forever at the tail (never promoted, still listed).
    #[serde(default)]
    pub banned: bool,
}

fn default_file_prio_num() -> u32 {
    7
}

/// Maps an upload priority string to the eMule `GetFilePrioAsNumber` weight
/// (`UploadClient.cpp`): veryhigh=18, high=9, normal=7, low=6, verylow=2.
pub fn file_prio_num(priority: &str) -> u32 {
    match priority.to_lowercase().as_str() {
        "veryhigh" | "powershare" => 18,
        "high" => 9,
        "low" => 6,
        "verylow" => 2,
        _ => 7,
    }
}

/// eMule downloading-client bonus: an actively uploading client that
/// re-queues gets its upload time counted as waiting time plus a 15 min
/// bonus (`GetScore(isdownloading)`), so short sessions cannot churn the
/// queue.
const DOWNLOADING_BONUS_MS: i64 = 15 * 60 * 1000;

impl QueuedUpload {
    /// Computes the eMule waiting score:
    /// `wait_secs * credit_multiplier * (file_prio / 10)`, 0 when banned.
    /// Stored in centi-units (`*_bps` fields keep two decimals).
    pub fn effective_score(&self, now_ms: i64) -> u64 {
        if self.banned {
            return 0;
        }
        let wait_secs = ((now_ms.saturating_sub(self.enqueued_at_unix_ms)) / 1000).max(1) as u64;
        let mult = (self.credit_multiplier_bps as u64).max(100);
        wait_secs * mult * self.file_prio_num as u64 / 1000
    }
}

/// Fair bounded upload queue and active slot tracker.
/// Supports up to `MAX_ACTIVE_UPLOADS` concurrent upload slots
/// (eMule grants several simultaneous upload slots).
#[derive(Clone)]
pub struct UploadManager {
    queue: Arc<Mutex<VecDeque<QueuedUpload>>>,
    active_uploads: Arc<Mutex<std::collections::HashMap<String, UploadSessionSnapshot>>>,
    active_count: Arc<AtomicUsize>,
    max_active: Arc<AtomicUsize>,
    /// USS `AcceptNewClient` gate — when false, new clients stay queued
    /// while latency is above tolerance (`UssService::accept_flag`).
    uss_gate: Arc<std::sync::RwLock<Arc<AtomicBool>>>,
    /// Optional event channel (set by `AppState`) for `upload.uploadsUpdated`.
    event_tx: Arc<std::sync::RwLock<Option<broadcast::Sender<String>>>>,
    sequence: Arc<std::sync::RwLock<Option<Arc<AtomicU64>>>>,
    /// Throttle for progress-driven emissions (~500 ms).
    last_progress_emit_ms: Arc<AtomicI64>,
    /// Friend client keys (`uh:<userhash>`) — granted a dedicated slot even
    /// when `max_active` is reached (eMule friend slot).
    friends: Arc<std::sync::RwLock<std::collections::HashSet<String>>>,
    /// Maximum waiting-queue length (eMule `QueueSize` pref, default 500).
    max_queue: Arc<AtomicUsize>,
    /// Minimum upload bandwidth per slot in bytes/sec (eMule `MinUpload`):
    /// a new slot is only opened when existing slots still reach it.
    min_upload_rate_bps: Arc<std::sync::atomic::AtomicU64>,
    /// Quota journalier d'upload (EMule-Rust `dailyUploadQuotaGb`) — quand
    /// `false`, aucune nouvelle demande de slot n'est acceptée.
    quota_gate: Arc<std::sync::RwLock<Arc<AtomicBool>>>,
    /// Total cumulatif des octets uploadés depuis le démarrage du daemon
    /// (compteur source du quota journalier).
    total_uploaded_bytes: Arc<AtomicU64>,
    /// Totaux uploadés par fichier (hash hex en minuscules) — hydratés
    /// depuis `share_upload_stats` au démarrage puis incrémentés à chaque
    /// bloc envoyé ; persistés par deltas dans la boucle `statsTick`.
    file_upload_totals: Arc<std::sync::RwLock<std::collections::HashMap<String, u64>>>,
}

impl Default for UploadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UploadManager {
    /// Creates a new `UploadManager`.
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_UPLOAD_QUEUE))),
            active_uploads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            active_count: Arc::new(AtomicUsize::new(0)),
            max_active: Arc::new(AtomicUsize::new(MAX_ACTIVE_UPLOADS)),
            uss_gate: Arc::new(std::sync::RwLock::new(Arc::new(AtomicBool::new(true)))),
            event_tx: Arc::new(std::sync::RwLock::new(None)),
            sequence: Arc::new(std::sync::RwLock::new(None)),
            last_progress_emit_ms: Arc::new(AtomicI64::new(0)),
            friends: Arc::new(std::sync::RwLock::new(std::collections::HashSet::new())),
            max_queue: Arc::new(AtomicUsize::new(MAX_UPLOAD_QUEUE)),
            min_upload_rate_bps: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            quota_gate: Arc::new(std::sync::RwLock::new(Arc::new(AtomicBool::new(true)))),
            total_uploaded_bytes: Arc::new(AtomicU64::new(0)),
            file_upload_totals: Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    /// Hydrate les totaux uploadés par fichier persistés en SQLite
    /// (`share_upload_stats`) — appelé une fois au démarrage.
    pub fn set_file_upload_totals(&self, totals: std::collections::HashMap<String, u64>) {
        *self.file_upload_totals.write().unwrap() = totals;
    }

    /// Copie des totaux uploadés par fichier (hash en minuscules).
    pub fn per_file_uploaded_bytes(&self) -> std::collections::HashMap<String, u64> {
        self.file_upload_totals.read().unwrap().clone()
    }

    /// Débit effectif courant agrégé par fichier (somme des slots actifs
    /// sur le même hash, clés en minuscules) — eMule affiche ce débit dans
    /// la colonne « Upload speed » de la liste des fichiers partagés.
    pub async fn per_file_rates_bps(&self) -> std::collections::HashMap<String, u64> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let active = self.active_uploads.lock().await;
        let mut rates = std::collections::HashMap::new();
        for s in active.values() {
            *rates.entry(s.file_hash.to_lowercase()).or_insert(0u64) +=
                s.effective_rate_bps(now);
        }
        rates
    }

    /// Sets the minimum per-slot upload rate (eMule `MinUpload`, B/s).
    /// `0` disables the floor.
    pub fn set_min_upload_rate(&self, bps: u64) {
        self.min_upload_rate_bps
            .store(bps, std::sync::atomic::Ordering::Relaxed);
    }

    /// Sets the maximum waiting-queue length (eMule `QueueSize` pref).
    pub fn set_max_queue(&self, max: usize) {
        self.max_queue
            .store(max.clamp(10, 5000), std::sync::atomic::Ordering::Relaxed);
    }

    /// Installs the friend client-key set (`uh:<userhash>` entries).
    pub fn set_friends(&self, friends: std::collections::HashSet<String>) {
        *self.friends.write().unwrap() = friends;
    }

    /// Adds/removes a friend client key at runtime (RPC-driven).
    pub fn set_friend(&self, client_key: &str, is_friend: bool) {
        let mut f = self.friends.write().unwrap();
        if is_friend {
            f.insert(client_key.to_string());
        } else {
            f.remove(client_key);
        }
    }

    /// True when `client_key` belongs to a configured friend.
    fn is_friend(&self, client_key: &str) -> bool {
        self.friends.read().unwrap().contains(client_key)
    }

    /// Installs the broadcast channel used to emit `upload.uploadsUpdated`.
    pub fn set_event_channel(&self, tx: broadcast::Sender<String>, seq: Arc<AtomicU64>) {
        *self.event_tx.write().unwrap() = Some(tx);
        *self.sequence.write().unwrap() = Some(seq);
    }

    /// Emits the full uploads snapshot (active slots + queue + aggregate
    /// rate). No-op when no event channel is installed. `pub` pour la
    /// poussée périodique depuis la boucle `statsTick` (`state.rs`) qui
    /// rafraîchit les débits mesurés entre deux envois de blocs.
    pub async fn emit_uploads_updated(&self) {
        let (tx, seq) = {
            let tx = self.event_tx.read().unwrap().clone();
            let seq = self.sequence.read().unwrap().clone();
            (tx, seq)
        };
        let (Some(tx), Some(seq)) = (tx, seq) else {
            return;
        };
        let active = self.active_snapshots().await;
        let queue = self.list_queue().await;
        let rate = active.iter().map(|s| s.upload_rate_bps).sum();
        DaemonEvent::UploadsUpdated {
            sequence: seq.fetch_add(1, Ordering::SeqCst),
            active,
            queue,
            upload_rate_bps: rate,
        }
        .send(&tx);
    }

    /// Emits `uploadsUpdated` from the hot progress path, throttled to at
    /// most once every 500 ms.
    async fn emit_uploads_updated_throttled(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let last = self.last_progress_emit_ms.load(Ordering::SeqCst);
        if now - last < 500 {
            return;
        }
        if self
            .last_progress_emit_ms
            .compare_exchange(last, now, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            self.emit_uploads_updated().await;
        }
    }

    /// Installs the USS accept-new-client flag
    /// (`LastCommonRouteFinder::AcceptNewClient` semantics).
    pub fn set_uss_gate(&self, flag: Arc<AtomicBool>) {
        *self.uss_gate.write().unwrap() = flag;
    }

    /// Installe le gate du quota journalier (`dailyUploadQuotaGb`) — `false`
    /// bloque toute nouvelle demande de slot.
    pub fn set_quota_gate(&self, flag: Arc<AtomicBool>) {
        *self.quota_gate.write().unwrap() = flag;
    }

    /// Octets uploadés cumulés depuis le démarrage du daemon.
    pub fn total_uploaded_bytes(&self) -> u64 {
        self.total_uploaded_bytes.load(Ordering::SeqCst)
    }

    /// Overrides the maximum number of concurrent active upload slots.
    pub fn set_max_active(&self, max: usize) {
        self.max_active.store(max.max(1), Ordering::SeqCst);
    }

    /// Number of currently active upload slots.
    pub fn active_len(&self) -> usize {
        self.active_count.load(Ordering::SeqCst)
    }

    /// Requests an upload slot for an inbound client.
    ///
    /// - If a slot is available, immediately grants `UploadSlotState::Uploading`.
    /// - If all slots are occupied, enqueues the client in the
    ///   credit-weighted queue and returns `UploadSlotState::Waiting` + rank.
    /// - Returns an error if queue capacity is exceeded.
    ///
    /// `client_key` is the identity used for dedup/release — `uh:<userhash>`
    /// when the peer announced a user hash, otherwise `client_address`.
    /// `client_address` stays the display/UDP-lookup address (`ip:port`).
    pub async fn request_upload_slot(
        &self,
        client_key: &str,
        client_address: &str,
        file_hash: &str,
        file_name: &str,
        credit_multiplier: f64,
        file_prio: &str,
        banned: bool,
    ) -> Result<(UploadSlotState, Option<u32>), String> {
        let mut active = self.active_uploads.lock().await;

        // Check if client is already an active upload
        if let Some(cur) = active.get(client_key) {
            if cur.file_hash == file_hash {
                return Ok((UploadSlotState::Uploading, None));
            }
        }

        // Quota journalier atteint (`dailyUploadQuotaGb`) : aucune nouvelle
        // demande n'est acceptée jusqu'à la remise à zéro du compteur.
        if !self.quota_gate.read().unwrap().load(Ordering::SeqCst) {
            return Err("Daily upload quota reached".to_string());
        }

        // Grant a slot immediately while capacity remains and USS accepts
        // new clients (acceptNewClient=false when latency is too high).
        // Trickle slots keep residual bandwidth but do not occupy a full slot.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        if self.can_open_slot(&active, now) {
            let snap = UploadSessionSnapshot {
                client_key: client_key.to_string(),
                client_address: client_address.to_string(),
                file_hash: file_hash.to_string(),
                file_name: file_name.to_string(),
                transferred_bytes: 0,
                upload_rate_bps: 0,
                state: UploadSlotState::Uploading,
                queue_rank: None,
                started_at_unix_ms: now,
                rate_window_start_ms: 0,
                rate_window_bytes: 0,
                rate_sampled: false,
            };
            active.insert(client_key.to_string(), snap);
            self.active_count
                .store(Self::uploading_count(&active), Ordering::SeqCst);
            tracing::info!(
                "[UPLOAD_SLOT_GRANTED] Client {} granted active upload slot for file '{}' (hash: {}) ({}/{} slots used)",
                client_address, file_name, file_hash, Self::uploading_count(&active), self.max_active.load(Ordering::SeqCst)
            );
            drop(active);
            self.emit_uploads_updated().await;
            return Ok((UploadSlotState::Uploading, None));
        }

        // eMule friend slot: a friend bypasses the full queue and receives a
        // dedicated extra slot beyond `max_active`.
        if self.is_friend(client_key) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            let snap = UploadSessionSnapshot {
                client_key: client_key.to_string(),
                client_address: client_address.to_string(),
                file_hash: file_hash.to_string(),
                file_name: file_name.to_string(),
                transferred_bytes: 0,
                upload_rate_bps: 0,
                state: UploadSlotState::Uploading,
                queue_rank: None,
                started_at_unix_ms: now,
                rate_window_start_ms: 0,
                rate_window_bytes: 0,
                rate_sampled: false,
            };
            active.insert(client_key.to_string(), snap);
            tracing::info!(
                "[UPLOAD_FRIEND_SLOT] Friend {} granted dedicated upload slot for file '{}' ({} full slots)",
                client_address, file_name, Self::uploading_count(&active)
            );
            drop(active);
            self.emit_uploads_updated().await;
            return Ok((UploadSlotState::Uploading, None));
        }

        // All active slots occupied: enqueue if space available
        let mut q = self.queue.lock().await;
        let mult_bps = (credit_multiplier.clamp(1.0, 10.0) * 100.0) as u32;

        if let Some(pos) = q
            .iter()
            .position(|entry| entry.client_key == client_key && entry.file_hash == file_hash)
        {
            return Ok((UploadSlotState::Waiting, Some((pos + 1) as u32)));
        }

        // Minimal anti-leecher: when the queue is ≥90% full, clients that
        // never identified themselves with a user hash (`uh:` key) are
        // rejected instead of queued — identifiable clients keep priority
        // (eMule prefers accountable identities under contention).
        let max_queue = self.max_queue.load(std::sync::atomic::Ordering::Relaxed);
        if !client_key.starts_with("uh:") && q.len() >= max_queue * 9 / 10 {
            return Err("Upload queue nearly full for unidentified clients".to_string());
        }

        if q.len() >= max_queue {
            return Err(format!("Upload queue full ({max_queue} max)"));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        // eMule downloading-client bonus: a client currently uploading in
        // another slot re-queues with its upload time + 15 min counted as
        // waiting time (prevents "20-second downloads" churn).
        let enqueued_at = if active.contains_key(client_key) {
            now - DOWNLOADING_BONUS_MS
        } else {
            now
        };

        q.push_back(QueuedUpload {
            client_key: client_key.to_string(),
            client_address: client_address.to_string(),
            file_hash: file_hash.to_string(),
            file_name: file_name.to_string(),
            enqueued_at_unix_ms: enqueued_at,
            credit_multiplier_bps: mult_bps,
            file_prio_num: file_prio_num(file_prio),
            banned,
        });

        // Sort queue by effective score descending
        let now_copy = now;
        let mut entries: Vec<QueuedUpload> = q.drain(..).collect();
        entries.sort_by_key(|a| std::cmp::Reverse(a.effective_score(now_copy)));
        for e in entries {
            q.push_back(e);
        }

        let rank = q
            .iter()
            .position(|entry| entry.client_key == client_key && entry.file_hash == file_hash)
            .map(|pos| (pos + 1) as u32)
            .unwrap_or(q.len() as u32);

        tracing::info!(
            "[UPLOAD_QUEUED] Client {} queued for file '{}' (hash: {}) with rank #{} (credits mult: {:.2}x, queue size: {})",
            client_address, file_name, file_hash, rank, credit_multiplier, q.len()
        );
        drop(q);
        drop(active);
        self.emit_uploads_updated().await;

        Ok((UploadSlotState::Waiting, Some(rank)))
    }

    /// Number of slots currently occupying a full upload slot — trickle
    /// slots are excluded (eMule counts only full-speed uploads).
    fn uploading_count(
        active: &std::collections::HashMap<String, UploadSessionSnapshot>,
    ) -> usize {
        active
            .values()
            .filter(|s| s.state == UploadSlotState::Uploading)
            .count()
    }

    /// eMule `CUploadQueue::AcceptNewClient` : ouvre un slot si
    /// `uploading < max_active` **et** le veto USS est levé **et** soit
    /// les `GUARANTEED_UPLOAD_SLOTS` premiers slots (`max(MIN_UP_CLIENTS_ALLOWED, 4)`),
    /// soit la moyenne de débit par slot reste ≥ `min_upload_rate`.
    /// Partagé par `request_upload` (nouveau client) et `promote_waiting`
    /// (promotion de file) pour éviter la divergence des deux chemins.
    fn can_open_slot(
        &self,
        active: &std::collections::HashMap<String, UploadSessionSnapshot>,
        now: i64,
    ) -> bool {
        let uploading = Self::uploading_count(active);
        if uploading >= self.max_active.load(Ordering::SeqCst) {
            return false;
        }
        if !self.uss_gate.read().unwrap().load(Ordering::SeqCst) {
            return false;
        }
        if uploading < GUARANTEED_UPLOAD_SLOTS {
            return true;
        }
        let min_rate = self.min_upload_rate_bps.load(Ordering::Relaxed);
        if min_rate == 0 {
            return true;
        }
        let total_bps: u64 = active
            .values()
            .filter(|s| s.state == UploadSlotState::Uploading)
            .map(|s| s.effective_rate_bps(now))
            .sum();
        total_bps / uploading.max(1) as u64 >= min_rate
    }

    /// Promotes waiting clients while full slots remain. Caller must hold
    /// the `active_uploads` lock and pass it in.
    async fn promote_waiting(
        &self,
        active: &mut std::collections::HashMap<String, UploadSessionSnapshot>,
    ) {
        let mut q = self.queue.lock().await;
        loop {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            // Même éligibilité qu'`AcceptNewClient` : plancher de débit
            // par slot et veto USS s'appliquent aussi à la promotion.
            if !self.can_open_slot(active, now) {
                break;
            }
            let Some(next) = q.pop_front() else {
                break;
            };
            if active.contains_key(&next.client_key) {
                continue;
            }

            let snap = UploadSessionSnapshot {
                client_key: next.client_key.clone(),
                client_address: next.client_address.clone(),
                file_hash: next.file_hash,
                file_name: next.file_name,
                transferred_bytes: 0,
                upload_rate_bps: 0,
                state: UploadSlotState::Uploading,
                queue_rank: None,
                started_at_unix_ms: now,
                rate_window_start_ms: 0,
                rate_window_bytes: 0,
                rate_sampled: false,
            };
            tracing::info!(
                "[UPLOAD_PROMOTED] Client {} promoted from queue to active upload slot for file '{}' (hash: {})",
                snap.client_address, snap.file_name, snap.file_hash
            );
            active.insert(next.client_key, snap);
        }
        self.active_count
            .store(Self::uploading_count(active), Ordering::SeqCst);
    }

    /// Updates active upload progress and measures the slot's real rate.
    ///
    /// Le débit est mesuré en interne par fenêtres (`RATE_WINDOW_MS`,
    /// premier échantillon à `RATE_FIRST_SAMPLE_MS`) à partir des octets
    /// réellement envoyés — l'appelant ne fournit que `bytes_sent`, jamais
    /// un débit estimé. `effective_rate_bps` décroît vers 0 quand le pair
    /// cesse de demander des blocs.
    ///
    /// Slot focusing (eMule semantics): an active slot whose measured rate
    /// stays under `TRICKLE_RATE_BPS` past `TRICKLE_GRACE_MS` is demoted to
    /// `Trickle` — it keeps residual bandwidth but frees its slot so the
    /// best queued client is promoted. This concentrates bandwidth on fast
    /// clients.
    pub async fn update_progress(&self, client_key: &str, bytes_sent: u64) {
        let mut demoted = false;
        {
            let mut active = self.active_uploads.lock().await;
            if let Some(cur) = active.get_mut(client_key) {
                cur.transferred_bytes = cur.transferred_bytes.saturating_add(bytes_sent);
                self.total_uploaded_bytes
                    .fetch_add(bytes_sent, Ordering::SeqCst);
                *self
                    .file_upload_totals
                    .write()
                    .unwrap()
                    .entry(cur.file_hash.to_lowercase())
                    .or_default() += bytes_sent;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                if cur.rate_window_start_ms == 0 {
                    cur.rate_window_start_ms = now;
                }
                cur.rate_window_bytes = cur.rate_window_bytes.saturating_add(bytes_sent);
                let elapsed = now.saturating_sub(cur.rate_window_start_ms);
                // Referme la fenêtre échue — ou dès 500 ms pour le premier
                // échantillon — et mesure le débit réellement desservi.
                if elapsed >= RATE_WINDOW_MS
                    || (!cur.rate_sampled && elapsed >= RATE_FIRST_SAMPLE_MS)
                {
                    cur.upload_rate_bps =
                        cur.rate_window_bytes.saturating_mul(1000) / elapsed as u64;
                    cur.rate_window_start_ms = now;
                    cur.rate_window_bytes = 0;
                    cur.rate_sampled = true;
                }
                if cur.state == UploadSlotState::Uploading
                    && cur.rate_sampled
                    && cur.upload_rate_bps < TRICKLE_RATE_BPS
                    && now.saturating_sub(cur.started_at_unix_ms) > TRICKLE_GRACE_MS
                {
                    cur.state = UploadSlotState::Trickle;
                    demoted = true;
                    tracing::info!(
                        "[UPLOAD_TRICKLE] Client {} demoted to trickle slot (rate {} B/s < {} B/s) — slot freed for queue",
                        cur.client_address, cur.upload_rate_bps, TRICKLE_RATE_BPS
                    );
                }
            }
            if demoted {
                self.promote_waiting(&mut active).await;
            }
        }
        if demoted {
            self.emit_uploads_updated().await;
        } else {
            self.emit_uploads_updated_throttled().await;
        }
    }

    /// Releases the client's active upload slot and promotes waiting clients
    /// until all slots are filled again. Accepts either the `client_key`
    /// (`uh:<userhash>` or `ip:port`) or a bare `ip:port` address — the
    /// latter resolves through the stored `client_address` so RPC callers
    /// that only know the address keep working.
    pub async fn release_slot(&self, client_key: &str) {
        let mut active = self.active_uploads.lock().await;
        let resolved_key = if active.contains_key(client_key) {
            client_key.to_string()
        } else {
            active
                .iter()
                .find(|(_, s)| s.client_address == client_key)
                .map(|(k, _)| k.clone())
                .unwrap_or_else(|| client_key.to_string())
        };
        if let Some(cur) = active.remove(&resolved_key) {
            tracing::info!(
                "[UPLOAD_RELEASED] Active upload slot released by client {} for file '{}' (transferred: {} bytes)",
                resolved_key, cur.file_name, cur.transferred_bytes
            );
        }

        // Also remove from waiting queue if queued
        {
            let mut q = self.queue.lock().await;
            q.retain(|entry| {
                entry.client_key != resolved_key && entry.client_address != client_key
            });
        }

        // Promote from queue while free slots remain
        self.promote_waiting(&mut active).await;
        drop(active);
        self.emit_uploads_updated().await;
    }

    /// Clears queue and active slots on kill switch or network reset.
    pub async fn abort_all(&self) {
        let mut active = self.active_uploads.lock().await;
        active.clear();
        self.active_count.store(0, Ordering::SeqCst);
        let mut q = self.queue.lock().await;
        q.clear();
        drop(q);
        drop(active);
        self.emit_uploads_updated().await;
    }

    /// Returns the snapshot of the first active upload (if any).
    /// `upload_rate_bps` y est le débit effectif à l'instant de la lecture.
    pub async fn active_snapshot(&self) -> Option<UploadSessionSnapshot> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let active = self.active_uploads.lock().await;
        active.values().next().cloned().map(|mut s| {
            s.upload_rate_bps = s.effective_rate_bps(now);
            s
        })
    }

    /// Returns snapshots of all active upload slots.
    /// `upload_rate_bps` y est le débit effectif à l'instant de la lecture
    /// (décroît vers 0 quand un pair ne demande plus de blocs).
    pub async fn active_snapshots(&self) -> Vec<UploadSessionSnapshot> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let active = self.active_uploads.lock().await;
        active
            .values()
            .cloned()
            .map(|mut s| {
                s.upload_rate_bps = s.effective_rate_bps(now);
                s
            })
            .collect()
    }

    /// Returns the 1-based queue position of a waiting client matched by
    /// IP address (`CUploadQueue::GetWaitingClientByIP_UDP` semantics:
    /// UDP pings carry only the sender IP).
    pub async fn waiting_rank_by_ip(&self, ip: &std::net::IpAddr) -> Option<u32> {
        let ip_str = ip.to_string();
        let q = self.queue.lock().await;
        q.iter()
            .position(|entry| {
                entry
                    .client_address
                    .rsplit_once(':')
                    .map(|(host, _)| host == ip_str)
                    .unwrap_or(false)
            })
            .map(|pos| (pos + 1) as u32)
    }

    /// Number of clients currently waiting in the upload queue.
    pub async fn waiting_len(&self) -> usize {
        self.queue.lock().await.len()
    }

    /// True when the waiting queue is within 50 entries of capacity —
    /// eMule answers `OP_QUEUEFULL` to UDP pings from unknown clients in
    /// that state (`GetWaitingUserCount() + 50 > GetQueueSize()`).
    pub async fn is_nearly_full(&self) -> bool {
        self.waiting_len().await + 50
            > self.max_queue.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Returns list of all queued upload snapshots.
    pub async fn list_queue(&self) -> Vec<UploadSessionSnapshot> {
        let q = self.queue.lock().await;
        q.iter()
            .enumerate()
            .map(|(idx, item)| UploadSessionSnapshot {
                client_key: item.client_key.clone(),
                client_address: item.client_address.clone(),
                file_hash: item.file_hash.clone(),
                file_name: item.file_name.clone(),
                transferred_bytes: 0,
                upload_rate_bps: 0,
                state: UploadSlotState::Waiting,
                queue_rank: Some((idx + 1) as u32),
                started_at_unix_ms: item.enqueued_at_unix_ms,
                rate_window_start_ms: 0,
                rate_window_bytes: 0,
                rate_sampled: false,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn multiple_active_slots_up_to_limit() {
        let mgr = UploadManager::new();
        mgr.set_max_active(3);
        for i in 0..3 {
            let (state, _) = mgr
                .request_upload_slot(&format!("c{i}"), &format!("c{i}"), "h", "f", 1.0, "normal", false)
                .await
                .unwrap();
            assert_eq!(state, UploadSlotState::Uploading);
        }
        // 4th client queues
        let (state, rank) = mgr
            .request_upload_slot("c3", "c3", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
        assert_eq!(rank, Some(1));
        assert_eq!(mgr.active_snapshots().await.len(), 3);
    }

    #[tokio::test]
    async fn release_promotes_until_slots_full() {
        let mgr = UploadManager::new();
        mgr.set_max_active(2);
        mgr.request_upload_slot("a", "a", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        mgr.request_upload_slot("b", "b", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        mgr.request_upload_slot("c", "c", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        mgr.request_upload_slot("d", "d", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();

        mgr.release_slot("a").await;
        let actives = mgr.active_snapshots().await;
        assert_eq!(actives.len(), 2);
        assert!(actives.iter().any(|s| s.client_address == "c"));
        assert_eq!(mgr.list_queue().await.len(), 1);

        mgr.release_slot("b").await;
        assert!(mgr
            .active_snapshots()
            .await
            .iter()
            .any(|s| s.client_address == "d"));
        assert!(mgr.list_queue().await.is_empty());
    }

    #[tokio::test]
    async fn userhash_key_dedups_across_ips_and_release_by_address_works() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        // Same userhash reconnecting from a different IP is the same client.
        mgr.request_upload_slot("uh:AA", "1.1.1.1:4662", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        let (state, _) = mgr
            .request_upload_slot("uh:AA", "2.2.2.2:4662", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Uploading);
        // release_slot by bare ip:port resolves via client_address.
        mgr.release_slot("1.1.1.1:4662").await;
        assert_eq!(mgr.active_len(), 0);
    }

    #[tokio::test]
    async fn slow_slot_demoted_to_trickle_and_promotes_queue() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        // c1 takes the only slot.
        let (state, _) = mgr
            .request_upload_slot("c1", "c1", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Uploading);
        // c2 waits.
        let (state, _) = mgr
            .request_upload_slot("c2", "c2", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
        // c1 stays slow past the grace period → trickle → c2 promoted.
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        {
            let mut active = mgr.active_uploads.lock().await;
            let s = active.get_mut("c1").unwrap();
            s.started_at_unix_ms -= TRICKLE_GRACE_MS + 1;
            // Fenêtre de mesure amorcée il y a 1,5 s : le premier
            // échantillon (≥ 500 ms) mesure ~341 B/s < TRICKLE_RATE_BPS.
            s.rate_window_start_ms = now_ms - 1_500;
        }
        mgr.update_progress("c1", 512).await;
        let actives = mgr.active_snapshots().await;
        assert_eq!(
            actives.iter().find(|s| s.client_key == "c1").unwrap().state,
            UploadSlotState::Trickle
        );
        assert_eq!(
            actives.iter().find(|s| s.client_key == "c2").unwrap().state,
            UploadSlotState::Uploading
        );
        assert_eq!(mgr.active_len(), 1, "trickle does not occupy a full slot");
    }

    #[tokio::test]
    async fn per_file_stats_aggregate_sessions_and_persist_totals() {
        let mgr = UploadManager::new();
        // Hydratation au boot : le fichier a déjà 1000 B uploadés.
        mgr.set_file_upload_totals(std::collections::HashMap::from([(
            "haa".to_string(),
            1000u64,
        )]));
        // Deux slots actifs sur le même hash (casse mixte) + un autre fichier.
        mgr.request_upload_slot("c1", "c1", "HAA", "f", 1.0, "normal", false)
            .await
            .unwrap();
        mgr.request_upload_slot("c2", "c2", "haa", "f", 1.0, "normal", false)
            .await
            .unwrap();
        mgr.request_upload_slot("c3", "c3", "hbb", "g", 1.0, "normal", false)
            .await
            .unwrap();
        // Fenêtre de mesure amorcée il y a 1 s → premier échantillon au
        // prochain update_progress (elapsed ≥ RATE_FIRST_SAMPLE_MS).
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        {
            let mut active = mgr.active_uploads.lock().await;
            for s in active.values_mut() {
                s.rate_window_start_ms = now - 1_000;
            }
        }
        mgr.update_progress("c1", 1500).await;
        mgr.update_progress("c2", 1500).await;
        mgr.update_progress("c3", 1500).await;

        let totals = mgr.per_file_uploaded_bytes();
        assert_eq!(totals["haa"], 4000, "hydraté 1000 + 1500 + 1500");
        assert_eq!(totals["hbb"], 1500);
        assert!(!totals.contains_key("hcc"));

        let rates = mgr.per_file_rates_bps().await;
        // ~1500 B/s par slot → ~3000 B/s agrégé pour haa, ~1500 pour hbb.
        let haa = rates["haa"];
        let hbb = rates["hbb"];
        assert!(haa > hbb, "deux slots agrégés ({haa}) > un slot ({hbb})");
        assert!(haa > 0 && hbb > 0);
        assert!(!rates.contains_key("hcc"));

        // Slots libérés : débit retombe, total persisté conservé.
        mgr.release_slot("c1").await;
        mgr.release_slot("c2").await;
        assert_eq!(
            mgr.per_file_rates_bps().await.get("haa").copied().unwrap_or(0),
            0
        );
        assert_eq!(mgr.per_file_uploaded_bytes()["haa"], 4000);
    }

    #[tokio::test]
    async fn fast_slot_within_grace_is_not_demoted() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        mgr.request_upload_slot("c1", "c1", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        // Slow rate but still within grace period → stays Uploading.
        mgr.update_progress("c1", 512).await;
        let s = &mgr.active_snapshots().await[0];
        assert_eq!(s.state, UploadSlotState::Uploading);
    }

    #[tokio::test]
    async fn uss_gate_keeps_new_clients_queued() {
        let mgr = UploadManager::new();
        let flag = Arc::new(AtomicBool::new(false));
        mgr.set_uss_gate(flag.clone());
        // Gate closed: free slots exist but the client must queue.
        let (state, rank) = mgr
            .request_upload_slot("a", "a", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
        assert_eq!(rank, Some(1));
        assert_eq!(mgr.active_len(), 0);
        // Gate open: next request is granted immediately.
        flag.store(true, Ordering::SeqCst);
        let (state, _) = mgr
            .request_upload_slot("b", "b", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Uploading);
    }

    #[tokio::test]
    async fn events_emitted_on_grant_queue_release() {
        let mgr = UploadManager::new();
        let (tx, mut rx) = broadcast::channel(16);
        mgr.set_event_channel(tx, Arc::new(AtomicU64::new(1)));

        mgr.set_max_active(1);
        mgr.request_upload_slot("a", "a", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(ev.contains("upload.uploadsUpdated"));
        assert!(ev.contains("\"uploadRateBps\""));

        mgr.request_upload_slot("b", "b", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(ev.contains("upload.uploadsUpdated"));
        assert!(ev.contains("queue"));

        mgr.release_slot("a").await;
        let ev = rx.recv().await.unwrap();
        assert!(ev.contains("upload.uploadsUpdated"));
    }

    #[tokio::test]
    async fn progress_tracked_per_client() {
        let mgr = UploadManager::new();
        mgr.request_upload_slot("a", "a", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        mgr.request_upload_slot("b", "b", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        mgr.update_progress("a", 100).await;
        mgr.update_progress("b", 200).await;
        let actives = mgr.active_snapshots().await;
        assert_eq!(
            actives
                .iter()
                .find(|s| s.client_address == "a")
                .unwrap()
                .transferred_bytes,
            100
        );
        assert_eq!(
            actives
                .iter()
                .find(|s| s.client_address == "b")
                .unwrap()
                .transferred_bytes,
            200
        );
    }

    #[tokio::test]
    async fn rate_is_measured_per_window_and_decays_on_stall() {
        let mgr = UploadManager::new();
        mgr.request_upload_slot("a", "a", "h", "f", 1.0, "normal", false)
            .await
            .unwrap();
        let now_ms = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64
        };
        // Fenêtre amorcée il y a 1 s : 200 KiB envoyés → premier échantillon
        // (≥ 500 ms) ≈ 200 KiB/s — le débit mesuré reflète les octets
        // réellement servis, pas la limite configurée.
        {
            let mut active = mgr.active_uploads.lock().await;
            active.get_mut("a").unwrap().rate_window_start_ms = now_ms() - 1_000;
        }
        mgr.update_progress("a", 200 * 1024).await;
        let snaps = mgr.active_snapshots().await;
        let rate = snaps[0].upload_rate_bps;
        assert!(
            (150 * 1024..=250 * 1024).contains(&rate),
            "measured rate should be ~200 KiB/s, got {rate}"
        );
        // Pair inactif depuis > 2 s : le débit effectif retombe à 0 au lieu
        // de rester figé sur la dernière valeur.
        assert_eq!(snaps[0].effective_rate_bps(now_ms() + 10_000), 0);
    }
}
