// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

use super::types::PeerSessionSnapshot;
use crate::services::downloads::types::DownloadSnapshot;
use crate::services::DaemonEvent;

pub fn emit_event(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    snapshot: &PeerSessionSnapshot,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    DaemonEvent::PeerSessionUpdated {
        sequence: seq,
        session: snapshot.clone(),
    }
    .send(event_tx);
}

pub fn emit_download_update(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    snapshot: &DownloadSnapshot,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    DaemonEvent::DownloadUpdated {
        sequence: seq,
        download: snapshot.clone(),
    }
    .send(event_tx);
}
