//! Inbound eD2K TCP Listener service coordinator.

pub mod connection;
pub mod dispatcher;
pub mod types;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{info, warn};

use mule_network_policy::{ConnectionPurpose, NetworkPolicyManager};

pub use types::{
    InboundListenerService, InboundListenerSnapshot, MAX_INBOUND_CONNECTIONS, MAX_INBOUND_PER_IP,
    MAX_READ_BUFFER_SIZE,
};

use crate::services::downloads::DownloadService;
use crate::services::upload_queue::UploadManager;

impl InboundListenerService {
    /// Creates a new InboundListenerService instance.
    pub fn new(
        network_policy: Arc<NetworkPolicyManager>,
        downloads: Arc<Mutex<DownloadService>>,
        user_hash: [u8; 16],
        sui_pubkey_der: Option<Vec<u8>>,
        obfuscation_enabled: bool,
        obfuscation_required: bool,
        obfuscation_padding_max: u8,
        sui_stats: Arc<crate::services::identity::SuiStats>,
    ) -> Self {
        Self {
            network_policy,
            downloads,
            upload_manager: UploadManager::new(),
            is_running: Arc::new(AtomicBool::new(false)),
            listen_addr: Arc::new(Mutex::new(None)),
            listener_handle: Arc::new(Mutex::new(None)),
            user_hash,
            sui_pubkey_der,
            identity: None,
            last_sig_ip: Arc::new(Mutex::new(None)),
            sui_stats,
            active_conns: Arc::new(Mutex::new(HashMap::new())),
            obfuscation_enabled,
            obfuscation_required,
            obfuscation_padding_max,
            peer_session: Arc::new(std::sync::RwLock::new(None)),
            server_session: Arc::new(std::sync::RwLock::new(None)),
            client_udp: Arc::new(std::sync::RwLock::new(None)),
            kad_udp: Arc::new(std::sync::RwLock::new(None)),
            kad: Arc::new(std::sync::RwLock::new(None)),
            kad_udp_port: 0,
        }
    }

    /// Sets the Kad UDP port announced to inbound peers in handshake answers.
    pub fn set_kad_udp_port(&mut self, port: u16) {
        self.kad_udp_port = port;
    }

    /// Injects the client UDP service (inbound UDP version bookkeeping).
    pub fn set_client_udp(&mut self, svc: crate::services::client_udp::ClientUdpService) {
        *self.client_udp.write().unwrap() = Some(svc);
    }

    /// Injects the Kad UDP service (server side of the UDP firewall check).
    pub fn set_kad_udp(&mut self, svc: Arc<crate::services::kad_udp::KadUdpService>) {
        *self.kad_udp.write().unwrap() = Some(svc);
    }

    /// Injects the Kad service (bootstrap-from-client on inbound hellos).
    pub fn set_kad(&mut self, svc: Arc<crate::services::kad::KadService>) {
        *self.kad.write().unwrap() = Some(svc);
    }

    /// Injects the peer session manager (LowID callback adoption).
    pub fn set_peer_session(
        &mut self,
        peer_session: crate::services::peer_session::PeerSessionManager,
    ) {
        *self.peer_session.write().unwrap() = Some(peer_session);
    }

    /// Injects the server session manager (our HighID client ID for callback sessions).
    pub fn set_server_session(
        &mut self,
        server_session: crate::services::server_session::ServerSessionManager,
    ) {
        *self.server_session.write().unwrap() = Some(server_session);
    }

    /// Injects the local identity service so inbound challenges can be signed.
    pub fn set_identity(&mut self, identity: Arc<Mutex<crate::services::IdentityService>>) {
        self.identity = Some(identity);
    }

    /// Accesses the underlying upload manager.
    pub fn upload_manager(&self) -> &UploadManager {
        &self.upload_manager
    }

    /// Starts the inbound listener on the requested bind address.
    pub async fn start(&self, bind_addr: SocketAddr) -> Result<SocketAddr, String> {
        if self
            .is_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("Inbound listener is already running".to_string());
        }

        let listen_permit = match self
            .network_policy
            .request_tcp_listen(bind_addr, ConnectionPurpose::Listener)
        {
            Ok(p) => p,
            Err(e) => {
                self.is_running.store(false, Ordering::SeqCst);
                return Err(format!("Policy denied listener on {bind_addr}: {e}"));
            }
        };

        let tokio_listener = match TcpListener::bind(bind_addr).await {
            Ok(l) => l,
            Err(e) => {
                self.is_running.store(false, Ordering::SeqCst);
                return Err(format!("Failed to bind TCP listener to {bind_addr}: {e}"));
            }
        };

        let actual_addr = match tokio_listener.local_addr() {
            Ok(a) => a,
            Err(e) => {
                self.is_running.store(false, Ordering::SeqCst);
                return Err(format!("Failed to retrieve local address: {e}"));
            }
        };

        *self.listen_addr.lock().await = Some(actual_addr);

        let cancel_token = listen_permit.cancellation_token().clone();
        let service_self = self.clone();

        let handle = tokio::spawn(async move {
            info!("Inbound eD2K listener started on {}", actual_addr);
            let _permit = listen_permit;

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("Inbound listener cancelled by network policy / kill switch");
                        break;
                    }
                    accept_res = tokio_listener.accept() => {
                        match accept_res {
                            Ok((socket, remote_addr)) => {
                                let ip = remote_addr.ip();
                                let mut conns = service_self.active_conns.lock().await;
                                let total_conns: usize = conns.values().sum();
                                if total_conns >= MAX_INBOUND_CONNECTIONS {
                                    warn!("Rejected inbound connection from {}: global limit reached ({MAX_INBOUND_CONNECTIONS})", remote_addr);
                                    drop(socket);
                                    continue;
                                }
                                let ip_count = conns.entry(ip).or_insert(0);
                                if *ip_count >= MAX_INBOUND_PER_IP {
                                    warn!("Rejected inbound connection from {}: per-IP limit reached ({MAX_INBOUND_PER_IP})", remote_addr);
                                    drop(socket);
                                    continue;
                                }
                                *ip_count += 1;
                                drop(conns);

                                let service_clone = service_self.clone();
                                let cancel_clone = cancel_token.clone();
                                tokio::spawn(async move {
                                    service_clone.handle_inbound_connection(socket, remote_addr, actual_addr, cancel_clone).await;
                                    let mut conns = service_clone.active_conns.lock().await;
                                    if let std::collections::hash_map::Entry::Occupied(mut e) = conns.entry(ip) {
                                        if *e.get() <= 1 {
                                            e.remove();
                                        } else {
                                            *e.get_mut() -= 1;
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                warn!("Error accepting inbound connection on {}: {}", actual_addr, e);
                                break;
                            }
                        }
                    }
                }
            }

            service_self.is_running.store(false, Ordering::SeqCst);
            *service_self.listen_addr.lock().await = None;
            service_self.upload_manager.abort_all().await;
            service_self.active_conns.lock().await.clear();
            info!("Inbound eD2K listener stopped");
        });

        *self.listener_handle.lock().await = Some(handle);
        Ok(actual_addr)
    }

    /// Stops the inbound listener cleanly.
    pub async fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        let handle = self.listener_handle.lock().await.take();
        if let Some(h) = handle {
            h.abort();
            let _ = h.await;
        }
        *self.listen_addr.lock().await = None;
        self.upload_manager.abort_all().await;
        self.active_conns.lock().await.clear();
    }

    /// Returns a snapshot of listener status.
    pub async fn snapshot(&self) -> InboundListenerSnapshot {
        let is_listening = self.is_running.load(Ordering::SeqCst);
        let addr = self.listen_addr.lock().await.map(|a| a.to_string());
        let active_snaps = self.upload_manager.active_snapshots().await;
        let queue = self.upload_manager.list_queue().await;
        let upload_rate = active_snaps.iter().map(|s| s.upload_rate_bps).sum();

        InboundListenerSnapshot {
            is_listening,
            listen_address: addr,
            active_uploads: active_snaps.len(),
            queued_uploads: queue.len(),
            upload_rate_bps: upload_rate,
        }
    }
}
