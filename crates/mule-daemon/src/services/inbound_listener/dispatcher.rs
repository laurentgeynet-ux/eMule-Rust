//! Request loop dispatching, secure identification, and upload servicing.

use std::net::SocketAddr;

use bytes::{Bytes, BytesMut};
use md4::{Digest, Md4};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::warn;

use mule_crypto::legacy_sui::{
    build_sui_signing_payload, parse_peer_public_key, verify_sui_signature, SecureIdentVersion,
};

use mule_protocol::ed2k::{
    Ed2kAichAnswer, Ed2kAichFileHashRes, Ed2kAichRecovery, Ed2kDecoder, Ed2kEncoder,
    Ed2kFileStatus, Ed2kHashSetAnswer, Ed2kPublicKey, Ed2kSecIdentState, Ed2kSendingPart,
    Ed2kSignature, PartAvailabilityMask, OP_ACCEPTUPLOADREQ, OP_AICHFILEHASHREQ, OP_AICH_REQUEST,
    OP_CANCELTRANSFER, OP_COMPRESSEDPART, OP_COMPRESSEDPART_I64, OP_EMULE_INFO,
    OP_EMULE_INFO_ANSWER, OP_FILESTATUSREQ, OP_FWCHECKUDPREQ, OP_HASHSETREQUEST, OP_OUTOFPARTS,
    OP_PUBLICKEY, OP_QUEUERANKING, OP_REQUESTPARTS, OP_REQUESTPARTS_EMULE, OP_REQUESTPARTS_I64,
    OP_REQUESTSOURCES, OP_REQUESTSOURCES2, OP_SECIDENTSTATE, OP_SIGNATURE, OP_STARTUPLOADREQ,
    PR_ED2K, PR_EMULE,
};
use mule_protocol::framing::PacketCodec;

use super::types::{InboundListenerService, MAX_READ_BUFFER_SIZE};
use crate::services::obfuscation_stream::ObfuscatedStream;
use crate::services::upload_queue::UploadSlotState;

/// Converts an IPv4 address to the `u32` used in the SUI V2 signed payload
/// (little-endian on the wire). IPv6 peers carry `0`, matching eMule which is
/// IPv4-only for this field.
fn ip_to_u32(ip: Option<std::net::IpAddr>) -> u32 {
    match ip {
        Some(std::net::IpAddr::V4(v4)) => u32::from_be_bytes(v4.octets()),
        _ => 0,
    }
}

impl InboundListenerService {
    /// Dispatches post-handshake incoming peer requests.
    pub(crate) async fn dispatch_requests(
        &self,
        mut stream: ObfuscatedStream<tokio::net::TcpStream>,
        remote_str: String,
        local_ip: std::net::IpAddr,
        cancel_token: mule_network_policy::CancellationToken,
        peer_supports_compression: bool,
        peer_user_hash: Option<[u8; 16]>,
    ) {
        let mut read_buf = BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let decoder = Ed2kDecoder::default();
        let encoder = Ed2kEncoder::default();

        let mut remote_pubkey_hash: Option<String> = None;
        let mut remote_pubkey_der: Option<Vec<u8>> = None;
        let mut credit_multiplier = 1.0f64;
        // eMule IS_IDBADGUY: a peer whose SUI signature was explicitly
        // rejected gets score 0 in the upload queue (never promoted).
        let mut sui_rejected = false;
        let mut active_challenge: Option<u32> = None;
        let mut pending_peer_challenge: Option<(u8, u32)> = None;
        let mut answered_peer_challenge = false;
        let peer_ip = remote_str.parse::<SocketAddr>().ok().map(|a| a.ip());
        // Upload queue identity: userhash when the peer announced one in
        // OP_HELLO (eMule dedups queued clients by identity, not by IP —
        // a reconnected client keeps its rank, NAT-shared IPs stay distinct),
        // falling back to the socket address.
        let client_key = peer_user_hash
            .map(|h| format!("uh:{}", hex::encode(h)))
            .unwrap_or_else(|| remote_str.clone());

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
                                        if pk.key_bytes.len() > mule_crypto::legacy_sui::MAX_PUBKEY_WIRE {
                                            self.sui_stats.malformed_packets.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                            warn!(
                                                "[SUI] Inbound peer {} sent oversized public key ({} bytes), ignoring",
                                                remote_str, pk.key_bytes.len()
                                            );
                                            continue;
                                        }
                                        let hash = Md4::digest(&pk.key_bytes);
                                        let hash_hex = hex::encode(hash);
                                        remote_pubkey_hash = Some(hash_hex.clone());
                                        remote_pubkey_der = Some(pk.key_bytes.to_vec());
                                        // No credit multiplier is granted here: it requires a real
                                        // RSASSA-PKCS1v15-SHA1 signature verification (OP_SIGNATURE).
                                        tracing::info!(
                                            "[SUI] Inbound peer {} sent public key ({} bytes, md4={}...)",
                                            remote_str, pk.key_bytes.len(), &hash_hex[..16]
                                        );

                                        // Only issue a challenge if we hold a usable local identity:
                                        // the peer signs *our* public key, so no key means no SUI.
                                        let Some(my_pub) = &self.sui_pubkey_der else {
                                            warn!(
                                                "[SUI] No local identity key, not issuing challenge to {}",
                                                remote_str
                                            );
                                            continue;
                                        };
                                        let _ = my_pub;
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
                                            self.sui_stats.challenges_issued_in.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                            tracing::debug!(
                                                "[SUI] Challenge issued to {} (state=1)",
                                                remote_str
                                            );
                                        }
                                        if let (Some((state, ch)), Some(pk)) =
                                            (pending_peer_challenge.take(), remote_pubkey_der.clone())
                                        {
                                            self.answer_peer_challenge(
                                                state, ch, &pk, &remote_str, &encoder, &mut stream,
                                            ).await;
                                            answered_peer_challenge = true;
                                        }
                                    }
                                }
                                OP_SECIDENTSTATE => {
                                    // The inbound peer challenges us (signer
                                    // role): answer with our OP_PUBLICKEY when
                                    // asked (state=2) plus OP_SIGNATURE over
                                    // `peer_pubkey || challenge` (V1).
                                    match decoder.decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0) {
                                        Ok(st) if st.state != 0 && !answered_peer_challenge => {
                                            match remote_pubkey_der.clone() {
                                                Some(pk) => {
                                                    self.answer_peer_challenge(
                                                        st.state, st.challenge, &pk,
                                                        &remote_str, &encoder, &mut stream,
                                                    ).await;
                                                    answered_peer_challenge = true;
                                                }
                                                None => {
                                                    pending_peer_challenge = Some((st.state, st.challenge));
                                                    tracing::debug!(
                                                        "[SUI] Inbound peer {} sent challenge (state={}) before its public key; deferred",
                                                        remote_str, st.state
                                                    );
                                                }
                                            }
                                        }
                                        Ok(_) => {}
                                        Err(e) => {
                                            self.sui_stats.malformed_packets.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                            warn!("[SUI] Malformed OP_SECIDENTSTATE from {}: {}", remote_str, e);
                                        }
                                    }
                                }
                                OP_SIGNATURE => {
                                    // Real eMule SUI verification: RSA RSASSA-PKCS1v15-SHA1 over
                                    // `challenger_pubkey || challenge [|| ip || kind]`.
                                    // See docs/ed2k/SUI_WIRE_FORMAT.md.
                                    let challenge = active_challenge.take();

                                    // Anti-flood (eMule-conformant): one signature per remote
                                    // IP. `m_dwLastSignatureIP` is only assigned once the
                                    // packet reaches VerifyIdent — early exits (no pubkey, no
                                    // challenge) do NOT consume the slot.
                                    let flooded = match peer_ip {
                                        Some(ip) => *self.last_sig_ip.lock().await == Some(ip),
                                        None => false,
                                    };

                                    let mut kind_invalid = false;
                                    let mut verified_ok = false;
                                    let reject_reason: Option<&'static str> = if flooded {
                                        Some("signature flood (duplicate source IP)")
                                    } else {
                                        match (challenge, self.sui_pubkey_der.clone(), remote_pubkey_der.clone()) {
                                            (Some(ch), Some(my_pub), Some(peer_der)) => {
                                                match decoder.decode_signature(pkt.protocol, pkt.opcode, &pkt.payload, 0) {
                                                    Ok(sig) => {
                                                        // V2 wire: [u8 siglen][sig][u8 kind].
                                                        let sig_len = pkt.payload.first().copied().unwrap_or(0) as usize;
                                                        let kind = if pkt.payload.len() == sig_len + 2 {
                                                            Some(pkt.payload[sig_len + 1])
                                                        } else {
                                                            None
                                                        };
                                                        let ip_u32 = match kind {
                                                            Some(20) => ip_to_u32(peer_ip),  // LOCALCLIENT: signer's IP
                                                            Some(10) => ip_to_u32(Some(local_ip)), // REMOTECLIENT: verifier's IP
                                                            Some(30) => 0,                   // NONECLIENT
                                                            Some(_) => { kind_invalid = true; 0 }
                                                            None => 0,                        // V1
                                                        };
                                                        if kind_invalid {
                                                            Some("invalid IP-binding kind")
                                                        } else {
                                                            // Verification is about to run:
                                                            // consume this IP's signature slot.
                                                            if let Some(ip) = peer_ip {
                                                                *self.last_sig_ip.lock().await = Some(ip);
                                                            }
                                                            let version = if kind.is_some() {
                                                                SecureIdentVersion::V2
                                                            } else {
                                                                SecureIdentVersion::V1
                                                            };
                                                            let payload = build_sui_signing_payload(
                                                                version, &my_pub, ch, Some(ip_u32), kind,
                                                            );
                                                            let peer_key = parse_peer_public_key(&peer_der);
                                                            match (payload, peer_key) {
                                                                (Ok(pl), Ok(pk)) => {
                                                                    match verify_sui_signature(&pk, &pl, &sig.signature_bytes) {
                                                                        Ok(()) => {
                                                                            verified_ok = true;
                                                                            None
                                                                        }
                                                                        Err(_) => Some("RSA verification failed"),
                                                                    }
                                                                }
                                                                (Err(_), _) => Some("invalid signing payload"),
                                                                (_, Err(_)) => Some("invalid peer public key"),
                                                            }
                                                        }
                                                    }
                                                    Err(_) => Some("malformed signature packet"),
                                                }
                                            }
                                            (None, _, _) => Some("no active challenge"),
                                            (_, None, _) => Some("no local identity key"),
                                            (_, _, None) => Some("no peer public key"),
                                        }
                                    };

                                    use std::sync::atomic::Ordering::Relaxed;
                                    if flooded {
                                        self.sui_stats.signatures_rejected_flood.fetch_add(1, Relaxed);
                                    }
                                    if verified_ok {
                                        self.sui_stats.signatures_verified.fetch_add(1, Relaxed);
                                        // Verified: pin pubkey to credit record (by pubkey hash)
                                        // and grant the credit multiplier now.
                                        if let Some(ref ph) = remote_pubkey_hash {
                                            let mut dl_svc = self.downloads.lock().await;
                                            let pub_hex = remote_pubkey_der
                                                .as_ref()
                                                .map(hex::encode)
                                                .unwrap_or_default();
                                            if let Ok(Some(credit)) = dl_svc.get_peer_credit(ph) {
                                                credit_multiplier = credit.credit_multiplier();
                                            }
                                            let _ = dl_svc.record_credit_upload(ph, &pub_hex, 0);
                                            tracing::info!(
                                                "[SUI] Inbound peer {} signature VERIFIED (md4={}..., credit_multiplier={:.2})",
                                                remote_str, &ph[..16.min(ph.len())], credit_multiplier
                                            );
                                        }
                                    } else {
                                        self.sui_stats.signatures_rejected.fetch_add(1, Relaxed);
                                        if reject_reason == Some("malformed signature packet") {
                                            self.sui_stats.malformed_packets.fetch_add(1, Relaxed);
                                        }
                                        tracing::warn!(
                                            "[SUI] Inbound peer {} signature REJECTED: {}",
                                            remote_str,
                                            reject_reason.unwrap_or("unknown")
                                        );
                                        remote_pubkey_hash = None;
                                        remote_pubkey_der = None;
                                        credit_multiplier = 1.0;
                                        sui_rejected = true;
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
                                OP_AICHFILEHASHREQ => {
                                    if let Ok(req) = decoder.decode_aich_file_hash_req(
                                        pkt.protocol, pkt.opcode, &pkt.payload, 0,
                                    ) {
                                        let hash_hex = hex::encode(req.file_hash);
                                        let shared_file = {
                                            let dl_svc = self.downloads.lock().await;
                                            dl_svc.get_shared_file(&hash_hex).unwrap_or(None)
                                        };
                                        let root = match shared_file {
                                            Some(sf) if sf.enabled && sf.file_size > 0 => {
                                                self.shared_aich_root(&sf, &hash_hex).await
                                            }
                                            _ => None,
                                        };
                                        // eMule only answers when the AICH hashset is
                                        // available; otherwise the requester times out.
                                        if let Some(root) = root {
                                            let res = Ed2kAichFileHashRes {
                                                file_hash: req.file_hash,
                                                aich_hash: root,
                                            };
                                            if let Ok(res_pkt) = encoder.encode_aich_file_hash_res(&res) {
                                                let mut out = Vec::new();
                                                out.push(res_pkt.protocol);
                                                out.extend_from_slice(&(res_pkt.frame_len() as u32).to_le_bytes());
                                                out.push(res_pkt.opcode);
                                                out.extend_from_slice(&res_pkt.payload);
                                                let _ = stream.write_all(&out).await;
                                                tracing::info!(
                                                    "[AICH] Served master hash to {} for {}",
                                                    remote_str, hash_hex
                                                );
                                            }
                                        }
                                    }
                                }
                                OP_AICH_REQUEST => {
                                    if let Ok(req) = decoder.decode_aich_request(
                                        pkt.protocol, pkt.opcode, &pkt.payload, 0,
                                    ) {
                                        let hash_hex = hex::encode(req.file_hash);
                                        let shared_file = {
                                            let dl_svc = self.downloads.lock().await;
                                            dl_svc.get_shared_file(&hash_hex).unwrap_or(None)
                                        };

                                        let recovery = match shared_file {
                                            Some(sf) if sf.enabled && sf.file_size > 0 => {
                                                match self.shared_aich_tree(&sf, &hash_hex).await {
                                                    Some((tree, root)) if root == req.master_hash => {
                                                        mule_format::create_part_recovery_data(
                                                            &tree, sf.file_size, req.part as u32,
                                                        )
                                                        .ok()
                                                    }
                                                    _ => None,
                                                }
                                            }
                                            _ => None,
                                        };

                                        let answer = Ed2kAichAnswer {
                                            file_hash: req.file_hash,
                                            recovery: recovery.map(|entries| {
                                                // eMule uses 16-bit identifiers unless the
                                                // file is large enough to overflow them.
                                                let mut entries16 = Vec::new();
                                                let mut entries32 = Vec::new();
                                                for e in &entries {
                                                    if e.ident <= u16::MAX as u32 {
                                                        entries16.push((e.ident as u16, e.hash.0));
                                                    } else {
                                                        entries32.push((e.ident, e.hash.0));
                                                    }
                                                }
                                                Ed2kAichRecovery {
                                                    part: req.part,
                                                    master_hash: req.master_hash,
                                                    entries16,
                                                    entries32,
                                                }
                                            }),
                                        };
                                        if let Ok(ans_pkt) = encoder.encode_aich_answer(&answer) {
                                            let mut out = Vec::new();
                                            out.push(ans_pkt.protocol);
                                            out.extend_from_slice(&(ans_pkt.frame_len() as u32).to_le_bytes());
                                            out.push(ans_pkt.opcode);
                                            out.extend_from_slice(&ans_pkt.payload);
                                            let _ = stream.write_all(&out).await;
                                            if answer.recovery.is_some() {
                                                tracing::info!(
                                                    "[AICH] Served recovery data for part {} of {} to {}",
                                                    req.part, hash_hex, remote_str
                                                );
                                            } else {
                                                tracing::debug!(
                                                    "[AICH] Could not serve recovery for part {} of {} to {} (failed answer sent)",
                                                    req.part, hash_hex, remote_str
                                                );
                                            }
                                        }
                                    }
                                }
                                OP_FWCHECKUDPREQ => {
                                    // eMule ProcessFirewallCheckUDPRequest:
                                    // read u16 intern + u16 extern + u32 key,
                                    // then send KADEMLIA2_FIREWALLUDP to the
                                    // requester's IP on its intern port (and
                                    // extern port when it differs). The UDP
                                    // verify key is only used for encrypted
                                    // Kad UDP (not implemented) — accepted and
                                    // ignored.
                                    if pkt.payload.len() >= 8 {
                                        let intern = u16::from_le_bytes([pkt.payload[0], pkt.payload[1]]);
                                        let extern_p = u16::from_le_bytes([pkt.payload[2], pkt.payload[3]]);
                                        if intern != 0 {
                                            let ku = {
                                                self.kad_udp.read().unwrap().as_ref().cloned()
                                            };
                                            if let (Some(ip), Some(ku)) = (peer_ip, ku) {
                                                // eMule flags results as biased when
                                                // the requester is already a known
                                                // contact/peer; not tracked here.
                                                let error_code: u8 = 0;
                                                let t1 = SocketAddr::new(ip, intern);
                                                match ku.send_firewall_udp(t1, error_code, intern).await {
                                                    Ok(()) => tracing::debug!(
                                                        "Kad FW check: sent FIREWALLUDP to {} for peer {}",
                                                        t1, remote_str
                                                    ),
                                                    Err(e) => tracing::debug!(
                                                        "Kad FW check: failed FIREWALLUDP to {}: {}", t1, e
                                                    ),
                                                }
                                                if extern_p != 0 && extern_p != intern {
                                                    let t2 = SocketAddr::new(ip, extern_p);
                                                    let _ = ku
                                                        .send_firewall_udp(t2, error_code, extern_p)
                                                        .await;
                                                }
                                            }
                                        }
                                    }
                                }
                                OP_EMULE_INFO | OP_EMULE_INFO_ANSWER => {
                                    // eMule ProcessMuleInfoPacket: decode the
                                    // extended capability announcement and
                                    // reply OP_EMULEINFOANSWER to OP_EMULEINFO.
                                    match decoder.decode_emule_info(
                                        pkt.protocol,
                                        pkt.opcode,
                                        &pkt.payload,
                                        0,
                                    ) {
                                        Ok(info) => {
                                            let tag_desc: Vec<String> = info
                                                .tags
                                                .iter()
                                                .map(|t| t.name.display_name())
                                                .collect();
                                            tracing::info!(
                                                "Inbound peer {} sent {} (client_ver=0x{:02X}, proto_ver=0x{:02X}, tags=[{}], unknown_tags={})",
                                                remote_str,
                                                if pkt.opcode == OP_EMULE_INFO {
                                                    "OP_EMULEINFO"
                                                } else {
                                                    "OP_EMULEINFO_ANSWER"
                                                },
                                                info.client_version,
                                                info.protocol_version,
                                                tag_desc.join(","),
                                                info.unknown_tags.len()
                                            );
                                            if pkt.opcode == OP_EMULE_INFO {
                                                let answer =
                                                    crate::services::peer_session::handshake::build_emule_info(
                                                        self.kad_udp_port,
                                                        if self.sui_pubkey_der.is_some() { 3 } else { 0 },
                                                    );
                                                if let Ok(ans_pkt) =
                                                    encoder.encode_emule_info_answer(&answer)
                                                {
                                                    let mut out = Vec::with_capacity(
                                                        6 + ans_pkt.payload.len(),
                                                    );
                                                    out.push(ans_pkt.protocol);
                                                    out.extend_from_slice(
                                                        &(ans_pkt.frame_len() as u32).to_le_bytes(),
                                                    );
                                                    out.push(ans_pkt.opcode);
                                                    out.extend_from_slice(&ans_pkt.payload);
                                                    let _ = stream.write_all(&out).await;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::debug!(
                                                "Inbound peer {} sent malformed OP_EMULEINFO, ignored: {e}",
                                                remote_str
                                            );
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
                                                 sf.file_size.div_ceil(9728000).max(1) as u16
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
                            OP_HASHSETREQUEST => {
                                if let Ok(req) = decoder.decode_hashset_request(pkt.protocol, pkt.opcode, &pkt.payload, 0) {
                                    let hash_hex = hex::encode(req.file_hash);

                                    // Serve the per-part hashset if known (learned during
                                    // download or already computed), else compute it from
                                    // the shared file on disk (cached in DB afterwards).
                                    let cached = {
                                        let dl_svc = self.downloads.lock().await;
                                        dl_svc.get_part_hashes_by_hash(&hash_hex)
                                    };

                                    let part_hashes = if !cached.is_empty() {
                                        Some(cached)
                                    } else {
                                        let shared_file = {
                                            let dl_svc = self.downloads.lock().await;
                                            dl_svc.get_shared_file(&hash_hex).unwrap_or(None)
                                        };
                                        match shared_file {
                                            Some(sf) if sf.enabled && !sf.file_path.is_empty() => {
                                                let path = sf.file_path.clone();
                                                let res = tokio::task::spawn_blocking(move || {
                                                    let mut f = std::fs::File::open(&path)?;
                                                    mule_format::ed2k::hash_reader(&mut f)
                                                })
                                                .await;
                                                match res {
                                                    Ok(Ok(hashed)) => {
                                                        if hashed.root_hash.0 == req.file_hash {
                                                            let mut dl_svc = self.downloads.lock().await;
                                                            let _ = dl_svc.store_part_hashes_by_hash(&hash_hex, &hashed.part_hashes);
                                                            Some(hashed.part_hashes)
                                                        } else {
                                                            warn!(
                                                                "[HASHSET] Shared file {} on disk does not match its ed2k hash {}, refusing to serve hashset",
                                                                sf.file_name, hash_hex
                                                            );
                                                            None
                                                        }
                                                    }
                                                    _ => None,
                                                }
                                            }
                                            _ => None,
                                        }
                                    };

                                    if let Some(part_hashes) = part_hashes {
                                        let answer = Ed2kHashSetAnswer {
                                            file_hash: req.file_hash,
                                            part_hashes,
                                        };
                                        if let Ok(ans_pkt) = encoder.encode_hashset_answer(&answer) {
                                            let mut out = Vec::new();
                                            out.push(ans_pkt.protocol);
                                            out.extend_from_slice(&(ans_pkt.frame_len() as u32).to_le_bytes());
                                            out.push(ans_pkt.opcode);
                                            out.extend_from_slice(&ans_pkt.payload);
                                            let _ = stream.write_all(&out).await;
                                            tracing::info!(
                                                "[HASHSET] Served {} part hashes to {} for {}",
                                                answer.part_hashes.len(),
                                                remote_str,
                                                hash_hex
                                            );
                                        }
                                    }
                                }
                            }
                            OP_CANCELTRANSFER => {
                                // eMule OP_CANCELTRANSFER (empty payload): the
                                // downloader aborts the session — free the
                                // upload slot immediately and drop the socket.
                                tracing::info!(
                                    "[UPLOAD] Inbound peer {} sent OP_CANCELTRANSFER, releasing upload slot",
                                    remote_str
                                );
                                self.upload_manager.release_slot(&client_key).await;
                                return;
                            }
                            OP_OUTOFPARTS => {
                                // The downloader signals it currently has no
                                // pending block requests for this file.
                                tracing::debug!(
                                    "Inbound peer {} sent OP_OUTOFPARTS",
                                    remote_str
                                );
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

                                    match self.upload_manager.request_upload_slot(&client_key, &remote_str, &hash_hex, &shared_file.file_name, credit_multiplier, &shared_file.up_priority, sui_rejected).await {
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
                            OP_REQUESTPARTS | OP_REQUESTPARTS_EMULE | OP_REQUESTPARTS_I64 => {
                                 // Decode 32-bit (0x47/0x4B) or 64-bit (0xA3) block
                                 // requests into a common (hash, u64 ranges) shape.
                                 let decoded_req = if pkt.opcode == OP_REQUESTPARTS_I64 {
                                     mule_protocol::ed2k::decoder::decode_request_parts_i64(
                                         pkt.protocol, pkt.opcode, &pkt.payload, 0,
                                     )
                                     .map(|r| {
                                         (
                                             r.file_hash,
                                             r.ranges.iter().map(|rg| (rg.start, rg.end)).collect::<Vec<_>>(),
                                         )
                                     })
                                 } else {
                                     decoder
                                         .decode_request_parts(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                                         .map(|r| {
                                             (
                                                 r.file_hash,
                                                 r.ranges
                                                     .iter()
                                                     .map(|rg| (rg.start as u64, rg.end as u64))
                                                     .collect::<Vec<_>>(),
                                             )
                                         })
                                 };
                                 let is_i64_req = pkt.opcode == OP_REQUESTPARTS_I64;
                                 if let Ok((req_file_hash, req_ranges)) = decoded_req {
                                     let hash_hex = hex::encode(req_file_hash);

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

                                     match self.upload_manager.request_upload_slot(&client_key, &remote_str, &hash_hex, &shared_file.file_name, credit_multiplier, &shared_file.up_priority, sui_rejected).await {
                                         Ok((UploadSlotState::Uploading, _)) => {
                                             if let Some(target_range) = req_ranges.first() {
                                                 let (start, end) = *target_range;
                                                 let length = (end.saturating_sub(start)).min(184320);

                                                 tracing::info!(
                                                     "[UPLOAD_REQUEST] Inbound peer {} requested block [{}..{}] for file '{}' (hash: {})",
                                                     remote_str, start, end, shared_file.file_name, hash_hex
                                                 );

                                                 // Rate limit pacing via network policy upload token bucket.
                                                 // Acquire in <=10240-byte quanta (eMule UploadBandwidthThrottler
                                                 // grants per-slot quanta): keeps inter-slot fairness when
                                                 // MAX_ACTIVE_UPLOADS slots share the global bucket.
                                                 let upload_bucket = &self.network_policy.rate_limiters().upload_bucket;
                                                 let is_cancelled = || cancel_token.is_cancelled();
                                                 let mut throttled_ok = true;
                                                 let mut remaining = length;
                                                 while remaining > 0 {
                                                     let quantum = remaining.min(10240);
                                                     if upload_bucket.acquire_tokens(quantum, is_cancelled).await.is_err() {
                                                         throttled_ok = false;
                                                         break;
                                                     }
                                                     remaining -= quantum;
                                                 }
                                                 if !throttled_ok {
                                                     break;
                                                 }

                                                 // Read real bytes from disk if shared file exists on disk; fallback to synthetic fixture
                                                 let mut payload = None;
                                                 if !shared_file.file_path.is_empty() {
                                                     let p = std::path::Path::new(&shared_file.file_path);
                                                     if p.is_file() {
                                                         if let Ok(mut f) = tokio::fs::File::open(p).await {
                                                             use tokio::io::AsyncSeekExt;
                                                             if f.seek(std::io::SeekFrom::Start(start)).await.is_ok() {
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

                                                 // eMule CreatePackedPackets: when the
                                                 // downloader announced compression support
                                                 // (MiscOptions1 DataCompVer), try
                                                 // OP_COMPRESSEDPART (zlib, Z_BEST_SPEED) and
                                                 // fall back to raw SENDINGPART when
                                                 // compression gains nothing. Already-packed
                                                 // extensions are skipped
                                                 // (ShouldCompressBasedOnFilename) and eMule
                                                 // only packs blocks longer than 31 bytes.
                                                 let compressible_name = match shared_file
                                                     .file_name
                                                     .rsplit_once('.')
                                                 {
                                                     Some((_, ext)) => !matches!(
                                                         ext.to_ascii_lowercase().as_str(),
                                                         "zip" | "rar" | "7z" | "cbz" | "cbr" | "ogm" | "ace"
                                                     ),
                                                     None => true,
                                                 };
                                                 let compressed = if peer_supports_compression
                                                     && compressible_name
                                                     && payload.len() > 31
                                                 {
                                                     use std::io::Write as _;
                                                     let mut enc = flate2::write::ZlibEncoder::new(
                                                         Vec::new(),
                                                         flate2::Compression::new(1),
                                                     );
                                                     if enc.write_all(&payload).is_ok() {
                                                         enc.finish().ok().filter(|c| c.len() < payload.len())
                                                     } else {
                                                         None
                                                     }
                                                 } else {
                                                     None
                                                 };

                                                 if let Some(compressed) = compressed {
                                                     // OP_COMPRESSEDPART (0x40, OP_EMULEPROT):
                                                     // hash16 + u32 block_start + u32 total
                                                     // compressed size + <=10240-byte chunks.
                                                     // I64 variant (0xA1): u64 block_start.
                                                     let mut sent = false;
                                                     for chunk in compressed.chunks(10240) {
                                                         let hdr_len = if is_i64_req { 28 } else { 24 };
                                                         let mut out = Vec::with_capacity(6 + hdr_len + chunk.len());
                                                         out.push(PR_EMULE);
                                                         out.extend_from_slice(&((hdr_len + chunk.len()) as u32).to_le_bytes());
                                                         out.push(if is_i64_req { OP_COMPRESSEDPART_I64 } else { OP_COMPRESSEDPART });
                                                         out.extend_from_slice(&req_file_hash);
                                                         if is_i64_req {
                                                             out.extend_from_slice(&start.to_le_bytes());
                                                         } else {
                                                             out.extend_from_slice(&(start as u32).to_le_bytes());
                                                         }
                                                         out.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
                                                         out.extend_from_slice(chunk);
                                                         if stream.write_all(&out).await.is_err() {
                                                             sent = false;
                                                             break;
                                                         }
                                                         sent = true;
                                                     }
                                                     if sent {
                                                         tracing::info!(
                                                             "[UPLOAD_BLOCK] Sent compressed block ({} -> {} bytes) to {} for file '{}' (offset {}..{})",
                                                             payload.len(), compressed.len(), remote_str, shared_file.file_name, start, start + length
                                                         );
                                                         let reported_rate = upload_bucket.rate_per_sec().max(length);
                                                         self.upload_manager.update_progress(&client_key, length, reported_rate).await;
                                                         if shared_file.file_size > 0 && (start + length) <= shared_file.file_size {
                                                             if let (Some(ref ph), Some(ref pk_der)) = (&remote_pubkey_hash, &remote_pubkey_der) {
                                                                 let pk_hex = hex::encode(pk_der);
                                                                 let mut dl_svc = self.downloads.lock().await;
                                                                 if dl_svc.record_credit_upload(ph, &pk_hex, length).is_ok() {
                                                                     let total_up = dl_svc.get_peer_credit(ph).ok().flatten().map(|c| c.uploaded_bytes).unwrap_or(length);
                                                                     tracing::info!(
                                                                         "[CREDITS_AWARDED] Awarded {} uploaded bytes to verified peer {} (pubkey hash: {}, total: {} bytes)",
                                                                         length, remote_str, ph, total_up
                                                                     );
                                                                 }
                                                             }
                                                         }
                                                     }
                                                     continue;
                                                 }

                                                 let send_pkt = if is_i64_req {
                                                     encoder.encode_sending_part_i64(
                                                         &mule_protocol::ed2k::Ed2kSendingPartI64 {
                                                             file_hash: req_file_hash,
                                                             start_offset: start,
                                                             end_offset: start + length,
                                                             data: Bytes::from(payload),
                                                         },
                                                     )
                                                 } else {
                                                     encoder.encode_sending_part(
                                                         &Ed2kSendingPart::new(
                                                             req_file_hash,
                                                             start as u32,
                                                             (start + length) as u32,
                                                             Bytes::from(payload),
                                                         ),
                                                     )
                                                 };

                                                 if let Ok(send_pkt) = send_pkt {
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

                                                     let reported_rate = upload_bucket.rate_per_sec().max(length);
                                                     self.upload_manager.update_progress(&client_key, length, reported_rate).await;

                                                    if shared_file.file_size > 0 && (start + length) <= shared_file.file_size {
                                                        if let (Some(ref ph), Some(ref pk_der)) = (&remote_pubkey_hash, &remote_pubkey_der) {
                                                        let pk_hex = hex::encode(pk_der);
                                                            let mut dl_svc = self.downloads.lock().await;
                                                            if dl_svc.record_credit_upload(ph, &pk_hex, length).is_ok() {
                                                                let total_up = dl_svc.get_peer_credit(ph).ok().flatten().map(|c| c.uploaded_bytes).unwrap_or(length);
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

        self.upload_manager.release_slot(&client_key).await;
    }

    /// Returns the trusted AICH root of a shared file, computing (and caching)
    /// the hash tree from disk when absent.
    async fn shared_aich_root(
        &self,
        sf: &mule_core::model::SharedFile,
        hash_hex: &str,
    ) -> Option<[u8; 20]> {
        self.shared_aich_tree(sf, hash_hex).await.map(|(_, r)| r)
    }

    /// Loads or computes the full AICH hash tree of a shared file.
    /// The computed root is persisted on the shared-file record.
    async fn shared_aich_tree(
        &self,
        sf: &mule_core::model::SharedFile,
        hash_hex: &str,
    ) -> Option<(mule_format::AichNode, [u8; 20])> {
        if sf.file_path.is_empty() || sf.file_size == 0 {
            return None;
        }
        let path = sf.file_path.clone();
        let size = sf.file_size;
        let tree = tokio::task::spawn_blocking(move || {
            let mut f = std::fs::File::open(&path).ok()?;
            mule_format::build_aich_tree_from_reader(&mut f, size).ok()
        })
        .await
        .ok()??;
        let root = tree.root_hash()?.0;
        if let Some(cached) = sf.aich_root {
            if cached != root {
                warn!(
                    "[AICH] Shared file {} on disk does not match its cached AICH root",
                    sf.file_name
                );
                return None;
            }
        } else {
            let mut dl_svc = self.downloads.lock().await;
            let _ = dl_svc.set_shared_aich_root(hash_hex, &root);
        }
        Some((tree, root))
    }

    /// Answers an inbound peer's `OP_SECIDENTSTATE` challenge (signer role):
    /// sends our `OP_PUBLICKEY` when requested (state=2), then `OP_SIGNATURE`
    /// over `peer_pubkey || challenge` (V1 payload).
    async fn answer_peer_challenge(
        &self,
        state: u8,
        challenge: u32,
        peer_pubkey_der: &[u8],
        remote_str: &str,
        encoder: &Ed2kEncoder,
        stream: &mut ObfuscatedStream<tokio::net::TcpStream>,
    ) {
        let Some(identity) = &self.identity else {
            return;
        };

        if state == 2 {
            if let Some(my_pub) = &self.sui_pubkey_der {
                if let Ok(pk_pkt) = encoder.encode_public_key(&Ed2kPublicKey {
                    key_bytes: Bytes::copy_from_slice(my_pub),
                }) {
                    let mut out = Vec::new();
                    out.push(pk_pkt.protocol);
                    out.extend_from_slice(&(pk_pkt.frame_len() as u32).to_le_bytes());
                    out.push(pk_pkt.opcode);
                    out.extend_from_slice(&pk_pkt.payload);
                    if let Err(e) = stream.write_all(&out).await {
                        warn!("[SUI] Failed to send OP_PUBLICKEY to {}: {}", remote_str, e);
                        return;
                    }
                }
            }
        }

        let payload = match build_sui_signing_payload(
            SecureIdentVersion::V1,
            peer_pubkey_der,
            challenge,
            None,
            None,
        ) {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    "[SUI] Failed to build signing payload for {}: {}",
                    remote_str, e
                );
                return;
            }
        };
        let sig = {
            let id = identity.lock().await;
            id.sign_sui_payload(&payload)
        };
        match sig {
            Ok(sig_bytes) => {
                if let Ok(sig_pkt) = encoder.encode_signature(&Ed2kSignature {
                    signature_bytes: Bytes::from(sig_bytes),
                }) {
                    let mut out = Vec::new();
                    out.push(sig_pkt.protocol);
                    out.extend_from_slice(&(sig_pkt.frame_len() as u32).to_le_bytes());
                    out.push(sig_pkt.opcode);
                    out.extend_from_slice(&sig_pkt.payload);
                    match stream.write_all(&out).await {
                        Ok(()) => {
                            self.sui_stats
                                .signatures_sent
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            tracing::info!(
                                "[SUI] Answered challenge from inbound peer {} (state={}, V1 signature)",
                                remote_str, state
                            );
                        }
                        Err(e) => {
                            warn!("[SUI] Failed to send OP_SIGNATURE to {}: {}", remote_str, e)
                        }
                    }
                }
            }
            Err(e) => warn!("[SUI] Failed to sign challenge from {}: {}", remote_str, e),
        }
    }
}
