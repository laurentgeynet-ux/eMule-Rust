// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::rpc::types::JsonRpcNotification;
use crate::services::downloads::types::DownloadSnapshot;
use crate::services::peer_session::types::PeerSessionSnapshot;
use crate::services::server_session::types::ServerSessionSnapshot;
use crate::services::upnp::UpnpStatusSnapshot;
use crate::services::upload_queue::UploadSessionSnapshot;

/// Aggregate daemon metrics emitted on a fixed cadence (~2 s) so clients can
/// render live top-bar badges and rate graphs without polling or sampling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatsTickSnapshot {
    /// Sum of `download_rate_bps` across all downloads.
    pub download_bps: u64,
    /// Sum of `upload_rate_bps` across active upload slots.
    pub upload_bps: u64,
    /// Downloads currently in `downloading` status.
    pub downloads_active: usize,
    /// Total active peer connections (all files).
    pub peers_connected: usize,
    /// Whether the eD2K server session is connected.
    pub server_connected: bool,
    /// Total Kad routing-table contacts.
    pub kad_contacts: usize,
    /// Whether the Kad UDP socket is bound.
    pub kad_udp_bound: bool,
    /// Bytes received since daemon start (sum of per-download session deltas).
    pub session_downloaded_bytes: u64,
    /// Bytes sent since daemon start (upload manager counter).
    pub session_uploaded_bytes: u64,
    /// Bytes received since install — persisted, resettable via
    /// `stats.resetLifetimeTotals`.
    pub lifetime_downloaded_bytes: u64,
    /// Bytes sent since install — persisted, resettable via
    /// `stats.resetLifetimeTotals`.
    pub lifetime_uploaded_bytes: u64,
}

/// Strongly-typed domain and lifecycle events emitted across the daemon.
///
/// This is the single definition of the notification wire format: every
/// event knows its JSON-RPC `method` name and serializes its payload into
/// `params`. Emit sites must go through [`DaemonEvent::send`] so the
/// contract documented in `docs/architecture/rpc_protocol.md` §6 cannot
/// drift.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum DaemonEvent {
    /// A download's progress, speed, or status was updated.
    DownloadUpdated {
        sequence: u64,
        download: DownloadSnapshot,
    },
    /// A download was added to the queue (any source: RPC, deep link, lab).
    DownloadAdded {
        sequence: u64,
        download: DownloadSnapshot,
    },
    /// A download was removed from the queue. Carries only the id.
    DownloadRemoved { sequence: u64, id: String },
    /// A download transitioned to `completed` (verified + moved).
    DownloadCompleted {
        sequence: u64,
        download: DownloadSnapshot,
    },
    /// A download transitioned to `failed`.
    DownloadFailed {
        sequence: u64,
        download: DownloadSnapshot,
        reason: Option<String>,
    },
    /// A peer session's connection or transfer status changed.
    PeerSessionUpdated {
        sequence: u64,
        session: PeerSessionSnapshot,
    },
    /// The eD2K server session connection status changed.
    ServerSessionUpdated {
        sequence: u64,
        session: ServerSessionSnapshot,
    },
    /// Upload slots/queue changed or upload progress ticked (throttled
    /// ~500 ms). Carries the full active+queue snapshot plus the
    /// aggregate upload rate — clients render without diffing.
    UploadsUpdated {
        sequence: u64,
        active: Vec<UploadSessionSnapshot>,
        queue: Vec<UploadSessionSnapshot>,
        upload_rate_bps: u64,
    },
    /// Periodic aggregate stats tick (topic `stats`, cadence ~2 s).
    StatsTick {
        sequence: u64,
        stats: StatsTickSnapshot,
    },
    /// Kad routing/UDP state changed (contacts, bootstrap, firewall
    /// status). Nudge event: clients refetch `kad.status`.
    KadStatusChanged { sequence: u64 },
    /// A history entry was recorded. Nudge event: clients refetch
    /// `history.list` / `history.stats`.
    HistoryEntryAdded { sequence: u64 },
    /// Daemon settings changed. `keys` lists the setting keys touched.
    SettingsChanged { sequence: u64, keys: Vec<String> },
    /// Network policy changed (profile, kill switch, budgets). Nudge
    /// event: clients refetch `network.getPolicy`.
    NetworkPolicyChanged { sequence: u64 },
    /// UPnP gateway/mapping status changed (enable, map, unmap).
    /// `params` is the full `UpnpStatus` snapshot — no `sequence`.
    UpnpStatusChanged { snapshot: UpnpStatusSnapshot },
    /// A notification outside the typed set (search results, found
    /// sources, control events like `event.snapshotRequired`).
    Custom {
        method: String,
        payload: serde_json::Value,
    },
}

impl DaemonEvent {
    /// JSON-RPC notification method name on the wire.
    pub fn method(&self) -> &str {
        match self {
            Self::DownloadUpdated { .. } => "event.downloadUpdated",
            Self::DownloadAdded { .. } => "event.downloadAdded",
            Self::DownloadRemoved { .. } => "event.downloadRemoved",
            Self::DownloadCompleted { .. } => "event.downloadCompleted",
            Self::DownloadFailed { .. } => "event.downloadFailed",
            Self::PeerSessionUpdated { .. } => "event.peerSessionUpdated",
            Self::ServerSessionUpdated { .. } => "server.sessionUpdated",
            Self::UploadsUpdated { .. } => "upload.uploadsUpdated",
            Self::StatsTick { .. } => "event.statsTick",
            Self::KadStatusChanged { .. } => "kad.statusChanged",
            Self::HistoryEntryAdded { .. } => "history.entryAdded",
            Self::SettingsChanged { .. } => "settings.changed",
            Self::NetworkPolicyChanged { .. } => "network.policyChanged",
            Self::UpnpStatusChanged { .. } => "upnp.statusChanged",
            Self::Custom { method, .. } => method.as_str(),
        }
    }

    /// Subscription topic for `events.subscribe` filtering; `None` marks
    /// control events that bypass the topic filter.
    pub fn topic(&self) -> Option<&'static str> {
        match self {
            Self::DownloadUpdated { .. }
            | Self::DownloadAdded { .. }
            | Self::DownloadRemoved { .. }
            | Self::DownloadCompleted { .. }
            | Self::DownloadFailed { .. } => Some("downloads"),
            Self::PeerSessionUpdated { .. } => Some("peers"),
            Self::ServerSessionUpdated { .. } => Some("servers"),
            Self::UploadsUpdated { .. } => Some("uploads"),
            Self::StatsTick { .. } => Some("stats"),
            Self::KadStatusChanged { .. } => Some("kad"),
            Self::HistoryEntryAdded { .. } => Some("history"),
            Self::SettingsChanged { .. } => Some("settings"),
            Self::NetworkPolicyChanged { .. } | Self::UpnpStatusChanged { .. } => {
                Some("network")
            }
            Self::Custom { method, .. } => match method.as_str() {
                "server.searchResult" | "server.foundSources" => Some("search"),
                "upnp.statusChanged" => Some("network"),
                "event.snapshotRequired" => None,
                _ => Some("other"),
            },
        }
    }

    /// Serializes to the JSON-RPC notification frame sent over the socket.
    pub fn to_wire_json(&self) -> Option<String> {
        let params = match self {
            Self::DownloadUpdated { sequence, download }
            | Self::DownloadAdded { sequence, download }
            | Self::DownloadCompleted { sequence, download } => serde_json::json!({
                "sequence": sequence,
                "download": download,
            }),
            Self::DownloadRemoved { sequence, id } => serde_json::json!({
                "sequence": sequence,
                "id": id,
            }),
            Self::DownloadFailed {
                sequence,
                download,
                reason,
            } => serde_json::json!({
                "sequence": sequence,
                "download": download,
                "reason": reason,
            }),
            Self::PeerSessionUpdated { sequence, session } => serde_json::json!({
                "sequence": sequence,
                "session": session,
            }),
            Self::ServerSessionUpdated { sequence, session } => serde_json::json!({
                "sequence": sequence,
                "session": session,
            }),
            Self::UploadsUpdated {
                sequence,
                active,
                queue,
                upload_rate_bps,
            } => serde_json::json!({
                "sequence": sequence,
                "active": active,
                "queue": queue,
                "uploadRateBps": upload_rate_bps,
            }),
            Self::StatsTick { sequence, stats } => serde_json::json!({
                "sequence": sequence,
                "stats": stats,
            }),
            Self::KadStatusChanged { sequence }
            | Self::HistoryEntryAdded { sequence }
            | Self::NetworkPolicyChanged { sequence } => serde_json::json!({
                "sequence": sequence,
            }),
            Self::SettingsChanged { sequence, keys } => serde_json::json!({
                "sequence": sequence,
                "keys": keys,
            }),
            Self::UpnpStatusChanged { snapshot } => {
                serde_json::to_value(snapshot).unwrap_or_default()
            }
            Self::Custom { payload, .. } => payload.clone(),
        };
        serde_json::to_string(&JsonRpcNotification::new(self.method(), params)).ok()
    }

    /// Broadcasts the event to all WebSocket subscribers. Returns the
    /// receiver count (0 when nobody listens — not an error).
    pub fn send(self, event_tx: &broadcast::Sender<String>) -> usize {
        match self.to_wire_json() {
            Some(json) => event_tx.send(json).unwrap_or(0),
            None => 0,
        }
    }
}
