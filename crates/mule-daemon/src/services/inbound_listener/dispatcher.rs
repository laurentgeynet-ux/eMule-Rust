//! Request loop dispatching, secure identification, and upload servicing.

use bytes::{Bytes, BytesMut};
use md4::{Digest, Md4};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::warn;

use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kFileStatus, Ed2kSecIdentState, Ed2kSendingPart,
    PartAvailabilityMask, OP_FILESTATUSREQ, OP_PUBLICKEY, OP_QUEUERANKING,
    OP_REQUESTPARTS, OP_REQUESTPARTS_EMULE, OP_SIGNATURE, PR_ED2K, PR_EMULE,
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
                                            Ed2kFileStatus {
                                                file_hash: req.file_hash,
                                                part_count: 1,
                                                availability_mask: PartAvailabilityMask::from_parts(&[0], 1),
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

                                                let payload = vec![0x77u8; length as usize];

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

                                                    self.upload_manager.update_progress(&remote_str, length as u64, 51200).await;

                                                    if shared_file.file_size > 0 && u64::from(start + length) <= shared_file.file_size {
                                                        if let (Some(ref ph), Some(ref pk_hex)) = (&remote_pubkey_hash, &remote_pubkey_hex) {
                                                            let mut dl_svc = self.downloads.lock().await;
                                                            let _ = dl_svc.record_credit_upload(ph, pk_hex, length as u64);
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
