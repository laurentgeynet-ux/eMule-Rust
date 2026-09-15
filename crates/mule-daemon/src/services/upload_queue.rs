use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Maximum number of queued upload requests.
pub const MAX_UPLOAD_QUEUE: usize = 10;
/// Maximum number of concurrent active uploads.
pub const MAX_ACTIVE_UPLOADS: usize = 1;

/// State of an upload slot or queue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadSlotState {
    Waiting,
    Uploading,
    Completed,
    Rejected,
    Aborted,
}

/// Snapshot of an upload session / slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UploadSessionSnapshot {
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
#[derive(Clone)]
pub struct UploadManager {
    queue: Arc<Mutex<VecDeque<QueuedUpload>>>,
    active_upload: Arc<Mutex<Option<UploadSessionSnapshot>>>,
    active_count: Arc<AtomicUsize>,
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
            active_upload: Arc::new(Mutex::new(None)),
            active_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Requests an upload slot for an inbound client.
    ///
    /// - If the single active slot is available, immediately grants `UploadSlotState::Uploading`.
    /// - If the slot is occupied, enqueues the client in the FIFO/credit-weighted queue and returns `UploadSlotState::Waiting` with rank.
    /// - Returns an error if queue capacity is exceeded.
    pub async fn request_upload_slot(
        &self,
        client_address: &str,
        file_hash: &str,
        file_name: &str,
        credit_multiplier: f64,
    ) -> Result<(UploadSlotState, Option<u32>), String> {
        let mut active = self.active_upload.lock().await;

        // Check if client is already the active upload
        if let Some(ref cur) = *active {
            if cur.client_address == client_address && cur.file_hash == file_hash {
                return Ok((UploadSlotState::Uploading, None));
            }
        }

        // If no active upload, assign slot immediately
        if active.is_none() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            let snap = UploadSessionSnapshot {
                client_address: client_address.to_string(),
                file_hash: file_hash.to_string(),
                file_name: file_name.to_string(),
                transferred_bytes: 0,
                upload_rate_bps: 0,
                state: UploadSlotState::Uploading,
                queue_rank: None,
                started_at_unix_ms: now,
            };
            *active = Some(snap);
            self.active_count.store(1, Ordering::SeqCst);
            return Ok((UploadSlotState::Uploading, None));
        }

        // Active slot is occupied: enqueue if space available
        let mut q = self.queue.lock().await;
        let mult_bps = (credit_multiplier.clamp(1.0, 10.0) * 100.0) as u32;

        if let Some(pos) = q.iter().position(|entry| entry.client_address == client_address && entry.file_hash == file_hash) {
            return Ok((UploadSlotState::Waiting, Some((pos + 1) as u32)));
        }

        if q.len() >= MAX_UPLOAD_QUEUE {
            return Err(format!("Upload queue full ({MAX_UPLOAD_QUEUE} max)"));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        q.push_back(QueuedUpload {
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
            .position(|entry| entry.client_address == client_address && entry.file_hash == file_hash)
            .map(|pos| (pos + 1) as u32)
            .unwrap_or(q.len() as u32);

        Ok((UploadSlotState::Waiting, Some(rank)))
    }

    /// Updates active upload progress.
    pub async fn update_progress(&self, client_address: &str, bytes_sent: u64, rate_bps: u64) {
        let mut active = self.active_upload.lock().await;
        if let Some(ref mut cur) = *active {
            if cur.client_address == client_address {
                cur.transferred_bytes = cur.transferred_bytes.saturating_add(bytes_sent);
                cur.upload_rate_bps = rate_bps;
            }
        }
    }

    /// Releases active upload slot and promotes next waiting client.
    pub async fn release_slot(&self, client_address: &str) {
        let mut active = self.active_upload.lock().await;
        if let Some(ref cur) = *active {
            if cur.client_address == client_address {
                *active = None;
                self.active_count.store(0, Ordering::SeqCst);
            }
        }

        // Promote next from queue
        if active.is_none() {
            let mut q = self.queue.lock().await;
            if let Some(next) = q.pop_front() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;

                let snap = UploadSessionSnapshot {
                    client_address: next.client_address,
                    file_hash: next.file_hash,
                    file_name: next.file_name,
                    transferred_bytes: 0,
                    upload_rate_bps: 0,
                    state: UploadSlotState::Uploading,
                    queue_rank: None,
                    started_at_unix_ms: now,
                };
                *active = Some(snap);
                self.active_count.store(1, Ordering::SeqCst);
            }
        }
    }

    /// Clears queue and active slot on kill switch or network reset.
    pub async fn abort_all(&self) {
        let mut active = self.active_upload.lock().await;
        *active = None;
        self.active_count.store(0, Ordering::SeqCst);
        let mut q = self.queue.lock().await;
        q.clear();
    }

    /// Returns the snapshot of the current active upload (if any).
    pub async fn active_snapshot(&self) -> Option<UploadSessionSnapshot> {
        let active = self.active_upload.lock().await;
        active.clone()
    }

    /// Returns list of all queued upload snapshots.
    pub async fn list_queue(&self) -> Vec<UploadSessionSnapshot> {
        let q = self.queue.lock().await;
        q.iter()
            .enumerate()
            .map(|(idx, item)| UploadSessionSnapshot {
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
