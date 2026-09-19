use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

use super::types::PeerSessionSnapshot;
use crate::services::downloads::types::DownloadSnapshot;

pub fn emit_event(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    snapshot: &PeerSessionSnapshot,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let notif = json!({
        "jsonrpc": "2.0",
        "method": "event.peerSessionUpdated",
        "params": {
            "sequence": seq,
            "session": snapshot,
        }
    });
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
}

pub fn emit_download_update(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    snapshot: &DownloadSnapshot,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let notif = json!({
        "jsonrpc": "2.0",
        "method": "event.downloadUpdated",
        "params": {
            "sequence": seq,
            "download": snapshot,
        }
    });
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
}
