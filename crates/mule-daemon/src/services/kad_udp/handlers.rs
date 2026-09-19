use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;

use mule_protocol::kad::*;

use super::types::{InFlightMap, KadExpectedResponse};

/// Internal dispatcher for incoming messages.
pub async fn handle_incoming_message(
    socket: &Arc<UdpSocket>,
    peer_addr: SocketAddr,
    msg: KadMessage,
    in_flight: &InFlightMap,
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
                if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip()) && tx.expected_response == KadExpectedResponse::Pong {
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
                if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip()) && tx.expected_response == KadExpectedResponse::HelloRes {
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
                if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip())
                    && (tx.expected_response == KadExpectedResponse::BootstrapRes || tx.expected_response == KadExpectedResponse::FindNodeRes)
                {
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
                let ip_match = tx.target == peer_addr || tx.target.ip() == peer_addr.ip();
                let hash_match = tx.expected_target_hash.map_or(true, |h| h == search_res.target_file_hash);
                let resp_match = tx.expected_response == KadExpectedResponse::SearchSourceRes;
                if ip_match && hash_match && resp_match {
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
            // Opcode 0x4B is the unified publish response for both source and keyword publish.
            // Match in-flight transactions expecting either PublishSourceRes or PublishKeywordRes.
            let mut inf = in_flight.lock().await;
            let mut matched_tx = None;
            for (id, tx) in inf.iter() {
                let ip_match = tx.target == peer_addr || tx.target.ip() == peer_addr.ip();
                let hash_match = tx.expected_target_hash.map_or(true, |h| h == pub_res.target_file_hash);
                let resp_match = tx.expected_response == KadExpectedResponse::PublishSourceRes
                    || tx.expected_response == KadExpectedResponse::PublishKeywordRes;
                if ip_match && hash_match && resp_match {
                    matched_tx = Some(*id);
                    break;
                }
            }
            tracing::info!(
                "Kad UDP: received PublishRes (0x4B) from {peer_addr} with status {} (matched_tx={matched_tx:?})",
                pub_res.status
            );
            if let Some(id) = matched_tx {
                if let Some(tx) = inf.remove(&id) {
                    if tx.expected_response == KadExpectedResponse::PublishKeywordRes {
                        let _ = tx.sender.send(Ok(KadMessage::PublishKeywordRes(KadPublishKeywordRes {
                            keyword_hash: pub_res.target_file_hash,
                            status: pub_res.status,
                        })));
                    } else {
                        let _ = tx.sender.send(Ok(KadMessage::PublishSourceRes(pub_res)));
                    }
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
                let ip_match = tx.target == peer_addr || tx.target.ip() == peer_addr.ip();
                let hash_match = tx.expected_target_hash.map_or(true, |h| h == search_res.keyword_hash);
                let resp_match = tx.expected_response == KadExpectedResponse::SearchKeywordRes;
                if ip_match && hash_match && resp_match {
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
                if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip()) && tx.expected_response == KadExpectedResponse::PublishKeywordRes {
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
