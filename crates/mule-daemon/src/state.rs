use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use mule_network_policy::{NetworkPolicyManager, ResourceBudgets};

use crate::config::DaemonConfig;
use crate::rpc::events::DownloadUpdatedEvent;
use crate::rpc::types::JsonRpcNotification;
use crate::services::downloads::types::DownloadSnapshot;
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
    /// Shared SUI anomaly counters (identity.status `sui` object).
    pub sui_stats: Arc<crate::services::identity::SuiStats>,
    pub kad: Arc<crate::services::KadService>,
    /// Client-to-client UDP service sharing the Kad UDP socket
    /// (`OP_REASKFILEPING`/`OP_REASKACK`/`OP_QUEUEFULL`/`OP_FILENOTFOUND`).
    pub client_udp: crate::services::ClientUdpService,
    /// Server UDP service (`OP_GLOBGETSOURCES2`/`OP_GLOBFOUNDSOURCES`),
    /// separate socket like eMule's `UDPSocket`.
    pub server_udp: crate::services::server_udp::ServerUdpService,
    /// USS (Upload Speed Sense) — dynamic upload limiter driven by
    /// measured latency to the connected server or a configured host.
    pub uss: crate::services::UssService,
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
        let (max_per_dl, max_global) = {
            let max_g = downloads
                .get_setting("max_global_download_peers")
                .ok()
                .flatten()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(config.max_global_download_peers);
            let max_f = downloads
                .get_setting("max_peers_per_download")
                .ok()
                .flatten()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(config.max_peers_per_download);
            (max_f, max_g)
        };

        let mut budgets = ResourceBudgets::default();
        if budgets.max_active_peer_connections < max_global {
            budgets.max_active_peer_connections = max_global.min(1500);
        }

        let network_policy = NetworkPolicyManager::new(config.network_profile, budgets);
        let sequence = Arc::new(AtomicU64::new(1));
        let server_session = ServerSessionManager::new(event_tx.clone(), Arc::clone(&sequence));
        let peer_session = PeerSessionManager::new(event_tx.clone(), Arc::clone(&sequence))
            .with_limits(max_per_dl, max_global);
        let dl_arc = Arc::new(Mutex::new(downloads));
        server_session.set_downloads(Arc::clone(&dl_arc));
        let identity_service = crate::services::IdentityService::load_or_generate(&config.data_dir);
        let user_hash = identity_service.user_hash();
        let sui_pubkey_der = if identity_service.has_key() {
            Some(identity_service.public_key_bytes().to_vec())
        } else {
            None
        };
        let identity = Arc::new(Mutex::new(identity_service));
        let sui_stats = Arc::new(crate::services::identity::SuiStats::default());
        let mut inbound_listener = crate::services::inbound_listener::InboundListenerService::new(
            Arc::new(network_policy.clone()),
            Arc::clone(&dl_arc),
            user_hash,
            sui_pubkey_der,
            config.obfuscation_enabled,
            config.obfuscation_required,
            config.obfuscation_padding_max,
            Arc::clone(&sui_stats),
        );
        inbound_listener.set_identity(Arc::clone(&identity));
        inbound_listener.set_peer_session(peer_session.clone());
        inbound_listener.set_server_session(server_session.clone());
        let kad_id = mule_core::kad::KadId::random();
        let kad = Arc::new(crate::services::KadService::new(
            kad_id,
            Arc::clone(&dl_arc),
            network_policy.clone(),
            network_policy.ip_filter(),
        ));
        kad.set_ed2k_tcp_port(config.ed2k_port);
        kad.set_fw_check_identity(user_hash);
        // Buddy relay role: TCP port the buddy client connects to, and the
        // GetMyConnectOptions-style flags byte sent in KADEMLIA_FINDBUDDY_RES
        // (bit0/1 = obfuscation supported+requested, bit2 = required).
        let connect_opts: u8 = if config.obfuscation_enabled { 0x03 } else { 0 }
            | if config.obfuscation_required { 0x04 } else { 0 };
        kad.udp().set_buddy_tcp_info(config.ed2k_port, connect_opts);
        inbound_listener.set_kad_udp(Arc::clone(kad.udp()));
        inbound_listener.set_kad(Arc::clone(&kad));
        let client_udp = crate::services::ClientUdpService::new();
        client_udp.set_downloads(Arc::clone(&dl_arc));
        client_udp.set_upload_manager(inbound_listener.upload_manager().clone());
        client_udp.set_peer_session(peer_session.clone());
        client_udp.set_buddy_registry(kad.udp().buddy_registry());
        inbound_listener.set_client_udp(client_udp.clone());
        let server_udp = crate::services::server_udp::ServerUdpService::new(network_policy.clone());
        server_udp.set_downloads(Arc::clone(&dl_arc));
        server_udp.set_event_channel(event_tx.clone(), sequence.clone());
        let uss = crate::services::UssService::new();
        uss.set_server_session(server_session.clone());
        uss.set_upload_manager(inbound_listener.upload_manager().clone());
        uss.set_downloads(Arc::clone(&dl_arc));
        inbound_listener
            .upload_manager()
            .set_uss_gate(uss.accept_flag());
        let kad_udp = Arc::clone(kad.udp());
        let client_udp_for_kad = client_udp.clone();
        let kad_udp_dl = Arc::clone(&dl_arc);
        tokio::spawn(async move {
            kad_udp.set_client_udp(client_udp_for_kad).await;
            kad_udp.set_downloads(kad_udp_dl).await;
        });
        peer_session.set_kad(Arc::clone(&kad));
        peer_session.set_rate_limiters(network_policy.rate_limiters());
        peer_session.set_identity(Arc::clone(&identity));
        peer_session.set_sui_stats(Arc::clone(&sui_stats));
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
            sui_stats,
            kad,
            client_udp,
            server_udp,
            uss,
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
