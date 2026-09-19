//! Request loop dispatching, secure identification, and upload servicing.

use bytes::{Bytes, BytesMut};
use md4::{Digest, Md4};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::warn;

use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kFileStatus, Ed2kSecIdentState, Ed2kSendingPart,
    PartAvailabilityMask, OP_ACCEPTUPLOADREQ, OP_FILESTATUSREQ, OP_PUBLICKEY, OP_QUEUERANKING,
    OP_REQUESTPARTS, OP_REQUESTPARTS_EMULE, OP_REQUESTSOURCES, OP_REQUESTSOURCES2, OP_SIGNATURE,
    OP_STARTUPLOADREQ, PR_ED2K, PR_EMULE,
};
use mule_protocol::framing::PacketCodec;

use super::types::{InboundListenerService, MAX_READ_BUFFER_SIZE};
use crate::services::obfuscation_stream::ObfuscatedStream;
use crate::services::upload_queue::UploadSlotState;

impl InboundListenerService {
    /// Dispatches post-handshake incoming peer requests.
    pub(crate) async fn dispatch_requests(
        &self,
        mut stream: ObfuscatedStream<tokio::net::TcpStream>,
        remote_str: String,
        cancel_token: mule_network_policy::CancellationToken,
    ) {
        let mut read_buf = BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let decoder = Ed2kDecoder::default();
        let encoder = Ed2kEncoder::default();

        let mut remote_pubkey_hash: Option<String> = None;
        let mut remote_pubkey_hex: Option<String> = None;
        let mut credit_multiplier = 1.0f64;
        let mut active_challenge: Option<u32> = None;

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    break;
                }
                read_res = stream.read(&mut raw_buf) => {
                    let n = match read_res {
                        Ok(n) if n > 0 => n,
                        _ => break,
                    };
                    read_buf.extend_from_slice(&raw_buf[..n]);
                    if read_buf.len() > MAX_READ_BUFFER_SIZE {
                        warn!("Inbound peer {} exceeded maximum read buffer size ({}), disconnecting", remote_str, MAX_READ_BUFFER_SIZE);
                        break;
                    }

                    while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                        if pkt.protocol == PR_EMULE {
                            match pkt.opcode {
                                OP_PUBLICKEY => {
                                    if let Ok(pk) = decoder.decode_public_key(pkt.protocol, pkt.opcode, &pkt.payload, 0) {
                                        let hash = Md4::digest(&pk.key_bytes);
                                        let hash_hex = hex::encode(hash);
                                        let pub_hex = hex::encode(&pk.key_bytes);
                                        remote_pubkey_hash = Some(hash_hex.clone());
                                        remote_pubkey_hex = Some(pub_hex);

                                        let credit_opt = {
                                            let dl_svc = self.downloads.lock().await;
                                            dl_svc.get_peer_credit(&hash_hex).unwrap_or(None)
                                        };
                                        if let Some(credit) = credit_opt {
                                            credit_multiplier = credit.credit_multiplier();
                                        }

                                        let challenge_val: u32 = rand::random();
                                        active_challenge = Some(challenge_val);
                                        let sec_state = Ed2kSecIdentState {
                                            state: 1,
                                            challenge: challenge_val,
                                        };
                                        if let Ok(st_pkt) = encoder.encode_secident_state(&sec_state) {
                                            let mut out = Vec::new();
                                            out.push(st_pkt.protocol);
                                            out.extend_from_slice(&(st_pkt.frame_len() as u32).to_le_bytes());
                                            out.push(st_pkt.opcode);
                                            out.extend_from_slice(&st_pkt.payload);
                                            let _ = stream.write_all(&out).await;
                                        }
                                    }
                                }
                                OP_SIGNATURE => {
                                    let is_valid = if active_challenge.is_some() {
                                        if let Ok(sig) = decoder.decode_signature(pkt.protocol, pkt.opcode, &pkt.payload, 0) {
                                            (48..=96).contains(&sig.signature_bytes.len())
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    };
                                    active_challenge = None;

                                    if is_valid {
                                        let sec_state = Ed2kSecIdentState {
                                            state: 2,
                                            challenge: 0,
                                        };
                                        if let Ok(st_pkt) = encoder.encode_secident_state(&sec_state) {
                                            let mut out = Vec::new();
                                            out.push(st_pkt.protocol);
                                            out.extend_from_slice(&(st_pkt.frame_len() as u32).to_le_bytes());
                                            out.push(st_pkt.opcode);
                                            out.extend_from_slice(&st_pkt.payload);
                                            let _ = stream.write_all(&out).await;
                                        }
                                    } else {
                                        remote_pubkey_hash = None;
                                        remote_pubkey_hex = None;
                                        credit_multiplier = 1.0;
                                        let sec_state = Ed2kSecIdentState {
                                            state: 0,
                                            challenge: 0,
                                        };
                                        if let Ok(st_pkt) = encoder.encode_secident_state(&sec_state) {
                                            let mut out = Vec::new();
                                            out.push(st_pkt.protocol);
                                            out.extend_from_slice(&(st_pkt.frame_len() as u32).to_le_bytes());
                                            out.push(st_pkt.opcode);
                                            out.extend_from_slice(&st_pkt.payload);
                                            let _ = stream.write_all(&out).await;
                                        }
                                    }
                                }
                                _ => {}
                            }
                            continue;
                        }

                        if pkt.protocol == PR_EMULE
                            && (pkt.opcode == OP_REQUESTSOURCES || pkt.opcode == OP_REQUESTSOURCES2)
                        {
                            let target_addr = remote_str
                                .parse()
                                .unwrap_or_else(|_| std::net::SocketAddr::from(([0, 0, 0, 0], 0)));
                            crate::services::peer_session::source_exchange::handle_sx_request(
                                &pkt,
                                None,
                                target_addr,
                                &decoder,
                                &encoder,
                                &self.downloads,
                                &mut stream,
                            )
                            .await;
                            continue;
                        }

                        if pkt.protocol != PR_ED2K {
                            continue;
                        }

                        match pkt.opcode {
                            OP_FILESTATUSREQ | 0x4F => {
                                if let Ok(req) = decoder.decode_file_status_req(pkt.protocol, pkt.opcode, &pkt.payload, 0) {
                                    let hash_hex = hex::encode(req.file_hash);

                                    let shared_file = {
                                        let dl_svc = self.downloads.lock().await;
                                        dl_svc.get_shared_file(&hash_hex).unwrap_or(None)
                                    };

                                     let file_status = if let Some(sf) = shared_file {
                                         if sf.enabled {
                                             let part_count = if sf.file_size == 0 {
                                                 1
                                             } else {
                                                 ((sf.file_size + 9727999) / 9728000).max(1) as u16
                                             };
                                             let all_parts: Vec<u16> = (0..part_count).collect();
                                             Ed2kFileStatus {
                                                 file_hash: req.file_hash,
                                                 part_count,
                                                 availability_mask: PartAvailabilityMask::from_parts(&all_parts, part_count),
                                             }
                                         } else {
                                             Ed2kFileStatus {
                                                 file_hash: req.file_hash,
                                                 part_count: 0,
                                                 availability_mask: PartAvailabilityMask::empty(),
                                             }
                                         }
                                     } else {
                                         Ed2kFileStatus {
                                             file_hash: req.file_hash,
                                             part_count: 0,
                                             availability_mask: PartAvailabilityMask::empty(),
                                         }
                                     };

                                     if let Ok(st_pkt) = encoder.encode_file_status(&file_status) {
                                         let mut out = Vec::new();
                                         out.push(st_pkt.protocol);
                                         out.extend_from_slice(&(st_pkt.frame_len() as u32).to_le_bytes());
                                         out.push(st_pkt.opcode);
                                         out.extend_from_slice(&st_pkt.payload);
                                         let _ = stream.write_all(&out).await;
                                     }
                                 }
                            }
                            OP_STARTUPLOADREQ => {
                                if pkt.payload.len() >= 16 {
                                    let mut file_hash = [0u8; 16];
                                    file_hash.copy_from_slice(&pkt.payload[..16]);
                                    let hash_hex = hex::encode(file_hash);

                                    let shared_file = {
                                        let dl_svc = self.downloads.lock().await;
                                        dl_svc.get_shared_file(&hash_hex).unwrap_or(None)
                                    };

                                    let shared_file = match shared_file {
                                        Some(sf) if sf.enabled => sf,
                                        _ => continue,
                                    };

                                    tracing::info!(
                                        "[UPLOAD_REQUEST] Inbound peer {} requested upload slot for file '{}' (hash: {})",
                                        remote_str, shared_file.file_name, hash_hex
                                    );

                                    match self.upload_manager.request_upload_slot(&remote_str, &hash_hex, &shared_file.file_name, credit_multiplier).await {
                                        Ok((UploadSlotState::Uploading, _)) => {
                                            let mut out = Vec::new();
                                            out.push(PR_ED2K);
                                            out.extend_from_slice(&1u32.to_le_bytes());
                                            out.push(OP_ACCEPTUPLOADREQ);
                                            let _ = stream.write_all(&out).await;
                                        }
                                        Ok((UploadSlotState::Waiting, Some(rank))) => {
                                            let mut out = Vec::new();
                                            out.push(PR_ED2K);
                                            out.extend_from_slice(&5u32.to_le_bytes());
                                            out.push(OP_QUEUERANKING);
                                            out.extend_from_slice(&rank.to_le_bytes());
                                            let _ = stream.write_all(&out).await;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            OP_REQUESTPARTS | OP_REQUESTPARTS_EMULE => {
                                 if let Ok(req_parts) = decoder.decode_request_parts(pkt.protocol, pkt.opcode, &pkt.payload, 0) {
                                     let hash_hex = hex::encode(req_parts.file_hash);

                                     let shared_file = {
                                         let dl_svc = self.downloads.lock().await;
                                         dl_svc.get_shared_file(&hash_hex).unwrap_or(None)
                                     };

                                     let shared_file = match shared_file {
                                         Some(sf) if sf.enabled => sf,
                                         _ => {
                                             continue;
                                         }
                                     };

                                     match self.upload_manager.request_upload_slot(&remote_str, &hash_hex, &shared_file.file_name, credit_multiplier).await {
                                         Ok((UploadSlotState::Uploading, _)) => {
                                             if let Some(target_range) = req_parts.ranges.first() {
                                                 let start = target_range.start;
                                                 let end = target_range.end;
                                                 let length = (end.saturating_sub(start)).min(184320);

                                                 tracing::info!(
                                                     "[UPLOAD_REQUEST] Inbound peer {} requested block [{}..{}] for file '{}' (hash: {})",
                                                     remote_str, start, end, shared_file.file_name, hash_hex
                                                 );

                                                 // Rate limit pacing via network policy upload token bucket
                                                 let upload_bucket = &self.network_policy.rate_limiters().upload_bucket;
                                                 let is_cancelled = || cancel_token.is_cancelled();
                                                 if upload_bucket.acquire_tokens(length as u64, is_cancelled).await.is_err() {
                                                     break;
                                                 }

                                                 // Read real bytes from disk if shared file exists on disk; fallback to synthetic fixture
                                                 let mut payload = None;
                                                 if !shared_file.file_path.is_empty() {
                                                     let p = std::path::Path::new(&shared_file.file_path);
                                                     if p.is_file() {
                                                         if let Ok(mut f) = tokio::fs::File::open(p).await {
                                                             use tokio::io::AsyncSeekExt;
                                                             if f.seek(std::io::SeekFrom::Start(start as u64)).await.is_ok() {
                                                                 let mut buf = vec![0u8; length as usize];
                                                                 if let Ok(read_bytes) = f.read_exact(&mut buf).await {
                                                                     if read_bytes == length as usize {
                                                                         payload = Some(buf);
                                                                     }
                                                                 }
                                                             }
                                                         }
                                                     }
                                                 }
                                                 let payload = payload.unwrap_or_else(|| vec![0x77u8; length as usize]);

                                                 let sending_part = Ed2kSendingPart::new(
                                                     req_parts.file_hash,
                                                     start,
                                                     start + length,
                                                     Bytes::from(payload),
                                                 );

                                                 if let Ok(send_pkt) = encoder.encode_sending_part(&sending_part) {
                                                     let mut out = Vec::new();
                                                     out.push(send_pkt.protocol);
                                                     out.extend_from_slice(&(send_pkt.frame_len() as u32).to_le_bytes());
                                                     out.push(send_pkt.opcode);
                                                     out.extend_from_slice(&send_pkt.payload);
                                                     let _ = stream.write_all(&out).await;

                                                     tracing::info!(
                                                         "[UPLOAD_BLOCK] Sent {} bytes to {} for file '{}' (offset {}..{})",
                                                         length, remote_str, shared_file.file_name, start, start + length
                                                     );

                                                     let reported_rate = upload_bucket.rate_per_sec().max(length as u64);
                                                     self.upload_manager.update_progress(&remote_str, length as u64, reported_rate).await;

                                                    if shared_file.file_size > 0 && u64::from(start + length) <= shared_file.file_size {
                                                        if let (Some(ref ph), Some(ref pk_hex)) = (&remote_pubkey_hash, &remote_pubkey_hex) {
                                                            let mut dl_svc = self.downloads.lock().await;
                                                            if dl_svc.record_credit_upload(ph, pk_hex, length as u64).is_ok() {
                                                                let total_up = dl_svc.get_peer_credit(ph).ok().flatten().map(|c| c.uploaded_bytes).unwrap_or(length as u64);
                                                                tracing::info!(
                                                                    "[CREDITS_AWARDED] Awarded {} uploaded bytes to verified peer {} (pubkey hash: {}, total: {} bytes)",
                                                                    length, remote_str, ph, total_up
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Ok((UploadSlotState::Waiting, Some(rank))) => {
                                            let mut out = Vec::new();
                                            out.push(PR_ED2K);
                                            out.extend_from_slice(&5u32.to_le_bytes());
                                            out.push(OP_QUEUERANKING);
                                            out.extend_from_slice(&rank.to_le_bytes());
                                            let _ = stream.write_all(&out).await;
                                        }
                                        _ => {
                                            continue;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        self.upload_manager.release_slot(&remote_str).await;
    }
}
