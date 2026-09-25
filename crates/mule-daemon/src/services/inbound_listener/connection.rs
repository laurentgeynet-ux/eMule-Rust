// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Inbound connection handling, obfuscation detection, and handshake logic.

use crate::services::peer_session::log_addr_str;
use crate::services::peer_session::log_peer_addr;
use bytes::BytesMut;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::warn;

use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kHelloAnswer, OP_HELLO, OP_HELLO_ANSWER, PR_ED2K, PR_EMULE,
};
use mule_protocol::framing::PacketCodec;

use super::types::{InboundListenerService, MAX_READ_BUFFER_SIZE};

impl InboundListenerService {
    /// Handles an accepted inbound connection from a remote peer.
    pub async fn handle_inbound_connection(
        &self,
        mut socket: TcpStream,
        remote_addr: SocketAddr,
        actual_listen_addr: SocketAddr,
        cancel_token: mule_network_policy::CancellationToken,
    ) {
        let remote_str = remote_addr.to_string();

        // 1. IP Filter check
        if let Some(rule) = self.network_policy.ip_filter().check_ip(remote_addr.ip()) {
            tracing::info!(
                "[IP_FILTER] Dropped inbound TCP connection from {} (rule: '{}', level: {})",
                log_addr_str(&remote_str),
                rule.description,
                rule.level
            );
            return;
        }

        // Protocol Obfuscation detection on first byte
        let mut first_byte = [0u8; 1];
        let read_res = tokio::select! {
            _ = cancel_token.cancelled() => return,
            res = socket.read_exact(&mut first_byte) => res,
        };
        if read_res.is_err() {
            return;
        }
        let b = first_byte[0];
        let is_standard_proto = mule_protocol::ed2k::obfuscation::is_standard_protocol_marker(b);

        let mut stream = if is_standard_proto {
            if self
                .obfuscation_required
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                warn!("Rejected unencrypted inbound connection from {} because obfuscation is required", log_addr_str(&remote_str));
                return;
            }
            crate::services::obfuscation_stream::ObfuscatedStream::plaintext_with_prefix(
                socket,
                vec![b],
            )
        } else {
            if !self
                .obfuscation_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                warn!("Rejected obfuscated inbound connection from {} because obfuscation is disabled", log_addr_str(&remote_str));
                return;
            }
            let local_user_hash = *self.user_hash.read().unwrap();
            match crate::services::obfuscation_stream::ObfuscatedStream::accept_peer_incoming(
                socket,
                b,
                &local_user_hash,
                self.obfuscation_padding_max,
            )
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    warn!(
                        "Inbound obfuscation handshake failed from {}: {}",
                        log_addr_str(&remote_str),
                        e
                    );
                    return;
                }
            }
        };

        let mut read_buf = BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let decoder = Ed2kDecoder::default();
        let encoder = Ed2kEncoder::default();

        let handshake_timeout = Duration::from_secs(8);

        // --- Step 1: Handshake (OP_HELLO) ---
        let handshake_res = tokio::time::timeout(handshake_timeout, async {
            loop {
                let n = stream.read(&mut raw_buf).await.map_err(|e| e.to_string())?;
                if n == 0 {
                    return Err("EOF during handshake".to_string());
                }
                read_buf.extend_from_slice(&raw_buf[..n]);
                if read_buf.len() > MAX_READ_BUFFER_SIZE {
                    return Err("Read buffer limit exceeded during handshake".to_string());
                }

                while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                    if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO {
                        let hello = decoder
                            .decode_hello(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                            .map_err(|e| format!("Invalid OP_HELLO payload: {e}"))?;
                        return Ok(hello);
                    }
                }
            }
        })
        .await;

        let callback_hello: Option<mule_protocol::ed2k::Ed2kHello>;
        match handshake_res {
            Ok(Ok(hello)) => {
                let peer_info = mule_protocol::ed2k::Ed2kPeerInfo::from_hello(&hello);
                tracing::info!(
                    "Inbound peer {peer} handshake: software={} version={:?} emule_version={:?} client_id={} misc1={:?} misc2={:?} \
                     caps: sx={} sx2={} aich={} sec_ident={} crypt(sup/req/reqd)={}/{}/{} udp_ver={} udp_port={:?} ext_req={} kad_ver={} large_files={} compression={} \
                     unknown_tags={}",
                    peer_info.software_name(),
                    peer_info.version,
                    peer_info.emule_version_string(),
                    peer_info.client_id,
                    peer_info.misc_options1.map(|m| format!("0x{m:08X}")),
                    peer_info.misc_options2.map(|m| format!("0x{m:08X}")),
                    peer_info.supports_source_exchange(),
                    peer_info.supports_source_exchange2(),
                    peer_info.supports_aich(),
                    peer_info.secure_ident_support(),
                    peer_info.supports_crypt_layer(),
                    peer_info.requests_crypt_layer(),
                    peer_info.requires_crypt_layer(),
                    peer_info.udp_version(),
                    peer_info.emule_udp_port,
                    peer_info.extended_requests_version(),
                    peer_info.kad_version(),
                    peer_info.supports_large_files(),
                    peer_info.has_compression_capability(),
                    peer_info.unknown_tags.len()
                , peer = log_addr_str(&remote_str));
                callback_hello = Some(hello);
                let mut tags = Vec::new();

                // Mirror the outbound OP_HELLO tag set: eMule expects name,
                // ed2k version, UDP ports and eMule version in HELLOANSWER too.
                use mule_protocol::ed2k::tags::{
                    Ed2kTag, TagName, TagValue, TAG_SPECIAL_CT_EMULE_VERSION,
                    TAG_SPECIAL_EMULE_MISCOPTIONS1, TAG_SPECIAL_EMULE_MISCOPTIONS2,
                    TAG_SPECIAL_EMULE_UDPPORTS, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
                };
                // TAG_NAME = le nickname effectif (eMule `nick` pref —
                // l'HELLOANSWER doit refléter le pseudo configuré, pas une
                // constante), avec le même défaut que `current_nickname`.
                let nickname = self
                    .downloads
                    .lock()
                    .await
                    .get_setting("nickname")
                    .ok()
                    .flatten()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "eMule-Rust".to_string());
                tags.push(Ed2kTag {
                    name: TagName::Id(TAG_SPECIAL_NAME),
                    value: TagValue::String(nickname),
                });
                tags.push(Ed2kTag {
                    name: TagName::Id(TAG_SPECIAL_VERSION),
                    value: TagValue::UInt32(0x3C),
                });
                let udp_ports =
                    ((self.kad_udp_port as u32) << 16) | (actual_listen_addr.port() as u32);
                tags.push(Ed2kTag {
                    name: TagName::Id(TAG_SPECIAL_EMULE_UDPPORTS),
                    value: TagValue::UInt32(udp_ports),
                });
                // Announced as eMule 0.70a (software ID 0, CT_EMULE_VERSION).
                tags.push(Ed2kTag {
                    name: TagName::Id(TAG_SPECIAL_CT_EMULE_VERSION),
                    value: TagValue::UInt32(70 << 10),
                });

                // eMule MiscOptions1 — SecIdent=3 (v1|v2) in bits 16-19 when a
                // usable RSA key is loaded, mirroring the outbound HELLO.
                let sec_ident: u32 = if self.sui_pubkey_der.read().unwrap().is_some() {
                    3
                } else {
                    0
                };
                // MultiPacket = 1 (comme eMule), Preview = 0 (non implémenté).
                let misc1: u32 = (1 << 29)
                    | (1 << 28)
                    | (4 << 24)
                    | (1 << 20)
                    | ((sec_ident & 0x0F) << 16)
                    | (4 << 12)
                    | (2 << 8)
                    | (1 << 4)
                    | (1 << 1);
                tags.push(Ed2kTag {
                    name: TagName::Id(TAG_SPECIAL_EMULE_MISCOPTIONS1),
                    value: TagValue::UInt32(misc1),
                });
                // Identité ed2k réelle — remontée avant MiscOptions2 pour
                // le bit « DirectUDPCallback » (`IsFirewalled` ⇔ pas de
                // HighID).
                let ids = self.sx_local_ids(actual_listen_addr.port()).await;

                // MiscOptions2 — same layout as the outbound HELLO:
                // FileIdent<<13 | DirectUDPCallback<<12 | SrcExch2<<10 |
                // CryptReq<<9 | CryptWants<<8 | CryptSup<<7 | ExtMultiPkt<<5
                // | LargeFiles<<4 | KadVersion<<0.
                // eMule `BaseClient.cpp:1007` — bit 12 quand TCP est
                // inatteignable (client_id < 0x01000000) mais l'UDP Kad est
                // testé ouvert (full-cone NAT) : un pair peut alors nous
                // demander un `OP_DIRECTCALLBACKREQ`.
                let udp_verified_open = {
                    let svc = self.kad_udp.read().unwrap().clone();
                    match svc {
                        Some(kad_udp) => {
                            let tracker = kad_udp.firewall_tracker();
                            let fw = tracker.lock().await;
                            fw.verified
                                && fw.status == crate::services::kad_udp::UdpFirewallStatus::Open
                        }
                        None => false,
                    }
                };
                let misc2: u32 = crate::services::hello_caps::misc_options2(
                    ids.client_id < 0x0100_0000,
                    udp_verified_open,
                    self.obfuscation_enabled
                        .load(std::sync::atomic::Ordering::Relaxed),
                    self.obfuscation_required
                        .load(std::sync::atomic::Ordering::Relaxed),
                );
                tags.push(Ed2kTag {
                    name: TagName::Id(TAG_SPECIAL_EMULE_MISCOPTIONS2),
                    value: TagValue::UInt32(misc2),
                });

                // Identité ed2k réelle (ID assigné par le serveur courant,
                // IP/port du serveur) — eMule `SendHelloTypePacket` écrit
                // `GetUserIDHybrid()` et `GetCurrentServer()` ; 0 si non
                // connecté.
                let ans = Ed2kHelloAnswer {
                    user_hash: *self.user_hash.read().unwrap(),
                    client_id: ids.client_id,
                    tcp_port: actual_listen_addr.port(),
                    tags,
                    unknown_tags: vec![],
                    server_ip: ids.server_ip,
                    server_port: ids.server_port,
                };
                if let Ok(ans_pkt) = encoder.encode_hello_answer(&ans) {
                    // eMule `ListenSocket.cpp:273-275` : le répondeur envoie
                    // OP_EMULEINFO *avant* OP_HELLOANSWER quand le pair est
                    // eMule-compatible (`SendMuleInfoPacket(false)`).
                    if peer_info.is_emule() {
                        let info = crate::services::peer_session::handshake::build_emule_info(
                            self.kad_udp_port,
                            sec_ident as u8,
                        );
                        if let Ok(info_pkt) = encoder.encode_emule_info(&info) {
                            let mut out = Vec::with_capacity(6 + info_pkt.payload.len());
                            out.push(info_pkt.protocol);
                            out.extend_from_slice(&(info_pkt.frame_len() as u32).to_le_bytes());
                            out.push(info_pkt.opcode);
                            out.extend_from_slice(&info_pkt.payload);
                            let _ = stream.write_all(&out).await;
                        }
                    }
                    let mut out = Vec::new();
                    out.push(ans_pkt.protocol);
                    out.extend_from_slice(&(ans_pkt.frame_len() as u32).to_le_bytes());
                    out.push(ans_pkt.opcode);
                    out.extend_from_slice(&ans_pkt.payload);
                    let _ = stream.write_all(&out).await;
                }
            }
            _ => {
                return;
            }
        }

        // Kad bootstrap from clients: an inbound peer advertising a Kad UDP
        // port (CT_EMULE_UDPPORTS high word) can seed our routing table when
        // it is thin — mirrors eMule's "bootstrap from clients" behaviour.
        if let Some(hello) = callback_hello.as_ref() {
            for tag in &hello.tags {
                if let mule_protocol::ed2k::tags::TagName::Id(
                    mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_UDPPORTS,
                ) = tag.name
                {
                    if let mule_protocol::ed2k::tags::TagValue::UInt32(ports) = tag.value {
                        let peer_kad_port = (ports >> 16) as u16;
                        if peer_kad_port > 0 {
                            let low_id = hello.client_id < 16_777_216;
                            let kad_version =
                                mule_protocol::ed2k::Ed2kPeerInfo::from_hello(hello).kad_version();
                            if let IpAddr::V4(ip4) = remote_addr.ip() {
                                let kad_svc = self.kad.read().unwrap().clone();
                                if let Some(k) = kad_svc {
                                    tokio::spawn(async move {
                                        k.maybe_bootstrap_from_peer(
                                            ip4,
                                            peer_kad_port,
                                            low_id,
                                            kad_version,
                                        )
                                        .await;
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Buddy link adoption: a LowID client we accepted via
        // KADEMLIA_FINDBUDDY_REQ connects to our TCP port; the link is kept
        // alive (OP_BUDDYPING→OP_BUDDYPONG) and used to relay OP_CALLBACK /
        // OP_REASKCALLBACKTCP from requesters (eMule `IncomingBuddy` +
        // `KS_CONNECTED_BUDDY`).
        let pending_buddy = {
            let kad_udp = self.kad_udp.read().unwrap().clone();
            match kad_udp {
                Some(k) => k
                    .buddy_registry()
                    .lock()
                    .await
                    .take_pending(&remote_addr.ip()),
                None => None,
            }
        };
        if let Some(pb) = pending_buddy {
            tracing::info!(
                "Inbound connection from {peer} matched pending Kad buddy (buddy_id={}) — adopting as buddy link",
                hex::encode(pb.buddy_id)
            , peer = log_addr_str(&remote_str));
            self.serve_buddy_link(stream, remote_addr, pb, cancel_token.clone())
                .await;
            return;
        }

        // LowID callback adoption: if the connecting peer announced a LowID
        // client_id that matches a pending server-relayed callback, adopt the
        // socket as a client-role download session instead of serving uploads.
        let pending_cb = callback_hello.as_ref().and_then(|hello| {
            if hello.client_id == 0 || hello.client_id >= 16_777_216 {
                return None;
            }
            let ps = self.peer_session.read().unwrap().clone()?;
            ps.take_pending_callback(hello.client_id)
                .map(|file_hash| (ps, file_hash, hello.client_id))
        });

        if let Some((ps, file_hash, lowid)) = pending_cb {
            let hello = callback_hello.as_ref().unwrap();
            let peer_info = mule_protocol::ed2k::Ed2kPeerInfo::from_hello(hello);
            let source_address = format!("lowid:{lowid}");
            tracing::info!(
                "Inbound connection from {peer} matched pending LowID callback (client_id={lowid}, file={file_hash}) — adopting as download session"
            , peer = log_addr_str(&remote_str));
            let software = format!(
                "{} {}",
                peer_info.software_name(),
                peer_info.emule_version_string().unwrap_or_default()
            );
            let _ = self.downloads.lock().await.update_peer_source_details(
                &file_hash,
                &source_address,
                Some(software.trim()),
                None,
            );
            if ps
                .accept_callback(
                    stream,
                    remote_addr,
                    &file_hash,
                    source_address,
                    crate::services::peer_session::source_exchange::peer_sx_caps(&peer_info, None),
                    peer_info.supports_aich(),
                    peer_info.secure_ident_support(),
                    peer_info.extended_requests_version(),
                    self.downloads.clone(),
                    cancel_token.clone(),
                    self.sx_env(),
                    self.sx_local_ids(actual_listen_addr.port()).await,
                )
                .await
                .is_ok()
            {
                return;
            }
            // Adoption refused (limits/dedup): fall through to normal serving.
            return;
        }

        // Kad buddy callback adoption: a LowID Kad source we pinged via
        // KADEMLIA_CALLBACK_REQ connects inbound; matched by remote IP.
        let pending_kad_cb = {
            let ps = self.peer_session.read().unwrap().clone();
            ps.and_then(|ps| {
                ps.take_pending_kad_callback(remote_addr.ip())
                    .map(|file_hash| (ps, file_hash))
            })
        };
        if let Some((ps, file_hash)) = pending_kad_cb {
            let peer_info = callback_hello
                .as_ref()
                .map(mule_protocol::ed2k::Ed2kPeerInfo::from_hello);
            let source_address = format!("kadcb:{}", remote_addr.ip());
            tracing::info!(
                "Inbound connection from {peer} matched pending Kad buddy callback (file={file_hash}) — adopting as download session"
            , peer = log_addr_str(&remote_str));
            if let Some(p) = peer_info.as_ref() {
                let software = format!(
                    "{} {}",
                    p.software_name(),
                    p.emule_version_string().unwrap_or_default()
                );
                let _ = self.downloads.lock().await.update_peer_source_details(
                    &file_hash,
                    &source_address,
                    Some(software.trim()),
                    None,
                );
            }
            let _ = ps
                .accept_callback(
                    stream,
                    remote_addr,
                    &file_hash,
                    source_address,
                    peer_info
                        .as_ref()
                        .map(|p| {
                            crate::services::peer_session::source_exchange::peer_sx_caps(p, None)
                        })
                        .unwrap_or_default(),
                    peer_info
                        .as_ref()
                        .map(|p| p.supports_aich())
                        .unwrap_or(false),
                    peer_info
                        .as_ref()
                        .map(|p| p.secure_ident_support())
                        .unwrap_or(0),
                    peer_info
                        .as_ref()
                        .map(|p| p.extended_requests_version())
                        .unwrap_or(0),
                    self.downloads.clone(),
                    cancel_token.clone(),
                    self.sx_env(),
                    self.sx_local_ids(actual_listen_addr.port()).await,
                )
                .await;
            return;
        }

        // Record the peer's client-UDP version so OP_REASKACK replies match
        // its GetUDPVersion() semantics (part status only for ver > 3).
        if let Some(hello) = callback_hello.as_ref() {
            let peer_info = mule_protocol::ed2k::Ed2kPeerInfo::from_hello(hello);
            let svc = self.client_udp.read().unwrap().clone();
            if let Some(svc) = svc {
                svc.note_client_udp_version(remote_addr.ip(), peer_info.udp_version())
                    .await;
            }
        }

        // eMule MiscOptions1 bit 4 (nDataCompVer): the peer may accept
        // OP_COMPRESSEDPART zlib blocks on uploads.
        let peer_supports_compression = callback_hello
            .as_ref()
            .map(|h| mule_protocol::ed2k::Ed2kPeerInfo::from_hello(h).has_compression_capability())
            .unwrap_or(false);

        // Upload identity: userhash from OP_HELLO when announced (eMule
        // keys the upload queue on the client identity, not the IP).
        let peer_user_hash = callback_hello
            .as_ref()
            .map(|h| h.user_hash)
            .filter(|h| *h != [0u8; 16]);

        // Capacités SX + version de requêtes étendues du pair, extraites du
        // HELLO (CT_EMULE_MISCOPTIONS*) — raffinées par OP_EMULEINFO dans le
        // dispatcher.
        let peer_info = callback_hello
            .as_ref()
            .map(mule_protocol::ed2k::Ed2kPeerInfo::from_hello);

        self.dispatch_requests(
            stream,
            remote_str,
            actual_listen_addr.ip(),
            cancel_token,
            peer_supports_compression,
            peer_user_hash,
            peer_info
                .as_ref()
                .map(|p| crate::services::peer_session::source_exchange::peer_sx_caps(p, None))
                .unwrap_or_default(),
            peer_info
                .as_ref()
                .map(|p| p.extended_requests_version())
                .unwrap_or(0),
            callback_hello.as_ref().map(|h| h.tcp_port).unwrap_or(0),
            peer_info.as_ref().and_then(|p| p.emule_udp_port),
        )
        .await;
    }

    /// Outbound connect-back after a server-relayed `OP_CALLBACKREQUESTED`
    /// (eMule `ServerSocket` → `TryToConnect`): we dial the requester, do
    /// the eD2K hello, then serve its requests like an inbound session.
    pub async fn connect_back(
        &self,
        target: SocketAddr,
        peer_user_hash: Option<[u8; 16]>,
        crypt_options: u8,
        our_client_id: u32,
        server_ip: u32,
        server_port: u16,
    ) -> Result<(), String> {
        let permit = self
            .network_policy
            .request_tcp_connect(target, mule_network_policy::ConnectionPurpose::Ed2kPeer)
            .map_err(|e| format!("Network policy rejected callback connect to {target}: {e}"))?;
        let cancel_token = permit.cancellation_token().clone();

        let stream = tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err("Cancelled by network policy".into());
            }
            res = tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(target)) => {
                res.map_err(|_| format!("TCP connect timed out to {target}"))?
                    .map_err(|e| format!("TCP connect to {target} failed: {e}"))?
            }
        };
        let local_ip = stream
            .local_addr()
            .map(|a| a.ip())
            .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));

        // Obfuscation: when the requester announced crypt support and we have
        // its user hash (CALLBACKREQUESTED ≥23B form), use the encrypted
        // handshake; otherwise plaintext.
        let use_obfu = crypt_options & 0x01 != 0
            && self
                .obfuscation_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
            && peer_user_hash.is_some();
        let mut stream = if use_obfu {
            crate::services::obfuscation_stream::ObfuscatedStream::connect_peer_outgoing(
                stream,
                &peer_user_hash.unwrap(),
                self.obfuscation_padding_max,
            )
            .await
            .map_err(|e| format!("Callback connect obfuscation handshake failed: {e}"))?
        } else {
            crate::services::obfuscation_stream::ObfuscatedStream::plaintext(stream)
        };

        // Send OP_HELLO (same capability tags as the inbound answer).
        let encoder = Ed2kEncoder::default();
        let decoder = Ed2kDecoder::default();
        let codec = PacketCodec::new();
        let sec_ident: u32 = if self.sui_pubkey_der.read().unwrap().is_some() {
            3
        } else {
            0
        };
        let misc1: u32 = (1 << 29)
            | (1 << 28)
            | (4 << 24)
            | (1 << 20)
            | ((sec_ident & 0x0F) << 16)
            | (4 << 12)
            | (1 << 4)
            | 1;
        let mut tags = vec![mule_protocol::ed2k::tags::Ed2kTag {
            name: mule_protocol::ed2k::tags::TagName::Id(
                mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_MISCOPTIONS1,
            ),
            value: mule_protocol::ed2k::tags::TagValue::UInt32(misc1),
        }];
        // Même MiscOptions2 que les hellos normaux — eMule utilise le même
        // `SendHelloPacket` pour le connect-back callback.
        let udp_verified_open = {
            let svc = self.kad_udp.read().unwrap().clone();
            match svc {
                Some(kad_udp) => {
                    let tracker = kad_udp.firewall_tracker();
                    let fw = tracker.lock().await;
                    fw.verified && fw.status == crate::services::kad_udp::UdpFirewallStatus::Open
                }
                None => false,
            }
        };
        let misc2: u32 = crate::services::hello_caps::misc_options2(
            our_client_id < 0x0100_0000,
            udp_verified_open,
            self.obfuscation_enabled
                .load(std::sync::atomic::Ordering::Relaxed),
            self.obfuscation_required
                .load(std::sync::atomic::Ordering::Relaxed),
        );
        tags.push(mule_protocol::ed2k::tags::Ed2kTag {
            name: mule_protocol::ed2k::tags::TagName::Id(
                mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_MISCOPTIONS2,
            ),
            value: mule_protocol::ed2k::tags::TagValue::UInt32(misc2),
        });
        let local_user_hash = *self.user_hash.read().unwrap();
        let hello = mule_protocol::ed2k::Ed2kHello {
            user_hash: local_user_hash,
            client_id: our_client_id,
            tcp_port: self.listen_addr.lock().await.map(|a| a.port()).unwrap_or(0),
            tags,
            unknown_tags: vec![],
            server_ip,
            server_port,
        };
        let pkt = encoder
            .encode_hello(&hello)
            .map_err(|e| format!("Failed to encode OP_HELLO: {e}"))?;
        let mut out = Vec::with_capacity(6 + pkt.payload.len());
        out.push(pkt.protocol);
        out.extend_from_slice(&(pkt.frame_len() as u32).to_le_bytes());
        out.push(pkt.opcode);
        out.extend_from_slice(&pkt.payload);
        stream
            .write_all(&out)
            .await
            .map_err(|e| format!("Failed to send OP_HELLO to {target}: {e}"))?;

        // Read OP_HELLO_ANSWER.
        let mut read_buf = BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let remote_str = target.to_string();
        let answer = tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                let n = stream.read(&mut raw_buf).await.map_err(|e| e.to_string())?;
                if n == 0 {
                    return Err("EOF during callback handshake".to_string());
                }
                read_buf.extend_from_slice(&raw_buf[..n]);
                if read_buf.len() > MAX_READ_BUFFER_SIZE {
                    return Err("Read buffer limit exceeded during handshake".to_string());
                }
                while let Ok(Some(p)) = codec.decode(&mut read_buf) {
                    if p.protocol == PR_ED2K && p.opcode == OP_HELLO_ANSWER {
                        return decoder
                            .decode_hello_answer(p.protocol, p.opcode, &p.payload, 0)
                            .map_err(|e| format!("Invalid OP_HELLO_ANSWER: {e}"));
                    }
                }
            }
        })
        .await;

        let answer = match answer {
            Ok(Ok(a)) => a,
            Ok(Err(e)) => return Err(format!("Callback handshake with {target} failed: {e}")),
            Err(_) => return Err(format!("Callback handshake with {target} timed out")),
        };

        let peer_info = mule_protocol::ed2k::Ed2kPeerInfo::from_hello_answer(&answer);
        let peer_hash = if answer.user_hash != [0u8; 16] {
            Some(answer.user_hash)
        } else {
            peer_user_hash
        };
        tracing::info!(
            "Callback connect-back to {peer} established (OP_HELLO_ANSWER ok) — serving requests",
            peer = log_peer_addr(target)
        );
        self.dispatch_requests(
            stream,
            remote_str,
            local_ip,
            cancel_token,
            peer_info.has_compression_capability(),
            peer_hash,
            crate::services::peer_session::source_exchange::peer_sx_caps(&peer_info, None),
            peer_info.extended_requests_version(),
            answer.tcp_port,
            peer_info.emule_udp_port,
        )
        .await;
        Ok(())
    }

    /// Serves an adopted Kad buddy link (eMule `KS_CONNECTED_BUDDY`): keeps
    /// the TCP connection open, answers `OP_BUDDYPING` with `OP_BUDDYPONG`,
    /// and forwards packets queued by the buddy relay (`OP_CALLBACK`,
    /// `OP_REASKCALLBACKTCP`) through an mpsc writer.
    pub(crate) async fn serve_buddy_link(
        &self,
        stream: crate::services::obfuscation_stream::ObfuscatedStream<TcpStream>,
        remote_addr: SocketAddr,
        pending: crate::services::kad_udp::PendingBuddy,
        cancel_token: mule_network_policy::CancellationToken,
    ) {
        let Some(kad_udp) = self.kad_udp.read().unwrap().clone() else {
            return;
        };
        let registry = kad_udp.buddy_registry();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        {
            let mut reg = registry.lock().await;
            if !reg.set_active(crate::services::kad_udp::ActiveBuddy {
                buddy_id: pending.buddy_id,
                user_id: pending.user_id,
                remote_addr,
                tx: tx.clone(),
            }) {
                tracing::warn!(
                    "Buddy link from {peer} rejected: another buddy is already active",
                    peer = log_peer_addr(remote_addr)
                );
                return;
            }
        }

        let (mut rd, mut wr) = tokio::io::split(stream);
        let writer = tokio::spawn(async move {
            while let Some(pkt) = rx.recv().await {
                if wr.write_all(&pkt).await.is_err() {
                    break;
                }
            }
        });

        let mut read_buf = BytesMut::with_capacity(8192);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => break,
                res = rd.read(&mut raw_buf) => {
                    match res {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            read_buf.extend_from_slice(&raw_buf[..n]);
                            if read_buf.len() > MAX_READ_BUFFER_SIZE {
                                break;
                            }
                            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                                if pkt.protocol == PR_EMULE
                                    && pkt.opcode == mule_protocol::ed2k::OP_BUDDYPING
                                {
                                    // eMule ListenSocket.cpp: OP_BUDDYPING →
                                    // empty OP_BUDDYPONG.
                                    let pong = {
                                        let mut f = Vec::with_capacity(6);
                                        f.push(PR_EMULE);
                                        f.extend_from_slice(&1u32.to_le_bytes());
                                        f.push(mule_protocol::ed2k::OP_BUDDYPONG);
                                        f
                                    };
                                    let _ = tx.send(pong);
                                } else {
                                    tracing::debug!(
                                        "Buddy link {}: ignoring packet proto=0x{:02X} opcode=0x{:02X}",
                                        log_peer_addr(remote_addr), pkt.protocol, pkt.opcode
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        writer.abort();
        registry.lock().await.clear_active(&remote_addr);
        tracing::info!(
            "Kad buddy link from {peer} closed",
            peer = log_peer_addr(remote_addr)
        );
    }
}
