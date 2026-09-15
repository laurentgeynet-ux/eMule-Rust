use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio::task::JoinHandle;

use mule_core::kad::{KadContact, KadId};
use mule_network_policy::{ConnectionPurpose, ListenPermit, NetworkPolicyManager};
use mule_protocol::kad::*;

/// Transaction timeout in seconds.
pub const KAD_TRANSACTION_TIMEOUT_SECS: u64 = 5;

/// Pending in-flight transaction state.
struct InFlightTransaction {
    target: SocketAddr,
    expected_opcode: u8,
    _created_at: Instant,
    sender: oneshot::Sender<Result<KadMessage, String>>,
}

/// Statistics snapshot of the UDP Kad service.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KadUdpSnapshot {
    pub is_active: bool,
    pub bind_address: Option<String>,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub active_transactions: usize,
}

/// Inner state of the Kad UDP service.
struct KadUdpInner {
    socket: Option<Arc<UdpSocket>>,
    listen_permit: Option<ListenPermit>,
    bind_addr: Option<SocketAddr>,
    listen_task: Option<JoinHandle<()>>,
    in_flight: Arc<Mutex<HashMap<u32, InFlightTransaction>>>,
}

/// Kad UDP Service managing transport and transaction lifecycles.
pub struct KadUdpService {
    local_id: KadId,
    policy_mgr: NetworkPolicyManager,
    inner: Arc<RwLock<KadUdpInner>>,
    next_tx_id: AtomicU32,
    packets_sent: AtomicU64,
    packets_received: AtomicU64,
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
            next_tx_id: AtomicU32::new(rand::random::<u32>() | 1),
            packets_sent: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
        }
    }

    /// Starts the UDP socket on the specified address (strictly loopback in LocalLab).
    pub async fn start(&self, requested_addr: SocketAddr) -> Result<SocketAddr, String> {
        let mut inner = self.inner.write().await;
        if inner.socket.is_some() {
            return Err("Kad UDP socket is already active".to_string());
        }

        // Acquire listen permit under policy
        let permit = self
            .policy_mgr
            .request_tcp_listen(requested_addr, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied UDP listen on {requested_addr}: {e}"))?;

        let std_socket = std::net::UdpSocket::bind(requested_addr)
            .map_err(|e| format!("Failed to bind UDP socket on {requested_addr}: {e}"))?;
        std_socket
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set nonblocking: {e}"))?;

        let tokio_socket = UdpSocket::from_std(std_socket)
            .map_err(|e| format!("Failed to convert to tokio UdpSocket: {e}"))?;
        let local_addr = tokio_socket
            .local_addr()
            .map_err(|e| format!("Failed to get local address: {e}"))?;

        let socket_arc = Arc::new(tokio_socket);
        inner.socket = Some(socket_arc.clone());
        inner.bind_addr = Some(local_addr);
        inner.listen_permit = Some(permit);

        // Spawn receive loop
        let in_flight_arc = inner.in_flight.clone();
        let socket_for_task = socket_arc.clone();
        let cancel_token = self.policy_mgr.cancellation_token();
        let local_kad_id_bytes = self.local_id.0;

        let task = tokio::spawn(async move {
            let mut buf = vec![0u8; MAX_KAD_UDP_PACKET_SIZE];
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        break;
                    }
                    recv_res = socket_for_task.recv_from(&mut buf) => {
                        match recv_res {
                            Ok((len, peer_addr)) => {
                                if len == 0 || len > MAX_KAD_UDP_PACKET_SIZE {
                                    continue;
                                }
                                let packet_slice = &buf[..len];
                                match KadCodec::decode(packet_slice) {
                                    Ok(msg) => {
                                        Self::handle_incoming_message(
                                            &socket_for_task,
                                            peer_addr,
                                            msg,
                                            &in_flight_arc,
                                            &local_kad_id_bytes,
                                            local_addr.port(),
                                        ).await;
                                    }
                                    Err(_) => {
                                        // Ignore malformed datagrams defensively
                                    }
                                }
                            }
                            Err(_) => {
                                break;
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

    /// Sends a Kad Ping to target address and awaits Pong response within timeout.
    pub async fn ping(
        &self,
        target: SocketAddr,
    ) -> Result<(KadPong, Duration), String> {
        let socket = {
            let inner = self.inner.read().await;
            inner.socket.clone().ok_or_else(|| "Kad UDP socket is not running".to_string())?
        };

        let _permit = self
            .policy_mgr
            .request_tcp_connect(target, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied Kad UDP send to {target}: {e}"))?;

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        let in_flight_entry = InFlightTransaction {
            target,
            expected_opcode: KADEMLIA2_PONG,
            _created_at: Instant::now(),
            sender: tx,
        };

        {
            let inner = self.inner.read().await;
            let mut inf = inner.in_flight.lock().await;
            inf.insert(tx_id, in_flight_entry);
        }

        let local_port = socket.local_addr().map(|a| a.port()).unwrap_or(0);
        let ping_bytes = KadCodec::encode_ping(&self.local_id.0, local_port);

        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&ping_bytes, target)
            .await
            .map_err(|e| format!("Failed to send UDP datagram: {e}"))?;

        let start_time = Instant::now();
        let timeout_fut = tokio::time::sleep(Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS));

        tokio::select! {
            _ = timeout_fut => {
                let inner = self.inner.read().await;
                let mut inf = inner.in_flight.lock().await;
                inf.remove(&tx_id);
                Err(format!("Kad ping to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                let elapsed = start_time.elapsed();
                match res {
                    Ok(Ok(KadMessage::Pong(pong))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        Ok((pong, elapsed))
                    }
                    Ok(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Ok(Err(err)) => Err(err),
                    Err(_) => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }

    /// Sends a Kad2 Bootstrap request to target address and awaits response with contacts.
    pub async fn send_bootstrap(
        &self,
        target: SocketAddr,
    ) -> Result<Vec<KadContact>, String> {
        let socket = {
            let inner = self.inner.read().await;
            inner.socket.clone().ok_or_else(|| "Kad UDP socket is not running".to_string())?
        };

        let _permit = self
            .policy_mgr
            .request_tcp_connect(target, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied Kad UDP send to {target}: {e}"))?;

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        let in_flight_entry = InFlightTransaction {
            target,
            expected_opcode: KADEMLIA2_BOOTSTRAP_RES,
            _created_at: Instant::now(),
            sender: tx,
        };

        {
            let inner = self.inner.read().await;
            let mut inf = inner.in_flight.lock().await;
            inf.insert(tx_id, in_flight_entry);
        }

        let local_port = socket.local_addr().map(|a| a.port()).unwrap_or(0);
        let boot_bytes = KadCodec::encode_bootstrap_req(&self.local_id.0, local_port);

        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&boot_bytes, target)
            .await
            .map_err(|e| format!("Failed to send UDP datagram: {e}"))?;

        let timeout_fut = tokio::time::sleep(Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS));

        tokio::select! {
            _ = timeout_fut => {
                let inner = self.inner.read().await;
                let mut inf = inner.in_flight.lock().await;
                inf.remove(&tx_id);
                Err(format!("Kad bootstrap to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                match res {
                    Ok(Ok(KadMessage::BootstrapRes(boot_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        let mut contacts = Vec::with_capacity(boot_res.contacts.len());
                        for c in boot_res.contacts {
                            let tcp_port = if c.tcp_port != 0 { Some(c.tcp_port) } else { None };
                            contacts.push(KadContact {
                                kad_id: KadId(c.kad_id),
                                ip: Ipv4Addr::from(c.ip),
                                udp_port: c.udp_port,
                                tcp_port,
                                version: c.version,
                                verified: false,
                                fail_count: 0,
                                first_seen_unix_ms: now_ms,
                                last_seen_unix_ms: now_ms,
                            });
                        }
                        Ok(contacts)
                    }
                    Ok(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Ok(Err(err)) => Err(err),
                    Err(_) => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }

    /// Sends a Kad2 Find Node request to target address and awaits nearest contacts.
    pub async fn send_find_node(
        &self,
        target: SocketAddr,
        target_kad_id: &KadId,
    ) -> Result<Vec<KadContact>, String> {
        let socket = {
            let inner = self.inner.read().await;
            inner.socket.clone().ok_or_else(|| "Kad UDP socket is not running".to_string())?
        };

        let _permit = self
            .policy_mgr
            .request_tcp_connect(target, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied Kad UDP send to {target}: {e}"))?;

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        let in_flight_entry = InFlightTransaction {
            target,
            expected_opcode: KADEMLIA2_BOOTSTRAP_RES, // 0x29 handles both bootstrap and find node res
            _created_at: Instant::now(),
            sender: tx,
        };

        {
            let inner = self.inner.read().await;
            let mut inf = inner.in_flight.lock().await;
            inf.insert(tx_id, in_flight_entry);
        }

        let find_bytes = KadCodec::encode_find_node_req(&target_kad_id.0, &self.local_id.0);

        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&find_bytes, target)
            .await
            .map_err(|e| format!("Failed to send UDP datagram: {e}"))?;

        let timeout_fut = tokio::time::sleep(Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS));

        tokio::select! {
            _ = timeout_fut => {
                let inner = self.inner.read().await;
                let mut inf = inner.in_flight.lock().await;
                inf.remove(&tx_id);
                Err(format!("Kad find_node to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                match res {
                    Ok(Ok(KadMessage::BootstrapRes(boot_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        let mut contacts = Vec::with_capacity(boot_res.contacts.len());
                        for c in boot_res.contacts {
                            let tcp_port = if c.tcp_port != 0 { Some(c.tcp_port) } else { None };
                            contacts.push(KadContact {
                                kad_id: KadId(c.kad_id),
                                ip: Ipv4Addr::from(c.ip),
                                udp_port: c.udp_port,
                                tcp_port,
                                version: c.version,
                                verified: false,
                                fail_count: 0,
                                first_seen_unix_ms: now_ms,
                                last_seen_unix_ms: now_ms,
                            });
                        }
                        Ok(contacts)
                    }
                    Ok(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Ok(Err(err)) => Err(err),
                    Err(_) => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }

    /// Internal dispatcher for incoming messages.
    async fn handle_incoming_message(
        socket: &Arc<UdpSocket>,
        peer_addr: SocketAddr,
        msg: KadMessage,
        in_flight: &Arc<Mutex<HashMap<u32, InFlightTransaction>>>,
        local_kad_id: &[u8; 16],
        local_port: u16,
    ) {
        match msg {
            KadMessage::Ping(_ping) => {
                let pong_bytes = KadCodec::encode_pong(peer_addr.port());
                let _ = socket.send_to(&pong_bytes, peer_addr).await;
            }
            KadMessage::Pong(pong) => {
                let mut inf = in_flight.lock().await;
                let mut matched_tx = None;
                for (id, tx) in inf.iter() {
                    if tx.target == peer_addr && tx.expected_opcode == KADEMLIA2_PONG {
                        matched_tx = Some(*id);
                        break;
                    }
                }
                if let Some(id) = matched_tx {
                    if let Some(tx) = inf.remove(&id) {
                        let _ = tx.sender.send(Ok(KadMessage::Pong(pong)));
                    }
                }
            }
            KadMessage::HelloReq(_hello) => {
                let hello_res = KadCodec::encode_hello_res(local_kad_id, local_port, 2, 0);
                let _ = socket.send_to(&hello_res, peer_addr).await;
            }
            KadMessage::HelloRes(hello_res) => {
                let mut inf = in_flight.lock().await;
                let mut matched_tx = None;
                for (id, tx) in inf.iter() {
                    if tx.target == peer_addr && tx.expected_opcode == KADEMLIA2_HELLO_RES {
                        matched_tx = Some(*id);
                        break;
                    }
                }
                if let Some(id) = matched_tx {
                    if let Some(tx) = inf.remove(&id) {
                        let _ = tx.sender.send(Ok(KadMessage::HelloRes(hello_res)));
                    }
                }
            }
            KadMessage::BootstrapReq(_) | KadMessage::Req(_) => {
                // Return empty list of contacts for now when queried
                let empty_res = KadCodec::encode_bootstrap_res(local_kad_id, local_port, 2, &[]);
                let _ = socket.send_to(&empty_res, peer_addr).await;
            }
            KadMessage::BootstrapRes(boot_res) => {
                let mut inf = in_flight.lock().await;
                let mut matched_tx = None;
                for (id, tx) in inf.iter() {
                    if tx.target == peer_addr && tx.expected_opcode == KADEMLIA2_BOOTSTRAP_RES {
                        matched_tx = Some(*id);
                        break;
                    }
                }
                if let Some(id) = matched_tx {
                    if let Some(tx) = inf.remove(&id) {
                        let _ = tx.sender.send(Ok(KadMessage::BootstrapRes(boot_res)));
                    }
                }
            }
        }
    }
}
