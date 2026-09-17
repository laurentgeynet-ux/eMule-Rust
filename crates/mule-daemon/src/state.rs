use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use mule_network_policy::{NetworkPolicyManager, ResourceBudgets};

use crate::config::DaemonConfig;
use crate::rpc::events::{DownloadSnapshot, DownloadUpdatedEvent};
use crate::rpc::types::JsonRpcNotification;
use crate::services::downloads::DownloadService;
use crate::services::peer_session::PeerSessionManager;
use crate::services::server_session::ServerSessionManager;

#[derive(Clone)]
pub struct AppState {
    pub config: DaemonConfig,
    pub downloads: Arc<Mutex<DownloadService>>,
    pub network_policy: NetworkPolicyManager,
    pub server_session: ServerSessionManager,
    pub peer_session: PeerSessionManager,
    pub inbound_listener: crate::services::inbound_listener::InboundListenerService,
    pub identity: Arc<Mutex<crate::services::IdentityService>>,
    pub kad: Arc<crate::services::KadService>,
    pub upnp: Arc<crate::services::UpnpService>,
    pub sequence: Arc<AtomicU64>,
    pub event_tx: broadcast::Sender<String>,
    pub instance_id: Uuid,
    pub session_id: Uuid,
    pub abort_handles: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
    pub shutdown_notify: Arc<tokio::sync::Notify>,
    #[cfg(feature = "lab-tools")]
    pub active_lab_peer: Arc<Mutex<Option<(std::net::SocketAddr, tokio::task::AbortHandle)>>>,
}

impl AppState {
    pub fn new(config: DaemonConfig, downloads: DownloadService) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        let network_policy = NetworkPolicyManager::new(
            config.network_profile,
            ResourceBudgets::default(),
        );
        let sequence = Arc::new(AtomicU64::new(1));
        let server_session = ServerSessionManager::new(event_tx.clone(), Arc::clone(&sequence));
        let peer_session = PeerSessionManager::new(event_tx.clone(), Arc::clone(&sequence));
        let dl_arc = Arc::new(Mutex::new(downloads));
        server_session.set_downloads(Arc::clone(&dl_arc));
        let identity_service = crate::services::IdentityService::load_or_generate(&config.data_dir);
        let user_hash = identity_service.user_hash();
        let identity = Arc::new(Mutex::new(identity_service));
        let inbound_listener = crate::services::inbound_listener::InboundListenerService::new(
            Arc::new(network_policy.clone()),
            Arc::clone(&dl_arc),
            user_hash,
            config.obfuscation_enabled,
            config.obfuscation_required,
            config.obfuscation_padding_max,
        );
        let kad_id = mule_core::kad::KadId::random();
        let kad = Arc::new(crate::services::KadService::new(
            kad_id,
            Arc::clone(&dl_arc),
            network_policy.clone(),
            network_policy.ip_filter(),
        ));
        peer_session.set_kad(Arc::clone(&kad));
        let upnp = Arc::new(crate::services::UpnpService::new(
            config.upnp_enabled,
            config.ed2k_port,
            config.kad_port,
        ));
        Self {
            config,
            downloads: dl_arc,
            network_policy,
            server_session,
            peer_session,
            inbound_listener,
            identity,
            kad,
            upnp,
            sequence,
            event_tx,
            instance_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            abort_handles: Arc::new(Mutex::new(Vec::new())),
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
            #[cfg(feature = "lab-tools")]
            active_lab_peer: Arc::new(Mutex::new(None)),
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
        #[cfg(feature = "lab-tools")]
        {
            let mut peer = self.active_lab_peer.lock().await;
            if let Some((_, h)) = peer.take() {
                h.abort();
            }
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
