use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

use mule_network_policy::ConnectionPurpose;
use mule_protocol::kad::*;

use super::types::{
    InFlightTransaction, KadExpectedResponse, KAD_TRANSACTION_TIMEOUT_SECS,
    MAX_IN_FLIGHT_TRANSACTIONS,
};
use super::KadUdpService;

impl KadUdpService {
    /// Sends a Kad2 Search Source request to target address and awaits discovered sources.
    pub async fn send_search_sources(
        &self,
        target: SocketAddr,
        target_file_hash: &[u8; 16],
        file_size: u64,
    ) -> Result<Vec<KadSourceWire>, String> {
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
        let (tx, rx) = oneshot::channel();
        let in_flight_entry = InFlightTransaction {
            target,
            expected_response: KadExpectedResponse::SearchSourceRes,
            expected_target_hash: Some(*target_file_hash),
            _created_at: Instant::now(),
            sender: tx,
        };

        {
            let inner = self.inner.read().await;
            inner.insert_in_flight(tx_id, in_flight_entry).await?;
        }

        let search_bytes = KadCodec::encode_search_source_req(target_file_hash, 0, file_size);

        tracing::info!(
            "Kad sending SearchSourceReq (0x34) to node {} for file {} (size={}, tx_id={})",
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
        let (tx, rx) = oneshot::channel();
        let in_flight_entry = InFlightTransaction {
            target,
            expected_response: KadExpectedResponse::PublishSourceRes,
            expected_target_hash: Some(*target_file_hash),
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

        let pub_bytes = KadCodec::encode_publish_source_req(target_file_hash, &contact, file_size);

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
                    Ok(Ok(KadMessage::PublishKeywordRes(pub_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received PublishRes from node {} with status {} (tx_id={})",
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
                return Err(format!(
                    "Too many in-flight Kad transactions ({MAX_IN_FLIGHT_TRANSACTIONS} max)"
                ));
            }
            inf.insert(
                tx_id,
                InFlightTransaction {
                    target,
                    expected_response: KadExpectedResponse::SearchKeywordRes,
                    expected_target_hash: Some(*keyword_hash),
                    _created_at: Instant::now(),
                    sender: tx,
                },
            );
        }

        let search_bytes =
            KadCodec::encode_search_keyword_req(keyword_hash, start_pos, &self.local_id.0);

        tracing::info!(
            "Kad sending SearchKeywordReq (0x33) to node {} for keyword {} (start_pos={}, tx_id={})",
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
                return Err(format!(
                    "Too many in-flight Kad transactions ({MAX_IN_FLIGHT_TRANSACTIONS} max)"
                ));
            }
            inf.insert(
                tx_id,
                InFlightTransaction {
                    target,
                    expected_response: KadExpectedResponse::PublishKeywordRes,
                    expected_target_hash: Some(*keyword_hash),
                    _created_at: Instant::now(),
                    sender: tx,
                },
            );
        }

        let pub_bytes = KadCodec::encode_publish_keyword_req(keyword_hash, entries);

        tracing::info!(
            "Kad sending PublishKeywordReq (0x43) to node {} for keyword {} with {} entries (tx_id={})",
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
                    Ok(Ok(KadMessage::PublishSourceRes(pub_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received PublishRes from node {} with status {} (tx_id={})",
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

    /// Sends a Kad2 Search Notes request (0x35) and waits for a notes
    /// response (unified 0x3B or legacy 0x3A).
    pub async fn send_search_notes(
        &self,
        target: SocketAddr,
        file_hash: &[u8; 16],
        file_size: u64,
    ) -> Result<Vec<KadNoteWire>, String> {
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
                return Err(format!(
                    "Too many in-flight Kad transactions ({MAX_IN_FLIGHT_TRANSACTIONS} max)"
                ));
            }
            inf.insert(
                tx_id,
                InFlightTransaction {
                    target,
                    expected_response: KadExpectedResponse::SearchNotesRes,
                    expected_target_hash: Some(*file_hash),
                    _created_at: Instant::now(),
                    sender: tx,
                },
            );
        }

        let req_bytes = KadCodec::encode_search_notes_req(file_hash, file_size);

        tracing::info!(
            "Kad sending SearchNotesReq (0x35) to node {} for file {} (size={}, tx_id={})",
            target,
            hex::encode(file_hash),
            file_size,
            tx_id
        );

        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&req_bytes, target)
            .await
            .map_err(|e| format!("Failed to send UDP datagram: {e}"))?;

        let timeout_fut = tokio::time::sleep(Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS));

        tokio::select! {
            _ = timeout_fut => {
                let inner = self.inner.read().await;
                let mut inf = inner.in_flight.lock().await;
                inf.remove(&tx_id);
                tracing::warn!(
                    "Kad SearchNotes to node {} timed out after {}s (file={}, tx_id={})",
                    target,
                    KAD_TRANSACTION_TIMEOUT_SECS,
                    hex::encode(file_hash),
                    tx_id
                );
                Err(format!("Kad search_notes to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                match res {
                    Ok(Ok(KadMessage::SearchNotesRes(notes_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received SearchNotesRes from node {} with {} notes (tx_id={})",
                            target,
                            notes_res.notes.len(),
                            tx_id
                        );
                        Ok(notes_res.notes)
                    }
                    Ok(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Ok(Err(err)) => Err(err),
                    Err(_) => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }

    /// Sends a Kad2 Publish Notes request (0x45) and waits for the
    /// shared 0x4B publish response (status byte = indexer load).
    pub async fn send_publish_notes(
        &self,
        target: SocketAddr,
        file_hash: &[u8; 16],
        source_id: &[u8; 16],
        tags: &[mule_protocol::ed2k::tags::Ed2kTag],
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
                return Err(format!(
                    "Too many in-flight Kad transactions ({MAX_IN_FLIGHT_TRANSACTIONS} max)"
                ));
            }
            inf.insert(
                tx_id,
                InFlightTransaction {
                    target,
                    expected_response: KadExpectedResponse::PublishNotesRes,
                    expected_target_hash: Some(*file_hash),
                    _created_at: Instant::now(),
                    sender: tx,
                },
            );
        }

        let Some(req_bytes) = KadCodec::encode_publish_notes_req(file_hash, source_id, tags) else {
            return Err("Failed to encode notes tag list (oversized)".to_string());
        };

        tracing::info!(
            "Kad sending PublishNotesReq (0x45) to node {} for file {} with {} tags (tx_id={})",
            target,
            hex::encode(file_hash),
            tags.len(),
            tx_id
        );

        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        socket
            .send_to(&req_bytes, target)
            .await
            .map_err(|e| format!("Failed to send UDP datagram: {e}"))?;

        let timeout_fut = tokio::time::sleep(Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS));

        tokio::select! {
            _ = timeout_fut => {
                let inner = self.inner.read().await;
                let mut inf = inner.in_flight.lock().await;
                inf.remove(&tx_id);
                tracing::warn!(
                    "Kad PublishNotes to node {} timed out after {}s (file={}, tx_id={})",
                    target,
                    KAD_TRANSACTION_TIMEOUT_SECS,
                    hex::encode(file_hash),
                    tx_id
                );
                Err(format!("Kad publish_notes to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx => {
                match res {
                    Ok(Ok(KadMessage::PublishNotesRes(pub_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received PublishNotesRes from node {} with load {} (tx_id={})",
                            target,
                            pub_res.load,
                            tx_id
                        );
                        Ok(pub_res.load)
                    }
                    Ok(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Ok(Err(err)) => Err(err),
                    Err(_) => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }
}
