use serde::{Deserialize, Serialize};

use crate::services::downloads::types::DownloadSnapshot;
use crate::services::peer_session::types::PeerSessionSnapshot;
use crate::services::server_session::types::ServerSessionSnapshot;

/// Strongly-typed domain and lifecycle events emitted across the daemon.
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
    /// Raw JSON-RPC notification for backward compatibility.
    Custom {
        method: String,
        payload: serde_json::Value,
    },
}
