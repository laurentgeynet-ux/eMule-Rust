// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Outbound eMule Secure User Identification (signer side).
//!
//! When a remote client challenges us with `OP_SECIDENTSTATE`, we answer with
//! our RSA-384 public key (if requested, state=2) followed by `OP_SIGNATURE`
//! over `peer_pubkey || challenge` (V1 payload — see
//! docs/ed2k/SUI_WIRE_FORMAT.md). The peer's public key is learned from its
//! `OP_PUBLICKEY` packet; without it the payload cannot be built, matching
//! eMule which requires the verifier's key.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use mule_crypto::legacy_sui::{
    build_sui_signing_payload, parse_peer_public_key, verify_sui_signature, SecureIdentVersion,
    MAX_PUBKEY_WIRE,
};
use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kPublicKey, Ed2kSecIdentState, Ed2kSignature, OP_PUBLICKEY,
    OP_SECIDENTSTATE, OP_SIGNATURE, PR_EMULE,
};

use crate::services::obfuscation_stream::ObfuscatedStream;
use crate::services::IdentityService;

pub(crate) struct OutboundSui {
    identity: Option<Arc<Mutex<IdentityService>>>,
    /// Challenger (remote peer) public key DER, learned from its OP_PUBLICKEY.
    peer_pubkey_der: Option<Vec<u8>>,
    /// Only one signature per connection (eMule anti-flood semantics).
    answered: bool,
    /// Peer's `m_bySupportSecIdent` announced in the handshake (0 = none).
    peer_sec_ident: u8,
    /// Challenge we issued to the peer (verifier role), consumed once.
    challenge_issued: Option<u32>,
    /// Whether we already sent our OP_SECIDENTSTATE challenge.
    challenge_sent: bool,
    /// Challenge the peer asked us to answer, parked until its OP_PUBLICKEY
    /// arrives — real eMule peers may send OP_SECIDENTSTATE first.
    pending_challenge: Option<(u8, u32)>,
    /// Shared SUI anomaly counters (identity.status `sui` object).
    sui_stats: Option<Arc<crate::services::identity::SuiStats>>,
    /// Peer signature verified — its pubkey identity may accrue download
    /// credits (eMule only credits cryptographically verified clients).
    peer_verified: bool,
}

impl OutboundSui {
    pub fn new(identity: Option<Arc<Mutex<IdentityService>>>) -> Self {
        Self {
            identity,
            peer_pubkey_der: None,
            answered: false,
            peer_sec_ident: 0,
            challenge_issued: None,
            challenge_sent: false,
            pending_challenge: None,
            sui_stats: None,
            peer_verified: false,
        }
    }

    pub fn with_stats(mut self, stats: Arc<crate::services::identity::SuiStats>) -> Self {
        self.sui_stats = Some(stats);
        self
    }

    fn bump(
        &self,
        f: impl Fn(&crate::services::identity::SuiStats) -> &std::sync::atomic::AtomicU64,
    ) {
        if let Some(s) = &self.sui_stats {
            f(s).fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    pub fn set_peer_sec_ident(&mut self, cap: u8) {
        self.peer_sec_ident = cap;
    }

    /// eMule `SendSecIdentStatePacket` / `InfoPacketsReceived`: once both info
    /// packets are exchanged, if the peer announced Secure Ident support we
    /// challenge it — `state=2` (IS_KEYANDSIGNEEDED) since we never carry a
    /// stored peer key across connections.
    pub async fn initiate(
        &mut self,
        target_addr: SocketAddr,
        encoder: &Ed2kEncoder,
        stream: &mut ObfuscatedStream<TcpStream>,
    ) {
        if self.challenge_sent || self.peer_sec_ident == 0 {
            return;
        }
        let has_key = match &self.identity {
            Some(id) => id.lock().await.has_key(),
            None => false,
        };
        if !has_key {
            return;
        }
        let challenge: u32 = rand::random::<u32>().wrapping_add(1);
        match encoder.encode_secident_state(&Ed2kSecIdentState {
            state: 2,
            challenge,
        }) {
            Ok(pkt) => {
                let mut out = Vec::with_capacity(6 + pkt.payload.len());
                out.push(pkt.protocol);
                out.extend_from_slice(&(pkt.frame_len() as u32).to_le_bytes());
                out.push(pkt.opcode);
                out.extend_from_slice(&pkt.payload);
                match stream.write_all(&out).await {
                    Ok(()) => {
                        self.challenge_sent = true;
                        self.challenge_issued = Some(challenge);
                        self.bump(|s| &s.challenges_issued_out);
                        tracing::debug!(
                            "[SUI] Sent OP_SECIDENTSTATE challenge to {target_addr} (state=2)"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[SUI] Failed to send OP_SECIDENTSTATE to {target_addr}: {e}"
                        )
                    }
                }
            }
            Err(e) => tracing::warn!("[SUI] Failed to encode OP_SECIDENTSTATE: {e}"),
        }
    }

    /// Answers a peer challenge (`OP_SECIDENTSTATE` signer role): sends our
    /// `OP_PUBLICKEY` when the peer requested it (state=2), then `OP_SIGNATURE`
    /// over `peer_pubkey || challenge` (V1). Requires `peer_pubkey_der`.
    async fn answer_challenge(
        &mut self,
        state: u8,
        challenge: u32,
        target_addr: SocketAddr,
        encoder: &Ed2kEncoder,
        stream: &mut ObfuscatedStream<TcpStream>,
    ) {
        let Some(identity) = &self.identity else {
            return;
        };
        let Some(peer_pub) = &self.peer_pubkey_der else {
            return;
        };

        if state == 2 {
            let my_pub = {
                let id = identity.lock().await;
                id.public_key_bytes().to_vec()
            };
            if let Ok(pk_pkt) = encoder.encode_public_key(&Ed2kPublicKey {
                key_bytes: bytes::Bytes::from(my_pub),
            }) {
                let mut out = Vec::new();
                out.push(pk_pkt.protocol);
                out.extend_from_slice(&(pk_pkt.frame_len() as u32).to_le_bytes());
                out.push(pk_pkt.opcode);
                out.extend_from_slice(&pk_pkt.payload);
                if let Err(e) = stream.write_all(&out).await {
                    tracing::warn!("[SUI] Failed to send OP_PUBLICKEY to {target_addr}: {e}");
                    return;
                }
            }
        }

        let payload = match build_sui_signing_payload(
            SecureIdentVersion::V1,
            peer_pub,
            challenge,
            None,
            None,
        ) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("[SUI] Failed to build signing payload for {target_addr}: {e}");
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
                    signature_bytes: bytes::Bytes::from(sig_bytes),
                }) {
                    let mut out = Vec::new();
                    out.push(sig_pkt.protocol);
                    out.extend_from_slice(&(sig_pkt.frame_len() as u32).to_le_bytes());
                    out.push(sig_pkt.opcode);
                    out.extend_from_slice(&sig_pkt.payload);
                    match stream.write_all(&out).await {
                        Ok(()) => {
                            self.answered = true;
                            self.bump(|s| &s.signatures_sent);
                            tracing::info!(
                                "[SUI] Answered challenge from peer {target_addr} (state={}, V1 signature)",
                                state
                            );
                        }
                        Err(e) => tracing::warn!(
                            "[SUI] Failed to send OP_SIGNATURE to {target_addr}: {e}"
                        ),
                    }
                }
            }
            Err(e) => tracing::warn!("[SUI] Failed to sign challenge from {target_addr}: {e}"),
        }
    }

    /// Handles an outbound-side SUI packet. Returns true when the packet was
    /// consumed (SUI opcodes are never fatal to the transfer).
    pub async fn handle(
        &mut self,
        packet: &mule_protocol::framing::RawPacket,
        target_addr: SocketAddr,
        decoder: &Ed2kDecoder,
        encoder: &Ed2kEncoder,
        stream: &mut ObfuscatedStream<TcpStream>,
    ) -> bool {
        if packet.protocol != PR_EMULE {
            return false;
        }
        match packet.opcode {
            OP_PUBLICKEY => {
                match decoder.decode_public_key(packet.protocol, packet.opcode, &packet.payload, 0)
                {
                    Ok(pk) if pk.key_bytes.len() <= MAX_PUBKEY_WIRE => {
                        tracing::debug!(
                            "[SUI] Peer {target_addr} sent its public key ({} bytes)",
                            pk.key_bytes.len()
                        );
                        self.peer_pubkey_der = Some(pk.key_bytes.to_vec());
                        if let Some((state, challenge)) = self.pending_challenge.take() {
                            self.answer_challenge(state, challenge, target_addr, encoder, stream)
                                .await;
                        }
                    }
                    Ok(pk) => {
                        self.bump(|s| &s.malformed_packets);
                        tracing::warn!(
                            "[SUI] Peer {target_addr} sent oversized public key ({} bytes), ignoring",
                            pk.key_bytes.len()
                        );
                    }
                    Err(e) => {
                        self.bump(|s| &s.malformed_packets);
                        tracing::warn!("[SUI] Malformed OP_PUBLICKEY from {target_addr}: {e}");
                    }
                }
                true
            }
            OP_SECIDENTSTATE => {
                let st = match decoder.decode_secident_state(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                ) {
                    Ok(st) => st,
                    Err(e) => {
                        tracing::warn!("[SUI] Malformed OP_SECIDENTSTATE from {target_addr}: {e}");
                        return true;
                    }
                };
                if st.state == 0 || self.answered {
                    return true;
                }
                if self.identity.is_none() {
                    tracing::debug!(
                        "[SUI] Peer {target_addr} challenged us but no local identity key"
                    );
                    return true;
                }
                if self.peer_pubkey_der.is_none() {
                    // eMule may send the challenge before its OP_PUBLICKEY —
                    // park it and answer when the key arrives.
                    self.pending_challenge = Some((st.state, st.challenge));
                    tracing::debug!(
                        "[SUI] Peer {target_addr} sent challenge (state={}) before its public key; deferred",
                        st.state
                    );
                    return true;
                }
                self.answer_challenge(st.state, st.challenge, target_addr, encoder, stream)
                    .await;
                true
            }
            OP_SIGNATURE => {
                // Verifier role: the peer answered our OP_SECIDENTSTATE
                // challenge. Payload: `our_pubkey || challenge [|| ip || kind]`.
                let Some(challenge) = self.challenge_issued.take() else {
                    tracing::debug!(
                        "[SUI] Peer {target_addr} sent OP_SIGNATURE but we issued no challenge"
                    );
                    return true;
                };
                let Some(identity) = &self.identity else {
                    return true;
                };
                let Some(peer_der) = self.peer_pubkey_der.clone() else {
                    tracing::warn!("[SUI] Peer {target_addr} signed but never sent its public key");
                    return true;
                };
                let sig = match decoder.decode_signature(
                    packet.protocol,
                    packet.opcode,
                    &packet.payload,
                    0,
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        self.bump(|s| &s.malformed_packets);
                        self.bump(|s| &s.signatures_rejected);
                        tracing::warn!("[SUI] Malformed OP_SIGNATURE from {target_addr}: {e}");
                        return true;
                    }
                };
                // V2 wire: [u8 siglen][sig][u8 kind].
                let sig_len = packet.payload.first().copied().unwrap_or(0) as usize;
                let kind = if packet.payload.len() == sig_len + 2 {
                    Some(packet.payload[sig_len + 1])
                } else {
                    None
                };
                let peer_ip_u32 = match target_addr.ip() {
                    std::net::IpAddr::V4(v4) => u32::from_be_bytes(v4.octets()),
                    _ => 0,
                };
                let local_ip_u32 = match stream.inner().local_addr().map(|a| a.ip()) {
                    Ok(std::net::IpAddr::V4(v4)) => u32::from_be_bytes(v4.octets()),
                    _ => 0,
                };
                let (ip_u32, version) = match kind {
                    Some(20) => (peer_ip_u32, SecureIdentVersion::V2), // LOCALCLIENT: signer's IP
                    Some(10) => (local_ip_u32, SecureIdentVersion::V2), // REMOTECLIENT: our IP
                    Some(30) => (0, SecureIdentVersion::V2),           // NONECLIENT
                    Some(_) => {
                        tracing::warn!("[SUI] Peer {target_addr} used invalid IP-binding kind");
                        return true;
                    }
                    None => (0, SecureIdentVersion::V1),
                };
                let my_pub = {
                    let id = identity.lock().await;
                    id.public_key_bytes().to_vec()
                };
                let result =
                    build_sui_signing_payload(version, &my_pub, challenge, Some(ip_u32), kind)
                        .and_then(|payload| {
                            parse_peer_public_key(&peer_der).and_then(|pk| {
                                verify_sui_signature(&pk, &payload, &sig.signature_bytes)
                            })
                        });
                match result {
                    Ok(()) => {
                        self.bump(|s| &s.signatures_verified);
                        self.peer_verified = true;
                        tracing::info!("[SUI] Peer {target_addr} signature VERIFIED ({version:?})");
                    }
                    Err(e) => {
                        self.bump(|s| &s.signatures_rejected);
                        tracing::warn!("[SUI] Peer {target_addr} signature REJECTED: {e}");
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Returns the verified credit identity `(pubkey_md4_hex, pubkey_hex)` of
    /// the remote peer, or `None` when the signature was not verified.
    /// Callers persist download bytes under this identity (eMule credits the
    /// *client key*, never the transient IP).
    pub fn verified_credit_identity(&self) -> Option<(String, String)> {
        if !self.peer_verified {
            return None;
        }
        self.peer_pubkey_der.as_ref().map(|der| {
            (
                hex::encode(<md4::Md4 as md4::Digest>::digest(der)),
                hex::encode(der),
            )
        })
    }
}
