//! Inbound listener types and constants.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use mule_network_policy::NetworkPolicyManager;
use crate::services::downloads::DownloadService;
use crate::services::upload_queue::UploadManager;

/// Maximum concurrent inbound connections globally.
pub const MAX_INBOUND_CONNECTIONS: usize = 200;

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
    pub(crate) active_conns: Arc<Mutex<HashMap<IpAddr, usize>>>,
    pub(crate) obfuscation_enabled: bool,
    pub(crate) obfuscation_required: bool,
    pub(crate) obfuscation_padding_max: u8,
}
