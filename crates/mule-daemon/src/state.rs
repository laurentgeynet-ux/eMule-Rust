use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use mule_network_policy::{NetworkPolicyManager, ResourceBudgets};

use crate::config::DaemonConfig;
use crate::services::downloads::types::DownloadSnapshot;
use crate::services::downloads::DownloadService;
use crate::services::peer_session::PeerSessionManager;
use crate::services::server_session::ServerSessionManager;
use crate::services::DaemonEvent;

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
    pub fn new(config: DaemonConfig, mut downloads: DownloadService) -> Self {
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
        downloads.set_event_channel(event_tx.clone(), Arc::clone(&sequence));
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
        inbound_listener
            .upload_manager()
            .set_event_channel(event_tx.clone(), Arc::clone(&sequence));
        let kad_udp = Arc::clone(kad.udp());
        let client_udp_for_kad = client_udp.clone();
        let kad_udp_dl = Arc::clone(&dl_arc);
        tokio::spawn(async move {
            kad_udp.set_client_udp(client_udp_for_kad).await;
            kad_udp.set_downloads(kad_udp_dl).await;
        });

        // Periodic aggregate stats tick (event.statsTick, topic `stats`).
        // Skipped entirely while no WebSocket client is subscribed.
        {
            let stats_downloads = Arc::clone(&dl_arc);
            let stats_uploads = inbound_listener.upload_manager().clone();
            let stats_peers = peer_session.clone();
            let stats_server = server_session.clone();
            let stats_kad = Arc::clone(&kad);
            let stats_tx = event_tx.clone();
            let stats_seq = Arc::clone(&sequence);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    if stats_tx.receiver_count() == 0 {
                        continue;
                    }
                    let downloads = stats_downloads.lock().await.list_downloads();
                    let download_bps: u64 = downloads.iter().map(|d| d.download_rate_bps).sum();
                    let downloads_active = downloads
                        .iter()
                        .filter(|d| d.status == "downloading")
                        .count();
                    let upload_bps: u64 = stats_uploads
                        .active_snapshots()
                        .await
                        .iter()
                        .map(|u| u.upload_rate_bps)
                        .sum();
                    let peers_connected = stats_peers.swarm_snapshot(None).await.active_peer_count;
                    let server_connected = matches!(
                        stats_server.snapshot().await.status,
                        crate::services::server_session::ServerConnectionStatus::Connected
                    );
                    let kad_status = stats_kad.get_status().await;
                    DaemonEvent::StatsTick {
                        sequence: stats_seq.fetch_add(1, Ordering::SeqCst),
                        stats: crate::services::events::StatsTickSnapshot {
                            download_bps,
                            upload_bps,
                            downloads_active,
                            peers_connected,
                            server_connected,
                            kad_contacts: kad_status.total_contacts,
                            kad_udp_bound: kad_status.udp_bind_address.is_some(),
                        },
                    }
                    .send(&stats_tx);
                }
            });
        }
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
        DaemonEvent::DownloadUpdated {
            sequence: seq,
            download: snapshot,
        }
        .send(&self.event_tx);
    }
}
