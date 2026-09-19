use mule_core::model::PeerFailureReason;
use mule_protocol::ed2k::InFlightBlock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use mule_network_policy::SocketPermit;

/// Maximum active peer connections per file hash (hard safety ceiling).
pub const MAX_PEERS_PER_DOWNLOAD: usize = 400;

/// Maximum active peer connections across the entire daemon.
pub const MAX_GLOBAL_PEERS: usize = 500;

/// Maximum read buffer capacity for incoming peer data (256 KiB) to prevent OOM.
pub const MAX_PEER_READ_BUFFER_SIZE: usize = 256 * 1024;

/// Configurable granular timeouts for eD2K peer connections.
#[derive(Debug, Clone, Copy)]
pub struct PeerTimeouts {
    pub connect_timeout: Duration,
    pub handshake_timeout: Duration,
    pub status_timeout: Duration,
    pub queue_timeout: Duration,
    pub first_data_timeout: Duration,
    pub inactivity_timeout: Duration,
}

impl Default for PeerTimeouts {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            handshake_timeout: Duration::from_secs(8),
            status_timeout: Duration::from_secs(20),
            queue_timeout: Duration::from_secs(900),
            first_data_timeout: Duration::from_secs(25),
            inactivity_timeout: Duration::from_secs(25),
        }
    }
}

/// Computes the exponential backoff cooldown in milliseconds based on fail_count.
/// Formule: min(1_200_000, 2^(fail_count - 1) * 60_000) (1m, 2m, 4m, 8m... max 20 min)
pub fn calculate_backoff_cooldown_ms(fail_count: u32) -> i64 {
    if fail_count == 0 {
        return 0;
    }
    let exponent = (fail_count - 1).min(5);
    let base = 60_000i64;
    let factor = 1i64 << exponent;
    (base * factor).min(1_200_000)
}

/// Current lifecycle status of an eD2K peer connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PeerConnectionStatus {
    /// No peer connection active.
    Disconnected,
    /// TCP socket connect in progress.
    Connecting,
    /// Handshake (OP_HELLO / OP_HELLO_ANSWER) in progress.
    Handshaking,
    /// Queued on remote peer (OP_QUEUERANKING / waiting for upload slot).
    Queued,
    /// Active block downloading (OP_REQUESTPARTS / OP_SENDINGPART).
    Transferring,
    /// Teardown in progress.
    Disconnecting,
}

impl PeerConnectionStatus {
    /// String representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Handshaking => "handshaking",
            Self::Queued => "queued",
            Self::Transferring => "transferring",
            Self::Disconnecting => "disconnecting",
        }
    }
}

/// Snapshot of an individual eD2K peer session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PeerSessionSnapshot {
    /// Current connection status.
    pub status: PeerConnectionStatus,
    /// Target file hash in hex.
    pub file_hash: Option<String>,
    /// Remote peer address ("ip:port").
    pub peer_address: Option<String>,
    /// Currently requested part index, if transferring.
    pub current_part: Option<u16>,
    /// Transfer speed estimate in bytes/second.
    pub transfer_rate_bps: u64,
    /// Total bytes received in this session.
    pub transferred_bytes: u64,
    /// Queue rank reported by remote peer.
    pub queue_rank: Option<u32>,
    /// Last error message or failure reason.
    pub last_error: Option<String>,
    /// Normalized failure reason, if failed.
    pub failure_reason: Option<PeerFailureReason>,
    /// Timestamp when connection completed (unix ms).
    pub connected_at_unix_ms: Option<i64>,
    /// Whether this peer session is using encrypted protocol obfuscation.
    #[serde(default)]
    pub obfuscated: bool,
}

impl Default for PeerSessionSnapshot {
    fn default() -> Self {
        Self {
            status: PeerConnectionStatus::Disconnected,
            file_hash: None,
            peer_address: None,
            current_part: None,
            transfer_rate_bps: 0,
            transferred_bytes: 0,
            queue_rank: None,
            last_error: None,
            failure_reason: None,
            connected_at_unix_ms: None,
            obfuscated: false,
        }
    }
}

/// Aggregated swarm status for a specific file or globally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SwarmStatusSnapshot {
    /// Target file hash if filtered by file.
    pub file_hash: Option<String>,
    /// Total active peer connections.
    pub active_peer_count: usize,
    /// Active individual peer sessions.
    pub peers: Vec<PeerSessionSnapshot>,
    /// All active non-expired in-flight block ranges.
    pub in_flight_blocks: Vec<InFlightBlock>,
}

pub(crate) struct PeerSessionHandle {
    pub snapshot: PeerSessionSnapshot,
    pub abort_handle: tokio::task::AbortHandle,
    pub _permit: SocketPermit,
}

pub(crate) struct SwarmInnerState {
    /// Active sessions indexed by peer address ("ip:port").
    pub sessions: HashMap<String, PeerSessionHandle>,
    /// Active in-flight blocks allocated to peers.
    pub in_flight_blocks: Vec<InFlightBlock>,
    /// Most recent peer session snapshot (active or disconnected), preserved for inspections.
    pub last_session: Option<PeerSessionSnapshot>,
    /// Maximum active peer connections per file hash.
    pub max_peers_per_download: usize,
    /// Maximum active peer connections across the entire daemon.
    pub max_global_peers: usize,
}

#[derive(Debug)]
pub(crate) struct PeerError {
    pub reason: PeerFailureReason,
    pub message: String,
}

impl PeerError {
    pub fn new(reason: PeerFailureReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
}
