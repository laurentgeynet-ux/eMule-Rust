use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use crate::services::DaemonEvent;

/// Maximum number of queued upload requests.
pub const MAX_UPLOAD_QUEUE: usize = 500;
/// Maximum number of concurrent active uploads.
pub const MAX_ACTIVE_UPLOADS: usize = 10;

/// eMule trickle threshold: an active slot whose measured rate stays below
/// 1 KiB/s past the grace period is demoted to a trickle slot — it keeps a
/// residual share of bandwidth but no longer occupies a full upload slot
/// (`CUploadQueue` keeps slow clients alive while freeing the slot).
const TRICKLE_RATE_BPS: u64 = 1024;
/// Grace period before a slow slot may be demoted (60 s).
const TRICKLE_GRACE_MS: i64 = 60_000;

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
}

impl QueuedUpload {
    /// Computes effective waiting score = wait_time_sec * multiplier.
    pub fn effective_score(&self, now_ms: i64) -> u64 {
        let wait_secs = ((now_ms.saturating_sub(self.enqueued_at_unix_ms)) / 1000).max(1) as u64;
        let mult = (self.credit_multiplier_bps as u64).max(100);
        wait_secs * mult
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
        }
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
    /// rate). No-op when no event channel is installed.
    async fn emit_uploads_updated(&self) {
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
    ) -> Result<(UploadSlotState, Option<u32>), String> {
        let mut active = self.active_uploads.lock().await;

        // Check if client is already an active upload
        if let Some(cur) = active.get(client_key) {
            if cur.file_hash == file_hash {
                return Ok((UploadSlotState::Uploading, None));
            }
        }

        // Grant a slot immediately while capacity remains and USS accepts
        // new clients (acceptNewClient=false when latency is too high).
        // Trickle slots keep residual bandwidth but do not occupy a full slot.
        if Self::uploading_count(&active) < self.max_active.load(Ordering::SeqCst)
            && self.uss_gate.read().unwrap().load(Ordering::SeqCst)
        {
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
        if !client_key.starts_with("uh:") && q.len() >= MAX_UPLOAD_QUEUE * 9 / 10 {
            return Err("Upload queue nearly full for unidentified clients".to_string());
        }

        if q.len() >= MAX_UPLOAD_QUEUE {
            return Err(format!("Upload queue full ({MAX_UPLOAD_QUEUE} max)"));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        q.push_back(QueuedUpload {
            client_key: client_key.to_string(),
            client_address: client_address.to_string(),
            file_hash: file_hash.to_string(),
            file_name: file_name.to_string(),
            enqueued_at_unix_ms: now,
            credit_multiplier_bps: mult_bps,
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

    /// Promotes waiting clients while full slots remain. Caller must hold
    /// the `active_uploads` lock and pass it in.
    async fn promote_waiting(
        &self,
        active: &mut std::collections::HashMap<String, UploadSessionSnapshot>,
    ) {
        let mut q = self.queue.lock().await;
        while Self::uploading_count(active) < self.max_active.load(Ordering::SeqCst) {
            let Some(next) = q.pop_front() else {
                break;
            };
            if active.contains_key(&next.client_key) {
                continue;
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

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

    /// Updates active upload progress.
    ///
    /// Slot focusing (eMule semantics): an active slot whose rate stays under
    /// `TRICKLE_RATE_BPS` past `TRICKLE_GRACE_MS` is demoted to `Trickle` —
    /// it keeps residual bandwidth but frees its slot so the best queued
    /// client is promoted. This concentrates bandwidth on fast clients.
    pub async fn update_progress(&self, client_key: &str, bytes_sent: u64, rate_bps: u64) {
        let mut demoted = false;
        {
            let mut active = self.active_uploads.lock().await;
            if let Some(cur) = active.get_mut(client_key) {
                cur.transferred_bytes = cur.transferred_bytes.saturating_add(bytes_sent);
                cur.upload_rate_bps = rate_bps;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                if cur.state == UploadSlotState::Uploading
                    && rate_bps < TRICKLE_RATE_BPS
                    && now.saturating_sub(cur.started_at_unix_ms) > TRICKLE_GRACE_MS
                {
                    cur.state = UploadSlotState::Trickle;
                    demoted = true;
                    tracing::info!(
                        "[UPLOAD_TRICKLE] Client {} demoted to trickle slot (rate {} B/s < {} B/s) — slot freed for queue",
                        cur.client_address, rate_bps, TRICKLE_RATE_BPS
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
    pub async fn active_snapshot(&self) -> Option<UploadSessionSnapshot> {
        let active = self.active_uploads.lock().await;
        active.values().next().cloned()
    }

    /// Returns snapshots of all active upload slots.
    pub async fn active_snapshots(&self) -> Vec<UploadSessionSnapshot> {
        let active = self.active_uploads.lock().await;
        active.values().cloned().collect()
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
        self.waiting_len().await + 50 > MAX_UPLOAD_QUEUE
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
                .request_upload_slot(&format!("c{i}"), &format!("c{i}"), "h", "f", 1.0)
                .await
                .unwrap();
            assert_eq!(state, UploadSlotState::Uploading);
        }
        // 4th client queues
        let (state, rank) = mgr
            .request_upload_slot("c3", "c3", "h", "f", 1.0)
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
        mgr.request_upload_slot("a", "a", "h", "f", 1.0)
            .await
            .unwrap();
        mgr.request_upload_slot("b", "b", "h", "f", 1.0)
            .await
            .unwrap();
        mgr.request_upload_slot("c", "c", "h", "f", 1.0)
            .await
            .unwrap();
        mgr.request_upload_slot("d", "d", "h", "f", 1.0)
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
        mgr.request_upload_slot("uh:AA", "1.1.1.1:4662", "h", "f", 1.0)
            .await
            .unwrap();
        let (state, _) = mgr
            .request_upload_slot("uh:AA", "2.2.2.2:4662", "h", "f", 1.0)
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
            .request_upload_slot("c1", "c1", "h", "f", 1.0)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Uploading);
        // c2 waits.
        let (state, _) = mgr
            .request_upload_slot("c2", "c2", "h", "f", 1.0)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
        // c1 stays slow past the grace period → trickle → c2 promoted.
        {
            let mut active = mgr.active_uploads.lock().await;
            let s = active.get_mut("c1").unwrap();
            s.started_at_unix_ms -= TRICKLE_GRACE_MS + 1;
        }
        mgr.update_progress("c1", 512, 512).await;
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
    async fn fast_slot_within_grace_is_not_demoted() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        mgr.request_upload_slot("c1", "c1", "h", "f", 1.0)
            .await
            .unwrap();
        // Slow rate but still within grace period → stays Uploading.
        mgr.update_progress("c1", 512, 512).await;
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
            .request_upload_slot("a", "a", "h", "f", 1.0)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
        assert_eq!(rank, Some(1));
        assert_eq!(mgr.active_len(), 0);
        // Gate open: next request is granted immediately.
        flag.store(true, Ordering::SeqCst);
        let (state, _) = mgr
            .request_upload_slot("b", "b", "h", "f", 1.0)
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
        mgr.request_upload_slot("a", "a", "h", "f", 1.0)
            .await
            .unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(ev.contains("upload.uploadsUpdated"));
        assert!(ev.contains("\"uploadRateBps\""));

        mgr.request_upload_slot("b", "b", "h", "f", 1.0)
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
        mgr.request_upload_slot("a", "a", "h", "f", 1.0)
            .await
            .unwrap();
        mgr.request_upload_slot("b", "b", "h", "f", 1.0)
            .await
            .unwrap();
        mgr.update_progress("a", 100, 10).await;
        mgr.update_progress("b", 200, 20).await;
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
}
