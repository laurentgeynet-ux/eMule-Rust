use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::rpc::types::JsonRpcNotification;
use crate::services::downloads::types::DownloadSnapshot;
use crate::services::peer_session::types::PeerSessionSnapshot;
use crate::services::server_session::types::ServerSessionSnapshot;

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
            Self::PeerSessionUpdated { .. } => "event.peerSessionUpdated",
            Self::ServerSessionUpdated { .. } => "server.sessionUpdated",
            Self::Custom { method, .. } => method.as_str(),
        }
    }

    /// Subscription topic for `events.subscribe` filtering; `None` marks
    /// control events that bypass the topic filter.
    pub fn topic(&self) -> Option<&'static str> {
        match self {
            Self::DownloadUpdated { .. } => Some("downloads"),
            Self::PeerSessionUpdated { .. } => Some("peers"),
            Self::ServerSessionUpdated { .. } => Some("servers"),
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
            Self::DownloadUpdated { sequence, download } => serde_json::json!({
                "sequence": sequence,
                "download": download,
            }),
            Self::PeerSessionUpdated { sequence, session } => serde_json::json!({
                "sequence": sequence,
                "session": session,
            }),
            Self::ServerSessionUpdated { sequence, session } => serde_json::json!({
                "sequence": sequence,
                "session": session,
            }),
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
