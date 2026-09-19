//! Inbound listener types and constants.

use crate::services::identity::SuiStats;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::services::downloads::DownloadService;
use crate::services::upload_queue::UploadManager;
use mule_network_policy::NetworkPolicyManager;

/// Maximum concurrent inbound connections globally.
pub const MAX_INBOUND_CONNECTIONS: usize = 500;

/// Maximum concurrent inbound connections from a single IP.
pub const MAX_INBOUND_PER_IP: usize = 5;

/// Maximum buffer size for inbound connection framing (256 KiB) to prevent OOM.
pub const MAX_READ_BUFFER_SIZE: usize = 256 * 1024;

/// Snapshot of the Inbound Listener service state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InboundListenerSnapshot {
    /// Whether the TCP listener is currently running.
    pub is_listening: bool,
    /// Listening socket address if active.
    pub listen_address: Option<String>,
    /// Number of active uploads currently transferring.
    pub active_uploads: usize,
    /// Number of queued peers waiting for an upload slot.
    pub queued_uploads: usize,
    /// Current upload rate in bytes per second.
    pub upload_rate_bps: u64,
}

/// Inbound eD2K TCP Listener service state.
#[derive(Clone)]
pub struct InboundListenerService {
    pub(crate) network_policy: Arc<NetworkPolicyManager>,
    pub(crate) downloads: Arc<Mutex<DownloadService>>,
    pub(crate) upload_manager: UploadManager,
    pub(crate) is_running: Arc<AtomicBool>,
    pub(crate) listen_addr: Arc<Mutex<Option<SocketAddr>>>,
    pub(crate) listener_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub(crate) user_hash: [u8; 16],
    /// Local RSA-384 public key DER (PKCS#1) used as challenger key in SUI
    /// payloads. `None` when the daemon has no usable identity (e.g. quarantined
    /// cryptkey.dat): challenges must not be issued without it.
    pub(crate) sui_pubkey_der: Option<Vec<u8>>,
    /// Local identity (private key access) for answering inbound peer
    /// challenges — the peer signs our key, we sign its key.
    pub(crate) identity: Option<Arc<Mutex<crate::services::IdentityService>>>,
    /// Anti-flood: IP of the last processed signature (eMule `sigIP`).
    pub(crate) last_sig_ip: Arc<Mutex<Option<IpAddr>>>,
    /// Shared SUI anomaly counters (identity.status `sui` object).
    pub(crate) sui_stats: Arc<SuiStats>,
    pub(crate) active_conns: Arc<Mutex<HashMap<IpAddr, usize>>>,
    pub(crate) obfuscation_enabled: bool,
    pub(crate) obfuscation_required: bool,
    pub(crate) obfuscation_padding_max: u8,
    /// Peer session manager for adopting inbound connections that answer a
    /// pending LowID callback (`OP_CALLBACKREQUEST` relayed by the server).
    pub(crate) peer_session:
        Arc<std::sync::RwLock<Option<crate::services::peer_session::PeerSessionManager>>>,
    /// Server session manager — provides our HighID `client_id`/server info
    /// when adopting callback connections.
    pub(crate) server_session:
        Arc<std::sync::RwLock<Option<crate::services::server_session::ServerSessionManager>>>,

    /// Client UDP service — used to record inbound clients' announced UDP
    /// versions for correctly-shaped `OP_REASKACK` replies.
    pub(crate) client_udp:
        Arc<std::sync::RwLock<Option<crate::services::client_udp::ClientUdpService>>>,

    /// Kad UDP service — answers `OP_FWCHECKUDPREQ` by emitting
    /// `KADEMLIA2_FIREWALLUDP` probe datagrams on the shared UDP socket.
    pub(crate) kad_udp:
        Arc<std::sync::RwLock<Option<Arc<crate::services::kad_udp::KadUdpService>>>>,
}
