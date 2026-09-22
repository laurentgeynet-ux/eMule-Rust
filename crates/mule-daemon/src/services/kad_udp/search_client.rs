use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use mule_network_policy::ConnectionPurpose;
use mule_protocol::kad::*;

use super::types::{InFlightTransaction, KadExpectedResponse, KAD_TRANSACTION_TIMEOUT_SECS};
use super::KadUdpService;

/// Idle window after the first search-response packet while more batches are
/// still accepted (eMule streams up to ~300 results in ~50-entry packets).
const SEARCH_RESP_IDLE_WINDOW: Duration = Duration::from_millis(350);
/// Safety caps on the multi-packet collection loop.
const MAX_SEARCH_COLLECT_PACKETS: usize = 12;
const MAX_SEARCH_COLLECT_ENTRIES: usize = 300;

/// Collects the follow-up batches of a multi-packet `SEARCH_*_RES` response.
/// The first packet was already consumed by the caller; eMule nodes stream
/// the remaining results in subsequent datagrams arriving back-to-back.
/// Bounded by an idle window, a packet count, and an entry cap — and by the
/// outer transaction timeout for the first packet.
async fn collect_search_batches<T>(
    rx: &mut mpsc::UnboundedReceiver<Result<KadMessage, String>>,
    mut acc: Vec<T>,
    extract: impl Fn(KadMessage) -> Option<Vec<T>>,
) -> Vec<T> {
    for _ in 0..MAX_SEARCH_COLLECT_PACKETS {
        if acc.len() >= MAX_SEARCH_COLLECT_ENTRIES {
            break;
        }
        match tokio::time::timeout(SEARCH_RESP_IDLE_WINDOW, rx.recv()).await {
            Ok(Some(Ok(msg))) => match extract(msg) {
                Some(more) => acc.extend(more),
                None => continue,
            },
            _ => break,
        }
    }
    acc
}

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
        let (tx, mut rx) = mpsc::unbounded_channel();
        let in_flight_entry = InFlightTransaction {
            target,
            expected_response: KadExpectedResponse::SearchSourceRes,
            expected_target_hash: Some(*target_file_hash),
            created_at: Instant::now(),
            sender: tx,
        };

        let _tx_guard = {
            let inner = self.inner.read().await;
            inner.insert_in_flight(tx_id, in_flight_entry).await?
        };

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
                // eMule ne répond rien quand le nœud n'indexe pas le fichier
                // (CIndexed::SendValidSourceResult) : le silence vaut « 0 sources »,
                // ce n'est pas une anomalie — niveau debug, pas warn.
                tracing::debug!(
                    "Kad SearchSources to node {} timed out after {}s (file={}, tx_id={})",
                    target,
                    KAD_TRANSACTION_TIMEOUT_SECS,
                    hex::encode(target_file_hash),
                    tx_id
                );
                Err(format!("Kad search_sources to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx.recv() => {
                match res {
                    Some(Ok(KadMessage::SearchSourceRes(search_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        let first_batch = search_res.sources.len();
                        let sources = collect_search_batches(&mut rx, search_res.sources, |msg| match msg {
                            KadMessage::SearchSourceRes(more) => Some(more.sources),
                            _ => None,
                        })
                        .await;
                        tracing::info!(
                            "Kad received SearchSourceRes from node {} with {} sources (first batch {}, tx_id={})",
                            target,
                            sources.len(),
                            first_batch,
                            tx_id
                        );
                        Ok(sources)
                    }
                    Some(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Some(Err(err)) => Err(err),
                    None => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }

    /// Sends a Kad2 Publish Source request to target address and awaits status response.
    ///
    /// `advertised_ip`/`advertised_udp_port` are our *public* endpoint: indexers
    /// reject the publish when the announced contact IP differs from the packet
    /// source (eMule `ProcessPublishSourceRequest`), so the caller resolves the
    /// observed/HighID address — never the socket bind address behind NAT.
    pub async fn send_publish_source(
        &self,
        target: SocketAddr,
        target_file_hash: &[u8; 16],
        file_size: u64,
        tcp_port: u16,
        advertised_ip: std::net::Ipv4Addr,
        advertised_udp_port: u16,
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
        let (tx, mut rx) = mpsc::unbounded_channel();
        let in_flight_entry = InFlightTransaction {
            target,
            expected_response: KadExpectedResponse::PublishSourceRes,
            expected_target_hash: Some(*target_file_hash),
            created_at: Instant::now(),
            sender: tx,
        };

        let _tx_guard = {
            let inner = self.inner.read().await;
            inner.insert_in_flight(tx_id, in_flight_entry).await?
        };

        let contact = KadContactWire {
            kad_id: self.local_id.read().unwrap().0,
            ip: advertised_ip.octets(),
            udp_port: advertised_udp_port,
            tcp_port,
            version: crate::services::kad::firewall::OUR_KAD_VERSION,
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
                // eMule only answers PUBLISH_RES when the source entry was
                // actually stored (out-of-tolerance targets drop silently) —
                // a timeout here is expected noise, not an anomaly.
                tracing::debug!(
                    "Kad PublishSource to node {} timed out after {}s (tx_id={})",
                    target,
                    KAD_TRANSACTION_TIMEOUT_SECS,
                    tx_id
                );
                Err(format!("Kad publish_source to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx.recv() => {
                match res {
                    Some(Ok(KadMessage::PublishSourceRes(pub_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received PublishSourceRes from node {} with status {} (tx_id={})",
                            target,
                            pub_res.status,
                            tx_id
                        );
                        Ok(pub_res.status)
                    }
                    Some(Ok(KadMessage::PublishKeywordRes(pub_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received PublishRes from node {} with status {} (tx_id={})",
                            target,
                            pub_res.status,
                            tx_id
                        );
                        Ok(pub_res.status)
                    }
                    Some(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Some(Err(err)) => Err(err),
                    None => Err("Transaction channel cancelled".to_string()),
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
        let socket = {
            let inner = self.inner.read().await;
            inner.socket.clone().ok_or("UDP socket is not active")?
        };

        let _permit = self
            .policy_mgr
            .request_tcp_connect(target, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied Kad UDP send to {target}: {e}"))?;

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::Relaxed);
        let (tx, mut rx) = mpsc::unbounded_channel();

        let _tx_guard = {
            let inner = self.inner.read().await;
            inner
                .insert_in_flight(
                    tx_id,
                    InFlightTransaction {
                        target,
                        expected_response: KadExpectedResponse::SearchKeywordRes,
                        expected_target_hash: Some(*keyword_hash),
                        created_at: Instant::now(),
                        sender: tx,
                    },
                )
                .await?
        };

        let search_bytes =
            KadCodec::encode_search_keyword_req(
                keyword_hash,
                start_pos,
                &self.local_id.read().unwrap().0,
            );

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
            res = rx.recv() => {
                match res {
                    Some(Ok(KadMessage::SearchKeywordRes(search_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        let first_batch = search_res.entries.len();
                        let entries = collect_search_batches(&mut rx, search_res.entries, |msg| match msg {
                            KadMessage::SearchKeywordRes(more) => Some(more.entries),
                            _ => None,
                        })
                        .await;
                        tracing::info!(
                            "Kad received SearchKeywordRes from node {} with {} entries (first batch {}, tx_id={})",
                            target,
                            entries.len(),
                            first_batch,
                            tx_id
                        );
                        Ok(entries)
                    }
                    Some(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Some(Err(err)) => Err(err),
                    None => Err("Transaction channel cancelled".to_string()),
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
        let socket = {
            let inner = self.inner.read().await;
            inner.socket.clone().ok_or("UDP socket is not active")?
        };

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::Relaxed);
        let (tx, mut rx) = mpsc::unbounded_channel();

        let _tx_guard = {
            let inner = self.inner.read().await;
            inner
                .insert_in_flight(
                    tx_id,
                    InFlightTransaction {
                        target,
                        expected_response: KadExpectedResponse::PublishKeywordRes,
                        expected_target_hash: Some(*keyword_hash),
                        created_at: Instant::now(),
                        sender: tx,
                    },
                )
                .await?
        };

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
                // Out-of-tolerance nodes silently drop PUBLISH_KEY_REQ —
                // keep the timeout visible but below WARN noise level.
                tracing::debug!(
                    "Kad PublishKeyword to node {} timed out after {}s (keyword={}, tx_id={})",
                    target,
                    KAD_TRANSACTION_TIMEOUT_SECS,
                    hex::encode(keyword_hash),
                    tx_id
                );
                Err(format!("Kad publish_keyword to {target} timed out after {KAD_TRANSACTION_TIMEOUT_SECS}s"))
            }
            res = rx.recv() => {
                match res {
                    Some(Ok(KadMessage::PublishKeywordRes(pub_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received PublishKeywordRes from node {} with status {} (tx_id={})",
                            target,
                            pub_res.status,
                            tx_id
                        );
                        Ok(pub_res.status)
                    }
                    Some(Ok(KadMessage::PublishSourceRes(pub_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received PublishRes from node {} with status {} (tx_id={})",
                            target,
                            pub_res.status,
                            tx_id
                        );
                        Ok(pub_res.status)
                    }
                    Some(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Some(Err(err)) => Err(err),
                    None => Err("Transaction channel cancelled".to_string()),
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
        let socket = {
            let inner = self.inner.read().await;
            inner.socket.clone().ok_or("UDP socket is not active")?
        };

        let _permit = self
            .policy_mgr
            .request_tcp_connect(target, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied Kad UDP send to {target}: {e}"))?;

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::Relaxed);
        let (tx, mut rx) = mpsc::unbounded_channel();

        let _tx_guard = {
            let inner = self.inner.read().await;
            inner
                .insert_in_flight(
                    tx_id,
                    InFlightTransaction {
                        target,
                        expected_response: KadExpectedResponse::SearchNotesRes,
                        expected_target_hash: Some(*file_hash),
                        created_at: Instant::now(),
                        sender: tx,
                    },
                )
                .await?
        };

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
            res = rx.recv() => {
                match res {
                    Some(Ok(KadMessage::SearchNotesRes(notes_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        let first_batch = notes_res.notes.len();
                        let notes = collect_search_batches(&mut rx, notes_res.notes, |msg| match msg {
                            KadMessage::SearchNotesRes(more) => Some(more.notes),
                            _ => None,
                        })
                        .await;
                        tracing::info!(
                            "Kad received SearchNotesRes from node {} with {} notes (first batch {}, tx_id={})",
                            target,
                            notes.len(),
                            first_batch,
                            tx_id
                        );
                        Ok(notes)
                    }
                    Some(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Some(Err(err)) => Err(err),
                    None => Err("Transaction channel cancelled".to_string()),
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
        let socket = {
            let inner = self.inner.read().await;
            inner.socket.clone().ok_or("UDP socket is not active")?
        };

        let tx_id = self.next_tx_id.fetch_add(1, Ordering::Relaxed);
        let (tx, mut rx) = mpsc::unbounded_channel();

        let _tx_guard = {
            let inner = self.inner.read().await;
            inner
                .insert_in_flight(
                    tx_id,
                    InFlightTransaction {
                        target,
                        expected_response: KadExpectedResponse::PublishNotesRes,
                        expected_target_hash: Some(*file_hash),
                        created_at: Instant::now(),
                        sender: tx,
                    },
                )
                .await?
        };

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
            res = rx.recv() => {
                match res {
                    Some(Ok(KadMessage::PublishNotesRes(pub_res))) => {
                        self.packets_received.fetch_add(1, Ordering::Relaxed);
                        tracing::info!(
                            "Kad received PublishNotesRes from node {} with load {} (tx_id={})",
                            target,
                            pub_res.load,
                            tx_id
                        );
                        Ok(pub_res.load)
                    }
                    Some(Ok(_)) => Err("Unexpected Kad response type".to_string()),
                    Some(Err(err)) => Err(err),
                    None => Err("Transaction channel cancelled".to_string()),
                }
            }
        }
    }
}
