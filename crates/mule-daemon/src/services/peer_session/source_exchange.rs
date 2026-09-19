use mule_protocol::ed2k::{
    Ed2kAnswerSources, Ed2kDecoder, Ed2kEncoder, SxSource, OP_ANSWERSOURCES, OP_ANSWERSOURCES2,
    OP_REQUESTSOURCES, OP_REQUESTSOURCES2, PR_EMULE, PR_EMULE_COMPRESSED,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::services::downloads::DownloadService;
use crate::services::obfuscation_stream::ObfuscatedStream;

pub async fn handle_sx_answer(
    packet: &mule_protocol::framing::RawPacket,
    dl_hash_bytes: [u8; 16],
    file_hash_str: &str,
    target_addr: SocketAddr,
    decoder: &Ed2kDecoder,
    downloads: &Arc<Mutex<DownloadService>>,
) {
    if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED)
        && (packet.opcode == OP_ANSWERSOURCES || packet.opcode == OP_ANSWERSOURCES2)
    {
        match decoder.decode_answer_sources(packet.protocol, packet.opcode, &packet.payload) {
            Ok(ans) => {
                if ans.file_hash == dl_hash_bytes {
                    let mut discovered_sources = Vec::new();
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64;
                    for src in ans.sources {
                        if !src.is_low_id() && src.port > 0 {
                            let ip_u32 = src.ip;
                            let ip_addr = std::net::Ipv4Addr::from(src.ip_bytes);
                            let addr_str = format!("{}:{}", ip_addr, src.port);

                            let hex_user_hash = src.user_hash.map(hex::encode);

                            discovered_sources.push(mule_core::model::PeerSource {
                                file_hash: file_hash_str.to_string(),
                                address: addr_str,
                                ip: ip_u32,
                                port: src.port,
                                origin: mule_core::model::PeerSourceOrigin::SourceExchange,
                                discovered_at_unix_ms: now_ms,
                                last_seen_unix_ms: now_ms,
                                status: mule_core::model::PeerSourceStatus::Candidate,
                                fail_count: 0,
                                last_error: None,
                                cooldown_until_unix_ms: None,
                                user_hash: hex_user_hash,
                                crypt_options: src.crypt_options,
                            });
                        }
                    }
                    if !discovered_sources.is_empty() {
                        let mut dl_svc = downloads.lock().await;
                        let inserted = dl_svc.upsert_peer_sources(file_hash_str, &discovered_sources).unwrap_or(0);
                        let total_swarm = dl_svc.find_by_ed2k_hash(file_hash_str).map(|d| d.source_count).unwrap_or(0);
                        tracing::info!(
                            "[SOURCE_EXCHANGE] Received {} sources via SX from peer {} for file {} -> {} new unique sources persisted (total swarm: {} sources)",
                            discovered_sources.len(),
                            target_addr,
                            file_hash_str,
                            inserted,
                            total_swarm
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Peer {target_addr}: failed to decode SX answer: {e}");
            }
        }
    }
}

pub async fn handle_sx_request(
    packet: &mule_protocol::framing::RawPacket,
    dl_hash_bytes: Option<[u8; 16]>,
    target_addr: SocketAddr,
    decoder: &Ed2kDecoder,
    encoder: &Ed2kEncoder,
    downloads: &Arc<Mutex<DownloadService>>,
    stream: &mut ObfuscatedStream<TcpStream>,
) {
    if packet.protocol == PR_EMULE
        && (packet.opcode == OP_REQUESTSOURCES || packet.opcode == OP_REQUESTSOURCES2)
    {
        match decoder.decode_request_sources(packet.protocol, packet.opcode, &packet.payload) {
            Ok(req) => {
                if dl_hash_bytes.is_none() || dl_hash_bytes == Some(req.file_hash) {
                    let file_hash_str = hex::encode(req.file_hash);
                    let known_sources = {
                        let dl_svc = downloads.lock().await;
                        dl_svc.list_peer_sources(&file_hash_str).unwrap_or_default()
                    };
                    let mut resp_sources = Vec::new();
                    for src in known_sources {
                        if resp_sources.len() >= 50 {
                            break;
                        }
                        if src.port > 0 && src.ip >= 16_777_216 {
                            let ip_bytes = src.ip.to_le_bytes();
                            let user_hash_opt = if let Some(ref h) = src.user_hash {
                                if let Ok(hb) = hex::decode(h) {
                                    if hb.len() == 16 {
                                        let mut h16 = [0u8; 16];
                                        h16.copy_from_slice(&hb);
                                        Some(h16)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            resp_sources.push(SxSource::new(
                                ip_bytes,
                                src.port,
                                user_hash_opt,
                                src.crypt_options,
                            ));
                        }
                    }

                    let ans = Ed2kAnswerSources::new(req.file_hash, resp_sources);
                    if let Ok(ans_pkt) = encoder.encode_answer_sources(&ans, true) {
                        let mut resp_wire = Vec::with_capacity(6 + ans_pkt.payload.len());
                        resp_wire.push(ans_pkt.protocol);
                        resp_wire.extend_from_slice(&(ans_pkt.frame_len() as u32).to_le_bytes());
                        resp_wire.push(ans_pkt.opcode);
                        resp_wire.extend_from_slice(&ans_pkt.payload);
                        use tokio::io::AsyncWriteExt;
                        if let Err(e) = stream.write_all(&resp_wire).await {
                            tracing::warn!("Failed to send SX answer to {target_addr}: {e}");
                        } else {
                            tracing::info!(
                                "[SOURCE_EXCHANGE] Sent SX response with {} sources to peer {} for file {}",
                                ans.sources.len(),
                                target_addr,
                                file_hash_str
                            );
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Peer {target_addr}: failed to decode SX request: {e}");
            }
        }
    }
}
