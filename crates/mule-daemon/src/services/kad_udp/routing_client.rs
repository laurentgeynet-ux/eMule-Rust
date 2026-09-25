// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use crate::services::peer_session::log_peer_addr;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use mule_core::kad::KadContact;
use mule_network_policy::ConnectionPurpose;
use mule_protocol::kad::*;

use super::types::{InFlightTransaction, KadExpectedResponse, KAD_TRANSACTION_TIMEOUT_SECS};
use super::KadUdpService;
use mule_protocol::ed2k::packed::pack_kad_datagram;

impl KadUdpService {
    /// Sends a Kad Ping to target address and awaits Pong response within timeout.
    pub async fn ping(&self, target: SocketAddr) -> Result<(KadPong, Duration), String> {
        let socket = {
            let inner = self.inner.read().await;
            inner
                .socket
                .clone()
                .ok_or_else(|| "Kad UDP socket is not running".to_string())?
        };

        let _permit = self
            .policy_mgr
            .request_tcp_connect(target, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied Kad UDP send to {target}: {e}"))?;

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::SeqCst);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let in_flight_entry = InFlightTransaction {
            target,
            expected_response: KadExpectedResponse::Pong,
            expected_target_hash: None,
            created_at: Instant::now(),
            sender: tx,
        };

        let _tx_guard = {
            let inner = self.inner.read().await;
            inner.insert_in_flight(tx_id, in_flight_entry).await?
        };

        let local_port = socket.local_addr().map(|a| a.port()).unwrap_or(0);
        let ping_bytes = KadCodec::encode_ping(&self.local_id.read().unwrap().0, local_port);

        tracing::debug!(
            "Kad sending Ping (0x60) to node {} (tx_id={})",
            log_peer_addr(target),
            tx_id
        );
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&pack_kad_datagram(&ping_bytes), target)
            .await
            .map_err(|e| format!("Failed to send UDP datagram: {e}"))?;

        let start_time = Instant::now();
        let timeout_fut = tokio::time::sleep(Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS));

        tokio::select! {
            _ = timeout_fut => {
                let inner = self.inner.read().await;
                let mut inf = inner.in_flight.lock().await;
                inf.remove(&tx_id);
                tracing::warn!("Kad Ping to node {} timed out after {}s (tx_id={})", log_peer_addr(target), KAD_TRANSACTION_TIMEOUT_SECS, tx_id);
                Err(format!("Kad ping to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx.recv() => {
                let elapsed = start_time.elapsed();
                match res {
                    Some(Ok(KadMessage::Pong(pong))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::debug!("Kad received Pong (0x61) from node {} in {} ms (client_port={})", log_peer_addr(target), elapsed.as_millis(), pong.client_port);
                        Ok((pong, elapsed))
                    }
                    Some(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Some(Err(err)) => Err(err),
                    None => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }

    /// Sends a Kad2 Bootstrap request to target address and awaits response with contacts.
    pub async fn send_bootstrap(&self, target: SocketAddr) -> Result<Vec<KadContact>, String> {
        let socket = {
            let inner = self.inner.read().await;
            inner
                .socket
                .clone()
                .ok_or_else(|| "Kad UDP socket is not running".to_string())?
        };

        let _permit = self
            .policy_mgr
            .request_tcp_connect(target, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied Kad UDP send to {target}: {e}"))?;

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::SeqCst);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let in_flight_entry = InFlightTransaction {
            target,
            expected_response: KadExpectedResponse::BootstrapRes,
            expected_target_hash: None,
            created_at: Instant::now(),
            sender: tx,
        };

        let _tx_guard = {
            let inner = self.inner.read().await;
            inner.insert_in_flight(tx_id, in_flight_entry).await?
        };

        let local_port = socket.local_addr().map(|a| a.port()).unwrap_or(0);
        let boot_bytes =
            KadCodec::encode_bootstrap_req(&self.local_id.read().unwrap().0, local_port);

        tracing::debug!(
            "Kad sending BootstrapReq (0x01) to node {} (tx_id={})",
            log_peer_addr(target),
            tx_id
        );
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&pack_kad_datagram(&boot_bytes), target)
            .await
            .map_err(|e| format!("Failed to send UDP datagram: {e}"))?;

        let timeout_fut = tokio::time::sleep(Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS));

        tokio::select! {
            _ = timeout_fut => {
                let inner = self.inner.read().await;
                let mut inf = inner.in_flight.lock().await;
                inf.remove(&tx_id);
                tracing::warn!("Kad Bootstrap to node {} timed out after {}s (tx_id={})", log_peer_addr(target), KAD_TRANSACTION_TIMEOUT_SECS, tx_id);
                Err(format!("Kad bootstrap to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx.recv() => {
                match res {
                    Some(Ok(KadMessage::BootstrapRes(boot_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::debug!("Kad received BootstrapRes (0x09) from node {} with {} contacts", log_peer_addr(target), boot_res.contacts.len());
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        let mut contacts = Vec::with_capacity(boot_res.contacts.len() + 1);
                        // eMule inserts the responding node itself into the
                        // routing table (BOOTSTRAP_RES header = its KadID and
                        // TCP port; the UDP port is the socket we reached it on).
                        if let IpAddr::V4(responder_ip) = target.ip() {
                            contacts.push(KadContact {
                                kad_id: mule_core::kad::KadId(boot_res.kad_id),
                                ip: responder_ip,
                                udp_port: target.port(),
                                tcp_port: if boot_res.udp_port != 0 {
                                    Some(boot_res.udp_port)
                                } else {
                                    None
                                },
                                version: boot_res.version,
                                verified: true,
                                fail_count: 0,
                                first_seen_unix_ms: now_ms,
                                last_seen_unix_ms: now_ms,
                            });
                        }
                        for c in boot_res.contacts {
                            let tcp_port = if c.tcp_port != 0 { Some(c.tcp_port) } else { None };
                            contacts.push(KadContact {
                                kad_id: mule_core::kad::KadId(c.kad_id),
                                ip: std::net::Ipv4Addr::from(c.ip),
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
                    Some(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Some(Err(err)) => Err(err),
                    None => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }

    /// Sends a Kad2 Find Node request to target address and awaits nearest contacts.
    pub async fn send_find_node(
        &self,
        target: SocketAddr,
        target_kad_id: &mule_core::kad::KadId,
        receiver_kad_id: &mule_core::kad::KadId,
    ) -> Result<Vec<KadContact>, String> {
        let socket = {
            let inner = self.inner.read().await;
            inner
                .socket
                .clone()
                .ok_or_else(|| "Kad UDP socket is not running".to_string())?
        };

        let _permit = self
            .policy_mgr
            .request_tcp_connect(target, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied Kad UDP send to {target}: {e}"))?;

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::SeqCst);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let in_flight_entry = InFlightTransaction {
            target,
            expected_response: KadExpectedResponse::FindNodeRes,
            expected_target_hash: None,
            created_at: Instant::now(),
            sender: tx,
        };

        let _tx_guard = {
            let inner = self.inner.read().await;
            inner.insert_in_flight(tx_id, in_flight_entry).await?
        };

        let find_bytes = KadCodec::encode_find_node_req(&target_kad_id.0, &receiver_kad_id.0);
        tracing::debug!(
            "Kad sending FindNodeReq (0x21) to node {} for target {} (receiver={}, tx_id={})",
            log_peer_addr(target),
            target_kad_id.to_hex(),
            receiver_kad_id.to_hex(),
            tx_id
        );

        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&pack_kad_datagram(&find_bytes), target)
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
                    log_peer_addr(target),
                    KAD_TRANSACTION_TIMEOUT_SECS,
                    target_kad_id.to_hex(),
                    tx_id
                );
                Err(format!("Kad find_node to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx.recv() => {
                match res {
                    Some(Ok(KadMessage::BootstrapRes(boot_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::debug!(
                            "Kad received FindNode response from node {} with {} contacts",
                            log_peer_addr(target),
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
                                kad_id: mule_core::kad::KadId(c.kad_id),
                                ip: std::net::Ipv4Addr::from(c.ip),
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
                    Some(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Some(Err(err)) => Err(err),
                    None => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }
}
