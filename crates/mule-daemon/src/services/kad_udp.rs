use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
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

/// Maximum number of concurrent in-flight transactions to prevent memory exhaustion.
pub const MAX_IN_FLIGHT_TRANSACTIONS: usize = 200;

/// Maximum requests per second allowed per source IP (anti-amplification / rate limiting).
pub const MAX_KAD_REQUESTS_PER_SEC_PER_IP: u32 = 20;

/// Simple per-IP sliding rate limiter.
#[derive(Debug, Default)]
struct IpRateLimiter {
    counts: HashMap<std::net::IpAddr, (u32, Instant)>,
}

impl IpRateLimiter {
    fn new() -> Self {
        Self { counts: HashMap::new() }
    }

    fn check_and_record(&mut self, ip: std::net::IpAddr) -> bool {
        let now = Instant::now();
        if self.counts.len() > 1000 {
            self.counts.retain(|_, (_, ts)| now.duration_since(*ts) < Duration::from_secs(10));
        }

        let entry = self.counts.entry(ip).or_insert((0, now));
        if now.duration_since(entry.1) >= Duration::from_secs(1) {
            *entry = (1, now);
            true
        } else if entry.0 < MAX_KAD_REQUESTS_PER_SEC_PER_IP {
            entry.0 += 1;
            true
        } else {
            false
        }
    }
}

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

impl KadUdpInner {
    async fn insert_in_flight(
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
        tracing::info!("Kad UDP listener started on {}", local_addr);

        // Spawn receive loop
        let in_flight_arc = inner.in_flight.clone();
        let socket_for_task = socket_arc.clone();
        let cancel_token = self.policy_mgr.cancellation_token();
        let local_kad_id_bytes = self.local_id.0;
        let rate_limiter_arc = Arc::new(Mutex::new(IpRateLimiter::new()));
        let ip_filter = self.policy_mgr.ip_filter();
        let profile = self.policy_mgr.profile();

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

                                let proto_byte = buf.first().copied().unwrap_or(0);
                                let opcode_byte = buf.get(1).copied().unwrap_or(0);

                                // 1. IP filter check (banned IPs)
                                if ip_filter.check_ip(peer_addr.ip()).is_some() {
                                    tracing::warn!("Kad UDP: dropped packet from banned IP {}", peer_addr.ip());
                                    continue;
                                }

                                // 2. Network profile confinement check (anti-SSRF)
                                let cat = mule_network_policy::validator::classify_ip(peer_addr.ip());
                                let policy_allowed = match profile {
                                    mule_network_policy::NetworkProfile::LocalLab => {
                                        cat == mule_network_policy::validator::IpCategory::Loopback
                                    }
                                    _ => {
                                        cat == mule_network_policy::validator::IpCategory::PublicRoutable
                                            || cat == mule_network_policy::validator::IpCategory::Loopback
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
                                        tracing::warn!("Kad UDP rate limit exceeded for {}", peer_addr.ip());
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
                                        Self::handle_incoming_message(
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
                                // On Windows, UDP sockets can receive WSAECONNRESET (10054) when a previously sent
                                // UDP packet resulted in an ICMP Port Unreachable message. Do NOT break the loop!
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
            inner.insert_in_flight(tx_id, in_flight_entry).await?;
        }

        let local_port = socket.local_addr().map(|a| a.port()).unwrap_or(0);
        let ping_bytes = KadCodec::encode_ping(&self.local_id.0, local_port);

        tracing::info!("Kad sending Ping (0x60) to node {} (tx_id={})", target, tx_id);
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
                tracing::warn!("Kad Ping to node {} timed out after {}s (tx_id={})", target, KAD_TRANSACTION_TIMEOUT_SECS, tx_id);
                Err(format!("Kad ping to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                let elapsed = start_time.elapsed();
                match res {
                    Ok(Ok(KadMessage::Pong(pong))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!("Kad received Pong (0x61) from node {} in {} ms (client_port={})", target, elapsed.as_millis(), pong.client_port);
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
            inner.insert_in_flight(tx_id, in_flight_entry).await?;
        }

        let local_port = socket.local_addr().map(|a| a.port()).unwrap_or(0);
        let boot_bytes = KadCodec::encode_bootstrap_req(&self.local_id.0, local_port);

        tracing::info!("Kad sending BootstrapReq (0x01) to node {} (tx_id={})", target, tx_id);
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
                tracing::warn!("Kad Bootstrap to node {} timed out after {}s (tx_id={})", target, KAD_TRANSACTION_TIMEOUT_SECS, tx_id);
                Err(format!("Kad bootstrap to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                match res {
                    Ok(Ok(KadMessage::BootstrapRes(boot_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!("Kad received BootstrapRes (0x09) from node {} with {} contacts", target, boot_res.contacts.len());
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
        receiver_kad_id: &KadId,
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
            expected_opcode: KADEMLIA2_RES,
            _created_at: Instant::now(),
            sender: tx,
        };

        {
            let inner = self.inner.read().await;
            inner.insert_in_flight(tx_id, in_flight_entry).await?;
        }

        let find_bytes = KadCodec::encode_find_node_req(&target_kad_id.0, &receiver_kad_id.0);
        tracing::info!(
            "Kad sending FindNodeReq (0x21) to node {} for target {} (receiver={}, tx_id={})",
            target,
            target_kad_id.to_hex(),
            receiver_kad_id.to_hex(),
            tx_id
        );

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
                tracing::warn!(
                    "Kad FindNode to node {} timed out after {}s (target={}, tx_id={})",
                    target,
                    KAD_TRANSACTION_TIMEOUT_SECS,
                    target_kad_id.to_hex(),
                    tx_id
                );
                Err(format!("Kad find_node to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                match res {
                    Ok(Ok(KadMessage::BootstrapRes(boot_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received FindNode response from node {} with {} contacts",
                            target,
                            boot_res.contacts.len()
                        );
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

    /// Sends a Kad2 Search Source request to target address and awaits discovered sources.
    pub async fn send_search_sources(
        &self,
        target: SocketAddr,
        target_file_hash: &[u8; 16],
        file_size: u64,
    ) -> Result<Vec<KadSourceWire>, String> {
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
            expected_opcode: KADEMLIA2_SEARCH_SOURCE_RES,
            _created_at: Instant::now(),
            sender: tx,
        };

        {
            let inner = self.inner.read().await;
            inner.insert_in_flight(tx_id, in_flight_entry).await?;
        }

        let search_bytes = KadCodec::encode_search_source_req(
            target_file_hash,
            0,
            file_size,
            &self.local_id.0,
        );

        tracing::info!(
            "Kad sending SearchSourceReq (0x22) to node {} for file {} (size={}, tx_id={})",
            target,
            hex::encode(target_file_hash),
            file_size,
            tx_id
        );

        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&search_bytes, target)
            .await
            .map_err(|e| format!("Failed to send UDP datagram: {e}"))?;

        let timeout_fut = tokio::time::sleep(Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS));

        tokio::select! {
            _ = timeout_fut => {
                let inner = self.inner.read().await;
                let mut inf = inner.in_flight.lock().await;
                inf.remove(&tx_id);
                tracing::warn!(
                    "Kad SearchSources to node {} timed out after {}s (file={}, tx_id={})",
                    target,
                    KAD_TRANSACTION_TIMEOUT_SECS,
                    hex::encode(target_file_hash),
                    tx_id
                );
                Err(format!("Kad search_sources to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                match res {
                    Ok(Ok(KadMessage::SearchSourceRes(search_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received SearchSourceRes from node {} with {} sources (tx_id={})",
                            target,
                            search_res.sources.len(),
                            tx_id
                        );
                        Ok(search_res.sources)
                    }
                    Ok(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Ok(Err(err)) => Err(err),
                    Err(_) => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }

    /// Sends a Kad2 Publish Source request to target address and awaits status response.
    pub async fn send_publish_source(
        &self,
        target: SocketAddr,
        target_file_hash: &[u8; 16],
        file_size: u64,
        tcp_port: u16,
    ) -> Result<u8, String> {
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
            expected_opcode: KADEMLIA2_PUBLISH_SOURCE_RES,
            _created_at: Instant::now(),
            sender: tx,
        };

        {
            let inner = self.inner.read().await;
            inner.insert_in_flight(tx_id, in_flight_entry).await?;
        }

        let local_addr = socket.local_addr().map_err(|e| e.to_string())?;
        let ip_bytes = match local_addr.ip() {
            std::net::IpAddr::V4(v4) => v4.octets(),
            std::net::IpAddr::V6(_) => [127, 0, 0, 1],
        };

        let contact = KadContactWire {
            kad_id: self.local_id.0,
            ip: ip_bytes,
            udp_port: local_addr.port(),
            tcp_port,
            version: 2,
        };

        let pub_bytes = KadCodec::encode_publish_source_req(
            target_file_hash,
            &contact,
            file_size,
        );

        tracing::info!(
            "Kad sending PublishSourceReq (0x23) to node {} for file {} (tx_id={})",
            target,
            hex::encode(target_file_hash),
            tx_id
        );

        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&pub_bytes, target)
            .await
            .map_err(|e| format!("Failed to send UDP datagram: {e}"))?;

        let timeout_fut = tokio::time::sleep(Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS));

        tokio::select! {
            _ = timeout_fut => {
                let inner = self.inner.read().await;
                let mut inf = inner.in_flight.lock().await;
                inf.remove(&tx_id);
                tracing::warn!(
                    "Kad PublishSource to node {} timed out after {}s (tx_id={})",
                    target,
                    KAD_TRANSACTION_TIMEOUT_SECS,
                    tx_id
                );
                Err(format!("Kad publish_source to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                match res {
                    Ok(Ok(KadMessage::PublishSourceRes(pub_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received PublishSourceRes from node {} with status {} (tx_id={})",
                            target,
                            pub_res.status,
                            tx_id
                        );
                        Ok(pub_res.status)
                    }
                    Ok(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Ok(Err(err)) => Err(err),
                    Err(_) => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }

    /// Sends a Kad2 Search Keyword request and waits for a SearchKeywordRes.
    pub async fn send_search_keyword(
        &self,
        target: SocketAddr,
        keyword_hash: &[u8; 16],
        start_pos: u16,
    ) -> Result<Vec<KadKeywordEntryWire>, String> {
        let (socket, in_flight) = {
            let inner = self.inner.read().await;
            let socket = inner.socket.clone().ok_or("UDP socket is not active")?;
            (socket, inner.in_flight.clone())
        };

        let _permit = self
            .policy_mgr
            .request_tcp_connect(target, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied Kad UDP send to {target}: {e}"))?;

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();

        {
            let mut inf = in_flight.lock().await;
            if inf.len() >= MAX_IN_FLIGHT_TRANSACTIONS {
                return Err(format!("Too many in-flight Kad transactions ({MAX_IN_FLIGHT_TRANSACTIONS} max)"));
            }
            inf.insert(
                tx_id,
                InFlightTransaction {
                    target,
                    expected_opcode: KADEMLIA2_SEARCH_KEYWORD_RES,
                    _created_at: Instant::now(),
                    sender: tx,
                },
            );
        }

        let search_bytes = KadCodec::encode_search_keyword_req(
            keyword_hash,
            start_pos,
            &self.local_id.0,
        );

        tracing::info!(
            "Kad sending SearchKeywordReq (0x24) to node {} for keyword {} (start_pos={}, tx_id={})",
            target,
            hex::encode(keyword_hash),
            start_pos,
            tx_id
        );

        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&search_bytes, target)
            .await
            .map_err(|e| format!("Failed to send UDP datagram: {e}"))?;

        let timeout_fut = tokio::time::sleep(Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS));

        tokio::select! {
            _ = timeout_fut => {
                let inner = self.inner.read().await;
                let mut inf = inner.in_flight.lock().await;
                inf.remove(&tx_id);
                tracing::warn!(
                    "Kad SearchKeyword to node {} timed out after {}s (keyword={}, tx_id={})",
                    target,
                    KAD_TRANSACTION_TIMEOUT_SECS,
                    hex::encode(keyword_hash),
                    tx_id
                );
                Err(format!("Kad search_keyword to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                match res {
                    Ok(Ok(KadMessage::SearchKeywordRes(search_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received SearchKeywordRes from node {} with {} entries (tx_id={})",
                            target,
                            search_res.entries.len(),
                            tx_id
                        );
                        Ok(search_res.entries)
                    }
                    Ok(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Ok(Err(err)) => Err(err),
                    Err(_) => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }

    /// Sends a Kad2 Publish Keyword request and waits for a PublishKeywordRes.
    pub async fn send_publish_keyword(
        &self,
        target: SocketAddr,
        keyword_hash: &[u8; 16],
        entries: &[KadKeywordEntryWire],
    ) -> Result<u8, String> {
        let (socket, in_flight) = {
            let inner = self.inner.read().await;
            let socket = inner.socket.clone().ok_or("UDP socket is not active")?;
            (socket, inner.in_flight.clone())
        };

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();

        {
            let mut inf = in_flight.lock().await;
            if inf.len() >= MAX_IN_FLIGHT_TRANSACTIONS {
                return Err(format!("Too many in-flight Kad transactions ({MAX_IN_FLIGHT_TRANSACTIONS} max)"));
            }
            inf.insert(
                tx_id,
                InFlightTransaction {
                    target,
                    expected_opcode: KADEMLIA2_PUBLISH_KEYWORD_RES,
                    _created_at: Instant::now(),
                    sender: tx,
                },
            );
        }

        let pub_bytes = KadCodec::encode_publish_keyword_req(keyword_hash, entries);

        tracing::info!(
            "Kad sending PublishKeywordReq (0x25) to node {} for keyword {} with {} entries (tx_id={})",
            target,
            hex::encode(keyword_hash),
            entries.len(),
            tx_id
        );

        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&pub_bytes, target)
            .await
            .map_err(|e| format!("Failed to send UDP datagram: {e}"))?;

        let timeout_fut = tokio::time::sleep(Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS));

        tokio::select! {
            _ = timeout_fut => {
                let inner = self.inner.read().await;
                let mut inf = inner.in_flight.lock().await;
                inf.remove(&tx_id);
                tracing::warn!(
                    "Kad PublishKeyword to node {} timed out after {}s (keyword={}, tx_id={})",
                    target,
                    KAD_TRANSACTION_TIMEOUT_SECS,
                    hex::encode(keyword_hash),
                    tx_id
                );
                Err(format!("Kad publish_keyword to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                match res {
                    Ok(Ok(KadMessage::PublishKeywordRes(pub_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received PublishKeywordRes from node {} with status {} (tx_id={})",
                            target,
                            pub_res.status,
                            tx_id
                        );
                        Ok(pub_res.status)
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
                tracing::info!("Kad UDP: received Ping from {peer_addr}, replying with Pong");
                let pong_bytes = KadCodec::encode_pong(peer_addr.port());
                let _ = socket.send_to(&pong_bytes, peer_addr).await;
            }
            KadMessage::Pong(pong) => {
                let mut inf = in_flight.lock().await;
                let mut matched_tx = None;
                for (id, tx) in inf.iter() {
                    if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip()) && tx.expected_opcode == KADEMLIA2_PONG {
                        matched_tx = Some(*id);
                        break;
                    }
                }
                tracing::info!("Kad UDP: received Pong from {peer_addr} (matched_tx={matched_tx:?})");
                if let Some(id) = matched_tx {
                    if let Some(tx) = inf.remove(&id) {
                        let _ = tx.sender.send(Ok(KadMessage::Pong(pong)));
                    }
                }
            }
            KadMessage::HelloReq(_hello) => {
                tracing::info!("Kad UDP: received HelloReq from {peer_addr}, replying with HelloRes");
                let hello_res = KadCodec::encode_hello_res(local_kad_id, local_port, 2, 0);
                let _ = socket.send_to(&hello_res, peer_addr).await;
            }
            KadMessage::HelloRes(hello_res) => {
                let mut inf = in_flight.lock().await;
                let mut matched_tx = None;
                for (id, tx) in inf.iter() {
                    if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip()) && tx.expected_opcode == KADEMLIA2_HELLO_RES {
                        matched_tx = Some(*id);
                        break;
                    }
                }
                tracing::info!("Kad UDP: received HelloRes from {peer_addr} (matched_tx={matched_tx:?})");
                if let Some(id) = matched_tx {
                    if let Some(tx) = inf.remove(&id) {
                        let _ = tx.sender.send(Ok(KadMessage::HelloRes(hello_res)));
                    }
                }
            }
            KadMessage::BootstrapReq(_) | KadMessage::Req(_) => {
                tracing::info!("Kad UDP: received BootstrapReq/Req from {peer_addr}, replying with empty contacts");
                let empty_res = KadCodec::encode_bootstrap_res(local_kad_id, local_port, 2, &[]);
                let _ = socket.send_to(&empty_res, peer_addr).await;
            }
            KadMessage::BootstrapRes(boot_res) => {
                let mut inf = in_flight.lock().await;
                let mut matched_tx = None;
                for (id, tx) in inf.iter() {
                    if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip()) && (tx.expected_opcode == KADEMLIA2_BOOTSTRAP_RES || tx.expected_opcode == KADEMLIA2_RES) {
                        matched_tx = Some(*id);
                        break;
                    }
                }
                tracing::info!(
                    "Kad UDP: received BootstrapRes/FindNodeRes from {peer_addr} with {} contacts (matched_tx={matched_tx:?})",
                    boot_res.contacts.len()
                );
                if let Some(id) = matched_tx {
                    if let Some(tx) = inf.remove(&id) {
                        let _ = tx.sender.send(Ok(KadMessage::BootstrapRes(boot_res)));
                    }
                }
            }
            KadMessage::SearchSourceReq(req) => {
                tracing::info!(
                    "Kad UDP: received SearchSourceReq from {peer_addr} for file {}, replying with empty",
                    hex::encode(req.target_file_hash)
                );
                let empty_res = KadCodec::encode_search_source_res(&req.target_file_hash, &[]);
                let _ = socket.send_to(&empty_res, peer_addr).await;
            }
            KadMessage::SearchSourceRes(search_res) => {
                let mut inf = in_flight.lock().await;
                let mut matched_tx = None;
                for (id, tx) in inf.iter() {
                    if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip())
                        && (tx.expected_opcode == KADEMLIA2_SEARCH_SOURCE_RES
                            || tx.expected_opcode == KADEMLIA2_SEARCH_RES)
                    {
                        matched_tx = Some(*id);
                        break;
                    }
                }
                tracing::info!(
                    "Kad UDP: received SearchSourceRes from {peer_addr} with {} sources for file {} (matched_tx={matched_tx:?})",
                    search_res.sources.len(),
                    hex::encode(search_res.target_file_hash)
                );
                if let Some(id) = matched_tx {
                    if let Some(tx) = inf.remove(&id) {
                        let _ = tx.sender.send(Ok(KadMessage::SearchSourceRes(search_res)));
                    }
                }
            }
            KadMessage::PublishSourceReq(req) => {
                let peer_ip_bytes = match peer_addr.ip() {
                    std::net::IpAddr::V4(v4) => v4.octets(),
                    std::net::IpAddr::V6(_) => [127, 0, 0, 1],
                };
                let valid_contact = req.contact.ip == peer_ip_bytes && req.contact.tcp_port > 0;
                let status = if valid_contact { 0 } else { 1 };
                tracing::info!(
                    "Kad UDP: received PublishSourceReq from {peer_addr} for file {} (status={})",
                    hex::encode(req.target_file_hash),
                    status
                );
                let res = KadCodec::encode_publish_source_res(&req.target_file_hash, status);
                let _ = socket.send_to(&res, peer_addr).await;
            }
            KadMessage::PublishSourceRes(pub_res) => {
                let mut inf = in_flight.lock().await;
                let mut matched_tx = None;
                for (id, tx) in inf.iter() {
                    if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip()) && tx.expected_opcode == KADEMLIA2_PUBLISH_SOURCE_RES {
                        matched_tx = Some(*id);
                        break;
                    }
                }
                tracing::info!(
                    "Kad UDP: received PublishSourceRes from {peer_addr} with status {} (matched_tx={matched_tx:?})",
                    pub_res.status
                );
                if let Some(id) = matched_tx {
                    if let Some(tx) = inf.remove(&id) {
                        let _ = tx.sender.send(Ok(KadMessage::PublishSourceRes(pub_res)));
                    }
                }
            }
            KadMessage::SearchKeywordReq(req) => {
                tracing::info!(
                    "Kad UDP: received SearchKeywordReq from {peer_addr} for keyword {}, replying with empty",
                    hex::encode(req.keyword_hash)
                );
                let empty_res = KadCodec::encode_search_keyword_res(&req.keyword_hash, &[]);
                let _ = socket.send_to(&empty_res, peer_addr).await;
            }
            KadMessage::SearchKeywordRes(search_res) => {
                let mut inf = in_flight.lock().await;
                let mut matched_tx = None;
                for (id, tx) in inf.iter() {
                    if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip())
                        && (tx.expected_opcode == KADEMLIA2_SEARCH_KEYWORD_RES
                            || tx.expected_opcode == KADEMLIA2_SEARCH_KEYWORD_RES_LEGACY
                            || tx.expected_opcode == KADEMLIA2_SEARCH_RES)
                    {
                        matched_tx = Some(*id);
                        break;
                    }
                }
                tracing::info!(
                    "Kad UDP: received SearchKeywordRes from {peer_addr} with {} entries for keyword {} (matched_tx={matched_tx:?})",
                    search_res.entries.len(),
                    hex::encode(search_res.keyword_hash)
                );
                if let Some(id) = matched_tx {
                    if let Some(tx) = inf.remove(&id) {
                        let _ = tx.sender.send(Ok(KadMessage::SearchKeywordRes(search_res)));
                    }
                }
            }
            KadMessage::PublishKeywordReq(req) => {
                tracing::info!(
                    "Kad UDP: received PublishKeywordReq from {peer_addr} for keyword {}",
                    hex::encode(req.keyword_hash)
                );
                let res = KadCodec::encode_publish_keyword_res(&req.keyword_hash, 0);
                let _ = socket.send_to(&res, peer_addr).await;
            }
            KadMessage::PublishKeywordRes(pub_res) => {
                let mut inf = in_flight.lock().await;
                let mut matched_tx = None;
                for (id, tx) in inf.iter() {
                    if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip()) && tx.expected_opcode == KADEMLIA2_PUBLISH_KEYWORD_RES {
                        matched_tx = Some(*id);
                        break;
                    }
                }
                tracing::info!(
                    "Kad UDP: received PublishKeywordRes from {peer_addr} with status {} (matched_tx={matched_tx:?})",
                    pub_res.status
                );
                if let Some(id) = matched_tx {
                    if let Some(tx) = inf.remove(&id) {
                        let _ = tx.sender.send(Ok(KadMessage::PublishKeywordRes(pub_res)));
                    }
                }
            }
        }
    }
}
