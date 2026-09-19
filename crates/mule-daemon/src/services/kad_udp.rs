pub mod handlers;
pub mod routing_client;
pub mod search_client;
pub mod types;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

use mule_core::kad::KadId;
use mule_network_policy::{classify_ip, ConnectionPurpose, IpCategory, ListenPermit, NetworkPolicyManager, NetworkProfile};
use mule_protocol::kad::*;

pub use types::{
    InFlightTransaction, IpRateLimiter, KadUdpSnapshot, KAD_TRANSACTION_TIMEOUT_SECS,
    MAX_IN_FLIGHT_TRANSACTIONS, MAX_KAD_REQUESTS_PER_SEC_PER_IP,
};

/// Inner state of the Kad UDP service.
pub struct KadUdpInner {
    pub socket: Option<Arc<UdpSocket>>,
    pub listen_permit: Option<ListenPermit>,
    pub bind_addr: Option<SocketAddr>,
    pub listen_task: Option<JoinHandle<()>>,
    pub in_flight: Arc<Mutex<HashMap<u32, InFlightTransaction>>>,
}

impl KadUdpInner {
    pub async fn insert_in_flight(
        &self,
        tx_id: u32,
        entry: InFlightTransaction,
    ) -> Result<(), String> {
        let mut inf = self.in_flight.lock().await;
        if inf.len() >= MAX_IN_FLIGHT_TRANSACTIONS {
            return Err(format!(
                "Too many in-flight Kad transactions ({MAX_IN_FLIGHT_TRANSACTIONS} max)"
            ));
        }
        inf.insert(tx_id, entry);
        Ok(())
    }
}

/// Kad UDP Service managing transport and transaction lifecycles.
pub struct KadUdpService {
    pub(crate) local_id: KadId,
    pub(crate) policy_mgr: NetworkPolicyManager,
    pub(crate) inner: Arc<RwLock<KadUdpInner>>,
    pub(crate) next_tx_id: AtomicU32,
    pub(crate) packets_sent: AtomicU64,
    pub(crate) packets_received: AtomicU64,
}

impl KadUdpService {
    /// Creates a new Kad UDP service instance.
    pub fn new(local_id: KadId, policy_mgr: NetworkPolicyManager) -> Self {
        Self {
            local_id,
            policy_mgr,
            inner: Arc::new(RwLock::new(KadUdpInner {
                socket: None,
                listen_permit: None,
                bind_addr: None,
                listen_task: None,
                in_flight: Arc::new(Mutex::new(HashMap::new())),
            })),
            next_tx_id: AtomicU32::new(1),
            packets_sent: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
        }
    }

    /// Starts the UDP listener on specified bind address under network policy compliance.
    pub async fn start(&self, bind_addr: SocketAddr) -> Result<SocketAddr, String> {
        let mut inner = self.inner.write().await;
        if inner.socket.is_some() {
            if let Some(addr) = inner.bind_addr {
                return Ok(addr);
            }
        }

        let permit = self
            .policy_mgr
            .request_tcp_listen(bind_addr, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied Kad UDP bind on {bind_addr}: {e}"))?;

        let socket = UdpSocket::bind(&bind_addr)
            .await
            .map_err(|e| format!("Failed to bind Kad UDP socket to {bind_addr}: {e}"))?;

        let local_addr = socket
            .local_addr()
            .map_err(|e| format!("Failed to get local addr: {e}"))?;

        let socket_arc = Arc::new(socket);
        inner.socket = Some(socket_arc.clone());
        inner.bind_addr = Some(local_addr);
        inner.listen_permit = Some(permit);

        let socket_for_task = socket_arc.clone();
        let in_flight_arc = inner.in_flight.clone();
        let local_kad_id_bytes = self.local_id.0;
        let policy_mgr_clone = self.policy_mgr.clone();
        let rate_limiter_arc = Arc::new(Mutex::new(IpRateLimiter::new()));

        let task = tokio::spawn(async move {
            // Official eMule allocates a 64KB receive buffer to guarantee that incoming UDP datagrams
            // up to the maximum IP payload (65507 bytes) never fail with WSAEMSGSIZE (OS error 10040).
            let mut buf = vec![0u8; 65536];
            loop {
                tokio::select! {
                    recv_res = socket_for_task.recv_from(&mut buf) => {
                        match recv_res {
                            Ok((len, peer_addr)) => {
                                if len < 2 {
                                    continue;
                                }
                                let proto_byte = buf[0];
                                let opcode_byte = buf[1];

                                // 1. Check protocol identifier
                                if !is_valid_kad_protocol(proto_byte) {
                                    tracing::warn!(
                                        "Kad UDP: dropped invalid protocol byte 0x{:02X} from {}",
                                        proto_byte,
                                        peer_addr
                                    );
                                    continue;
                                }

                                // 2. Policy-based IP classification and conformance
                                let cat = classify_ip(peer_addr.ip());
                                let policy_allowed = match policy_mgr_clone.profile() {
                                    NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                                    NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                                        cat == IpCategory::PublicRoutable
                                    }
                                };
                                if !policy_allowed {
                                    tracing::warn!("Kad UDP: dropped packet from non-conforming IP {} ({:?})", peer_addr.ip(), cat);
                                    continue;
                                }

                                // 3. Anti-amplification / rate limiting per source IP
                                {
                                    let mut rl = rate_limiter_arc.lock().await;
                                    if !rl.check_and_record(peer_addr.ip()) {
                                        tracing::debug!("Kad UDP rate limit exceeded for {}", peer_addr.ip());
                                        continue;
                                    }
                                }

                                let packet_slice = &buf[..len];
                                tracing::info!(
                                    "Kad UDP recv {} bytes from {} (proto=0x{:02X}, opcode=0x{:02X})",
                                    len,
                                    peer_addr,
                                    proto_byte,
                                    opcode_byte
                                );

                                match KadCodec::decode(packet_slice) {
                                    Ok(msg) => {
                                        handlers::handle_incoming_message(
                                            &socket_for_task,
                                            peer_addr,
                                            msg,
                                            &in_flight_arc,
                                            &local_kad_id_bytes,
                                            local_addr.port(),
                                        ).await;
                                    }
                                    Err(err) => {
                                        tracing::warn!(
                                            "Kad UDP: decode error from {} (proto=0x{:02X}, opcode=0x{:02X}, len={}): {} (prefix: {:02X?})",
                                            peer_addr,
                                            proto_byte,
                                            opcode_byte,
                                            len,
                                            err,
                                            &packet_slice[..packet_slice.len().min(16)]
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Kad UDP: recv_from error: {e}");
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                        }
                    }
                }
            }
        });

        inner.listen_task = Some(task);
        Ok(local_addr)
    }

    /// Stops the UDP socket, drops permits, and cancels pending transactions.
    pub async fn stop(&self) -> Result<(), String> {
        let mut inner = self.inner.write().await;
        if let Some(task) = inner.listen_task.take() {
            task.abort();
        }
        inner.socket = None;
        inner.bind_addr = None;
        inner.listen_permit = None;

        // Abort all in-flight transactions
        let mut in_flight = inner.in_flight.lock().await;
        for (_, tx) in in_flight.drain() {
            let _ = tx.sender.send(Err("Kad UDP socket stopped".to_string()));
        }
        Ok(())
    }

    /// Returns a status snapshot of the Kad UDP service.
    pub async fn get_snapshot(&self) -> KadUdpSnapshot {
        let inner = self.inner.read().await;
        let in_flight_count = {
            let inf = inner.in_flight.lock().await;
            inf.len()
        };

        KadUdpSnapshot {
            is_active: inner.socket.is_some(),
            bind_address: inner.bind_addr.map(|a| a.to_string()),
            packets_sent: self.packets_sent.load(Ordering::Relaxed),
            packets_received: self.packets_received.load(Ordering::Relaxed),
            active_transactions: in_flight_count,
        }
    }
}
