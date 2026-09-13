use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::config::DaemonConfig;
use crate::rpc::events::{DownloadSnapshot, DownloadUpdatedEvent};
use crate::rpc::types::JsonRpcNotification;
use crate::services::downloads::DownloadService;

#[derive(Clone)]
pub struct AppState {
    pub config: DaemonConfig,
    pub downloads: Arc<Mutex<DownloadService>>,
    pub sequence: Arc<AtomicU64>,
    pub event_tx: broadcast::Sender<String>,
    pub instance_id: Uuid,
    pub session_id: Uuid,
    pub abort_handles: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
}

impl AppState {
    pub fn new(config: DaemonConfig, downloads: DownloadService) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            config,
            downloads: Arc::new(Mutex::new(downloads)),
            sequence: Arc::new(AtomicU64::new(1)),
            event_tx,
            instance_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            abort_handles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Registers a background task handle so it can be cleanly aborted on shutdown.
    pub async fn register_task(&self, handle: tokio::task::AbortHandle) {
        let mut handles = self.abort_handles.lock().await;
        handles.push(handle);
    }

    /// Aborts all active background tasks associated with this daemon state.
    pub async fn abort_all_tasks(&self) {
        let mut handles = self.abort_handles.lock().await;
        for handle in handles.drain(..) {
            handle.abort();
        }
    }

    /// Emits a sequenced download update event to all active WebSocket subscribers.
    pub fn broadcast_download_update(&self, snapshot: DownloadSnapshot) {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        let event = DownloadUpdatedEvent {
            sequence: seq,
            download: snapshot,
        };

        let notification = JsonRpcNotification::new(
            "event.downloadUpdated",
            serde_json::to_value(event).unwrap_or_default(),
        );

        if let Ok(payload) = serde_json::to_string(&notification) {
            let _ = self.event_tx.send(payload);
        }
    }
}
