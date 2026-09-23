//! eD2K Server Connection Manager.
//!
//! Manages at most 1 active TCP connection to an eD2K server.
//! Enforces network policy, token bucket rate limits, instant kill-switch abortion,
//! and bounded wire framing decoding.

use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::info;

use mule_network_policy::SocketPermit;
use mule_protocol::ed2k::{CallbackRequest, Ed2kLimits, GetSourcesRequest, SearchQuery};
use mule_protocol::framing::RawPacket;

use crate::error::DaemonError;
use crate::services::downloads::DownloadService;

pub mod connection;
pub mod events;
pub mod types;

use connection::{spawn_connection_task, ActiveSession};
use events::emit_event;
pub use types::{ServerConnectionStatus, ServerSessionSnapshot};

/// Thread-safe manager for the single eD2K server connection.
#[derive(Clone)]
pub struct ServerSessionManager {
    inner: Arc<Mutex<ActiveSession>>,
    event_tx: broadcast::Sender<String>,
    sequence: Arc<AtomicU64>,
    downloads: Arc<std::sync::RwLock<Option<Arc<Mutex<DownloadService>>>>>,
    /// Inbound listener — `OP_CALLBACKREQUESTED` (server-relayed callback to
    /// a LowID us) triggers an outbound connect-back served by this service.
    inbound:
        Arc<std::sync::RwLock<Option<crate::services::inbound_listener::InboundListenerService>>>,
    /// Kad service — a HighID `OP_IDCHANGE` teaches it our public IPv4
    /// (advertised in `PUBLISH_SOURCE` contacts).
    kad: Arc<std::sync::RwLock<Option<Arc<crate::services::kad::KadService>>>>,
    /// Prefer the server's dedicated obfuscation port (`ST_TCPPORTOBFUSCATION`)
    /// when advertised — mirrors eMule `IsServerCryptLayerConnection()`.
    prefer_obfuscation: Arc<std::sync::atomic::AtomicBool>,
    /// Max random padding for the obfuscated handshake (eMule PAD_DIGITS).
    obfuscation_padding_max: Arc<std::sync::atomic::AtomicU8>,
    /// Last successful connection target — used by the auto-reconnect
    /// watcher (eMule `Reconnect` pref). Cleared on user-initiated disconnect.
    last_target: Arc<std::sync::Mutex<Option<ServerTarget>>>,
}

/// Remembered server target for auto-reconnect (address + display name).
#[derive(Clone)]
pub struct ServerTarget {
    /// Server socket address (`ip:port`).
    pub addr: SocketAddr,
    /// Server display name, if known.
    pub name: Option<String>,
}

impl ServerSessionManager {
    /// Creates a new `ServerSessionManager`.
    pub fn new(event_tx: broadcast::Sender<String>, sequence: Arc<AtomicU64>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ActiveSession::new())),
            event_tx,
            sequence,
            downloads: Arc::new(std::sync::RwLock::new(None)),
            inbound: Arc::new(std::sync::RwLock::new(None)),
            kad: Arc::new(std::sync::RwLock::new(None)),
            prefer_obfuscation: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            obfuscation_padding_max: Arc::new(std::sync::atomic::AtomicU8::new(0)),
            last_target: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Returns the last connection target for the auto-reconnect watcher.
    pub fn last_target(&self) -> Option<ServerTarget> {
        self.last_target.lock().ok().and_then(|g| g.clone())
    }

    /// Clears the remembered target (auto-removed dead server, tests).
    pub fn clear_last_target(&self) {
        if let Ok(mut g) = self.last_target.lock() {
            *g = None;
        }
    }

    /// Enables/disables obfuscated server connections (`ST_TCPPORTOBFUSCATION`)
    /// and sets the max handshake padding (0 = no padding, eMule default ≤ 254).
    pub fn set_obfuscation_preference(&self, enabled: bool, padding_max: u8) {
        self.prefer_obfuscation
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
        self.obfuscation_padding_max
            .store(padding_max, std::sync::atomic::Ordering::Relaxed);
    }

    /// Injects the inbound listener service used for `OP_CALLBACKREQUESTED`
    /// connect-backs.
    pub fn set_inbound_listener(
        &self,
        svc: crate::services::inbound_listener::InboundListenerService,
    ) {
        *self.inbound.write().unwrap() = Some(svc);
    }

    /// Sets the download service reference for auto-ingesting found sources.
    pub fn set_downloads(&self, downloads: Arc<Mutex<DownloadService>>) {
        if let Ok(mut d) = self.downloads.write() {
            *d = Some(downloads);
        }
    }

    /// Injects the Kad service — the server-assigned HighID is forwarded to
    /// it as our observed public IPv4 for `PUBLISH_SOURCE` contacts.
    pub fn set_kad(&self, kad: Arc<crate::services::kad::KadService>) {
        if let Ok(mut k) = self.kad.write() {
            *k = Some(kad);
        }
    }

    /// Returns a snapshot of the current server session.
    pub async fn snapshot(&self) -> ServerSessionSnapshot {
        let guard = self.inner.lock().await;
        guard.snapshot.clone()
    }

    /// Returns true if a server connection is active or in progress.
    pub async fn is_active(&self) -> bool {
        let guard = self.inner.lock().await;
        matches!(
            guard.snapshot.status,
            ServerConnectionStatus::Connecting | ServerConnectionStatus::Connected
        )
    }

    /// Sends a search query to the connected server.
    pub async fn search(&self, query: SearchQuery) -> Result<(), DaemonError> {
        let guard = self.inner.lock().await;
        if guard.snapshot.status != ServerConnectionStatus::Connected {
            return Err(DaemonError::Business(
                "Cannot search: server is not connected".into(),
            ));
        }
        let tx = guard.outgoing_tx.as_ref().ok_or_else(|| {
            DaemonError::Business("Cannot search: outgoing channel closed".into())
        })?;
        let limits = Ed2kLimits::default();
        let packet = query
            .encode(&limits)
            .map_err(|e| DaemonError::Business(format!("Failed to encode search query: {e}")))?;

        info!(
            "Sending eD2K OP_SEARCHREQUEST to server (term='{}', min_size={:?}, max_size={:?}, ext={:?}, payload_len={})",
            query.term,
            query.min_size,
            query.max_size,
            query.extension,
            packet.payload.len()
        );

        tx.send(packet)
            .await
            .map_err(|_| DaemonError::Business("Failed to send search request to server".into()))?;
        Ok(())
    }

    /// Sends a get sources request to the connected server for a specific file hash.
    pub async fn get_sources(&self, request: GetSourcesRequest) -> Result<(), DaemonError> {
        let guard = self.inner.lock().await;
        if guard.snapshot.status != ServerConnectionStatus::Connected {
            return Err(DaemonError::Business(
                "Cannot query sources: server is not connected".into(),
            ));
        }
        let tx = guard.outgoing_tx.as_ref().ok_or_else(|| {
            DaemonError::Business("Cannot query sources: outgoing channel closed".into())
        })?;
        let limits = Ed2kLimits::for_server();
        let packet = request.encode(&limits).map_err(|e| {
            DaemonError::Business(format!("Failed to encode get sources request: {e}"))
        })?;
        info!(
            "Sending eD2K OP_GETSOURCES (obfuscation={}) to server for file hash: {}",
            request.obfuscation,
            hex::encode(request.file_hash)
        );
        tx.send(packet).await.map_err(|_| {
            DaemonError::Business("Failed to send get sources request to server".into())
        })?;
        Ok(())
    }

    /// Sends an `OP_CALLBACKREQUEST` to the connected server, asking it to
    /// relay a connection request to a LowID client (`client_id < 0x01000000`).
    /// The target then opens a TCP connection back to us.
    pub async fn request_callback(&self, client_id: u32) -> Result<(), DaemonError> {
        let guard = self.inner.lock().await;
        if guard.snapshot.status != ServerConnectionStatus::Connected {
            return Err(DaemonError::Business(
                "Cannot request callback: server is not connected".into(),
            ));
        }
        let tx = guard.outgoing_tx.as_ref().ok_or_else(|| {
            DaemonError::Business("Cannot request callback: outgoing channel closed".into())
        })?;
        let packet = CallbackRequest { client_id }.encode();
        info!("Sending eD2K OP_CALLBACKREQUEST to server for LowID client {client_id}");
        tx.send(packet).await.map_err(|_| {
            DaemonError::Business("Failed to send callback request to server".into())
        })?;
        Ok(())
    }

    /// Disconnects any active session and resets state.
    pub async fn disconnect(&self) -> Result<(), DaemonError> {
        self.disconnect_inner(false).await
    }

    /// Kill-switch variant (`network.disconnectAll`): identical teardown
    /// but keeps `last_target` so the reconnect watcher can restore the
    /// session once `network.enable` re-arms the policy.
    pub async fn disconnect_for_kill_switch(&self) -> Result<(), DaemonError> {
        self.disconnect_inner(true).await
    }

    async fn disconnect_inner(&self, keep_target: bool) -> Result<(), DaemonError> {
        let mut guard = self.inner.lock().await;
        if let Some(handle) = guard.abort_handle.take() {
            handle.abort();
        }
        guard.permit = None;
        guard.outgoing_tx = None;
        guard.snapshot = ServerSessionSnapshot::default();
        if !keep_target {
            // User-initiated disconnect: no auto-reconnect to this target.
            self.clear_last_target();
        }
        self.emit_notification(&guard.snapshot);
        Ok(())
    }

    /// Initiates a connection to `target_addr` under `permit`.
    /// Rejects if already connecting or connected.
    #[allow(clippy::too_many_arguments)]
    pub async fn connect(
        &self,
        target_addr: SocketAddr,
        server_name: Option<String>,
        permit: SocketPermit,
        connect_timeout_ms: u64,
        handshake_timeout_ms: u64,
        client_port: u16,
        nickname: String,
        user_hash: [u8; 16],
    ) -> Result<(), DaemonError> {
        let mut guard = self.inner.lock().await;
        if matches!(
            guard.snapshot.status,
            ServerConnectionStatus::Connecting | ServerConnectionStatus::Connected
        ) {
            return Err(DaemonError::Business(
                "An eD2K server connection is already in progress or connected".into(),
            ));
        }

        let address_str = target_addr.to_string();
        if let Ok(mut lt) = self.last_target.lock() {
            *lt = Some(ServerTarget {
                addr: target_addr,
                name: server_name.clone(),
            });
        }
        guard.snapshot = ServerSessionSnapshot {
            status: ServerConnectionStatus::Connecting,
            server_address: Some(address_str.clone()),
            server_name,
            server_desc: None,
            client_id: None,
            is_high_id: false,
            welcome_message: None,
            current_users: None,
            current_files: None,
            connected_at_unix_ms: None,
            ping_ms: None,
        };
        self.emit_notification(&guard.snapshot);

        let (outgoing_tx, outgoing_rx) = mpsc::channel::<RawPacket>(32);
        guard.outgoing_tx = Some(outgoing_tx);

        let inner_clone = Arc::clone(&self.inner);
        let event_tx_clone = self.event_tx.clone();
        let sequence_clone = Arc::clone(&self.sequence);
        let downloads_clone = self.downloads.read().ok().and_then(|g| g.clone());
        let inbound_clone = self.inbound.read().ok().and_then(|g| g.clone());
        let kad_clone = self.kad.read().ok().and_then(|g| g.clone());
        let cancel_token = permit.cancellation_token();
        let prefer_obf = self
            .prefer_obfuscation
            .load(std::sync::atomic::Ordering::Relaxed);
        let pad_max = self
            .obfuscation_padding_max
            .load(std::sync::atomic::Ordering::Relaxed);

        let task = spawn_connection_task(
            target_addr,
            cancel_token,
            connect_timeout_ms,
            handshake_timeout_ms,
            client_port,
            nickname,
            user_hash,
            outgoing_rx,
            inner_clone,
            event_tx_clone,
            sequence_clone,
            downloads_clone,
            inbound_clone,
            kad_clone,
            prefer_obf,
            pad_max,
        );

        guard.abort_handle = Some(task.abort_handle());
        guard.permit = Some(permit);

        Ok(())
    }

    fn emit_notification(&self, snapshot: &ServerSessionSnapshot) {
        emit_event(&self.event_tx, &self.sequence, snapshot);
    }
}
