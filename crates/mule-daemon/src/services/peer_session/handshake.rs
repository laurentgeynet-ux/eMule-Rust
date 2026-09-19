use bytes::BytesMut;
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};

use mule_core::model::PeerFailureReason;
use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kHello, OP_HELLO, OP_HELLO_ANSWER, PR_ED2K,
};
use mule_protocol::framing::PacketCodec;

use super::events::emit_event;
use super::types::{
    PeerConnectionStatus, PeerError, PeerTimeouts, SwarmInnerState, MAX_PEER_READ_BUFFER_SIZE,
};
use crate::services::downloads::DownloadService;
use crate::services::obfuscation_stream::ObfuscatedStream;

pub struct HandshakeResult {
    pub stream: ObfuscatedStream<TcpStream>,
    pub peer_supports_sx: bool,
    pub peer_supports_sx2: bool,
    pub peer_supports_aich: bool,
    /// Peer's `m_bySupportSecIdent` (0 = none, bit0 = v1, bit1 = v2).
    pub peer_sec_ident: u8,
    /// Peer's announced eMule UDP port (`CT_EMULE_UDPPORTS` low word), if any.
    pub peer_udp_port: Option<u16>,
    /// Peer's client UDP protocol version (`m_byUDPVer`, MiscOptions1 bits 24-27).
    pub peer_udp_version: u8,
    pub connected_ms: i64,
}

pub(crate) async fn perform_peer_handshake(
    target_addr: SocketAddr,
    file_hash_str: &str,
    address_str: &str,
    client_port: u16,
    nickname: &str,
    user_hash: [u8; 16],
    timeouts: &PeerTimeouts,
    cancel_token: &mule_network_policy::CancellationToken,
    inner: &Arc<Mutex<SwarmInnerState>>,
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    client_id: u32,
    server_ip: u32,
    server_port: u16,
    kad_port: u16,
    kad_service: &Option<Arc<crate::services::KadService>>,
    target_user_hash: Option<[u8; 16]>,
    peer_crypt_options: u8,
    obfuscation_enabled: bool,
    obfuscation_required: bool,
    obfuscation_padding_max: u8,
    downloads: &Arc<Mutex<DownloadService>>,
    sec_ident_cap: u8,
) -> Result<HandshakeResult, PeerError> {
    // 1. TCP connect with timeout & cancel_token
    let stream_res = tokio::select! {
        _ = cancel_token.cancelled() => {
            return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Cancelled by network policy kill switch"));
        }
        res = tokio::time::timeout(timeouts.connect_timeout, TcpStream::connect(target_addr)) => {
            match res {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("TCP connect refused: {e}"))),
                Err(_) => return Err(PeerError::new(PeerFailureReason::TimeoutConnect, "TCP connect timed out")),
            }
        }
    };

    let tcp_stream = stream_res;
    let connected_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Obfuscation decision:
    let peer_requires_crypt = (peer_crypt_options & 0x04) != 0;
    let peer_supports_crypt = (peer_crypt_options & 0x01) != 0 || (peer_crypt_options & 0x02) != 0;

    let use_obfuscation = if obfuscation_required || peer_requires_crypt {
        if target_user_hash.is_none() {
            return Err(PeerError::new(
                PeerFailureReason::MissingUserHashForObfuscation,
                "Peer requires protocol obfuscation but its user hash is unknown",
            ));
        }
        true
    } else if obfuscation_enabled && peer_supports_crypt && target_user_hash.is_some() {
        true
    } else {
        false
    };

    let mut stream = if use_obfuscation {
        let th = target_user_hash.unwrap();
        tracing::info!(
            "Initiating obfuscated handshake to peer {target_addr} with target user hash {:02X?}",
            &th
        );
        match ObfuscatedStream::connect_peer_outgoing(tcp_stream, &th, obfuscation_padding_max)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                if !obfuscation_required && !peer_requires_crypt {
                    tracing::warn!(
                        "Obfuscated handshake to peer {target_addr} failed ({e}), falling back to plaintext connection"
                    );
                    match tokio::time::timeout(
                        timeouts.connect_timeout,
                        TcpStream::connect(target_addr),
                    )
                    .await
                    {
                        Ok(Ok(plain_stream)) => ObfuscatedStream::plaintext(plain_stream),
                        Ok(Err(reconnect_err)) => {
                            return Err(PeerError::new(
                                PeerFailureReason::ConnectRefused,
                                format!("Plaintext fallback connect failed: {reconnect_err}"),
                            ));
                        }
                        Err(_) => {
                            return Err(PeerError::new(
                                PeerFailureReason::TimeoutConnect,
                                "Plaintext fallback connect timed out",
                            ));
                        }
                    }
                } else {
                    tracing::warn!("Obfuscated handshake to peer {target_addr} failed: {e}");
                    return Err(PeerError::new(
                        PeerFailureReason::ObfuscationNegotiationFailed,
                        format!("Protocol obfuscation negotiation failed: {e}"),
                    ));
                }
            }
        }
    } else {
        ObfuscatedStream::plaintext(tcp_stream)
    };

    // Update status: Handshaking
    let handshaking_snapshot = {
        let mut g = inner.lock().await;
        if let Some(session) = g.sessions.get_mut(address_str) {
            session.snapshot.status = PeerConnectionStatus::Handshaking;
            session.snapshot.connected_at_unix_ms = Some(connected_ms);
            session.snapshot.obfuscated = stream.is_obfuscated();
            Some(session.snapshot.clone())
        } else {
            None
        }
    };
    if let Some(snap) = handshaking_snapshot {
        emit_event(event_tx, sequence, &snap);
    }

    // 2. Send OP_HELLO
    let encoder = Ed2kEncoder::default();
    let mut tags = Vec::with_capacity(6);

    // Nickname
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_NAME),
        value: mule_protocol::ed2k::tags::TagValue::String(nickname.to_string()),
    });

    // Version
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(
            mule_protocol::ed2k::tags::TAG_SPECIAL_VERSION,
        ),
        value: mule_protocol::ed2k::tags::TagValue::UInt32(0x3C),
    });

    // UDPPorts
    let udp_ports = ((kad_port as u32) << 16) | (client_port as u32);
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(
            mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_UDPPORTS,
        ),
        value: mule_protocol::ed2k::tags::TagValue::UInt32(udp_ports),
    });

    // EmuleVersion
    let emule_ver: u32 = (0 << 17) | (50 << 10) | (0 << 7);
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(
            mule_protocol::ed2k::tags::TAG_SPECIAL_CT_EMULE_VERSION,
        ),
        value: mule_protocol::ed2k::tags::TagValue::UInt32(emule_ver),
    });

    // MiscOptions1 — eMule layout (BaseClient.cpp): AICH<<29 | Unicode<<28 |
    // UDPVer<<24 | DataComp<<20 | SecIdent<<16 | SrcExch<<12 | ExtReq<<8 |
    // Comments<<4 | NoView<<2 | MultiPkt<<1 | Preview<<0.
    // SecIdent = 3 (v1+v2) when a usable RSA key is loaded, else 0.
    let misc1: u32 = (1 << 29)
        | (1 << 28)
        | (4 << 24)
        | (1 << 20)
        | ((sec_ident_cap as u32 & 0x0F) << 16)
        | (4 << 12)
        | (0 << 8)
        | (1 << 4)
        | (0 << 1)
        | 1;
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(
            mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_MISCOPTIONS1,
        ),
        value: mule_protocol::ed2k::tags::TagValue::UInt32(misc1),
    });

    // MiscOptions2 — eMule layout: FileIdent<<13 | DirectUDPCallback<<12 |
    // Captcha<<11 | SrcExch2<<10 | CryptReq<<9 | CryptWants<<8 | CryptSup<<7 |
    // ExtMultiPkt<<5 | LargeFiles<<4 | KadVersion<<0. KadVersion = 6
    // (KADEMLIA_VERSION6_49aBETA): the daemon implements OP_FWCHECKUDPREQ /
    // KADEMLIA2_FIREWALLUDP and protocol obfuscation.
    let u_supports_crypt = obfuscation_enabled as u32;
    let u_requests_crypt = obfuscation_enabled as u32;
    let u_requires_crypt = obfuscation_required as u32;
    let misc2: u32 = (1 << 10)
        | (u_requires_crypt << 9)
        | (u_requests_crypt << 8)
        | (u_supports_crypt << 7)
        | (1 << 5)
        | (1 << 4)
        | 6;
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(
            mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_MISCOPTIONS2,
        ),
        value: mule_protocol::ed2k::tags::TagValue::UInt32(misc2),
    });

    let hello = Ed2kHello {
        user_hash,
        client_id,
        tcp_port: client_port,
        tags,
        unknown_tags: vec![],
        server_ip,
        server_port,
    };

    let hello_pkt = encoder.encode_hello(&hello).map_err(|e| {
        PeerError::new(
            PeerFailureReason::InvalidHandshake,
            format!("Failed to encode OP_HELLO: {e}"),
        )
    })?;

    let mut out_wire = Vec::with_capacity(6 + hello_pkt.payload.len());
    out_wire.push(hello_pkt.protocol);
    out_wire.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(hello_pkt.opcode);
    out_wire.extend_from_slice(&hello_pkt.payload);

    tracing::info!(
        "Peer {target_addr}: sending OP_HELLO (client_id={client_id}, port={client_port}, server={server_ip}:{server_port}, len={}): {:02X?}",
        out_wire.len(),
        &out_wire[..]
    );

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(
            PeerFailureReason::ConnectRefused,
            format!("Failed to send OP_HELLO: {e}"),
        ));
    }

    // 3. Receive OP_HELLO_ANSWER with Handshake Timeout
    let decoder = Ed2kDecoder::default();
    let codec = PacketCodec::new();
    let mut read_buf = BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let mut received_hello_answer = false;
    let mut peer_supports_sx = false;
    let mut peer_supports_sx2 = false;
    let mut peer_supports_aich = false;
    let mut peer_sec_ident: u8 = 0;
    let mut peer_udp_port = None;
    let mut peer_udp_version: u8 = 0;

    let handshake_deadline = tokio::time::Instant::now() + timeouts.handshake_timeout;
    while !received_hello_answer {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Handshake aborted by kill switch"));
            }
            _ = tokio::time::sleep_until(handshake_deadline) => {
                return Err(PeerError::new(PeerFailureReason::TimeoutHandshake, "Handshake timed out waiting for OP_HELLO_ANSWER"));
            }
            read_res = stream.read(&mut raw_buf) => {
                match read_res {
                    Ok(0) => {
                        let buffered_len = read_buf.len();
                        let snippet = if buffered_len > 0 {
                            format!("{:02X?}", &read_buf[..buffered_len.min(32)])
                        } else {
                            "none".to_string()
                        };
                        tracing::warn!(
                            "Peer {target_addr} closed connection during handshake (bytes buffered={buffered_len}: {snippet})"
                        );
                        return Err(PeerError::new(PeerFailureReason::ConnectRefused, "Peer closed connection during handshake"));
                    }
                    Ok(n) => {
                        tracing::debug!(
                            "Peer {target_addr}: read {n} bytes during handshake: {:02X?}",
                            &raw_buf[..n.min(32)]
                        );
                        read_buf.extend_from_slice(&raw_buf[..n]);
                        if read_buf.len() > MAX_PEER_READ_BUFFER_SIZE {
                            return Err(PeerError::new(PeerFailureReason::InvalidHandshake, "Peer read buffer exceeded limit during handshake"));
                        }
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            tracing::debug!(
                                "Peer {target_addr}: decoded packet proto=0x{:02X} opcode=0x{:02X} payload_len={}",
                                packet.protocol,
                                packet.opcode,
                                packet.payload.len()
                            );
                            if packet.protocol == PR_ED2K && (packet.opcode == OP_HELLO || packet.opcode == OP_HELLO_ANSWER) {
                                match decoder.decode_hello_answer(packet.protocol, packet.opcode, &packet.payload, 0) {
                                    Ok(ans) => {
                                        let peer_info = mule_protocol::ed2k::Ed2kPeerInfo::from_hello_answer(&ans);
                                        peer_supports_sx = peer_info.supports_source_exchange();
                                        peer_supports_sx2 = peer_info.supports_source_exchange2();
                                        peer_supports_aich = peer_info.supports_aich();
                                        peer_sec_ident = peer_info.secure_ident_support();
                                        peer_udp_port = peer_info.emule_udp_port;
                                        peer_udp_version = peer_info.udp_version();

                                        let mut peer_opts = 0u8;
                                        if peer_info.supports_crypt_layer() { peer_opts |= 0x01; }
                                        if peer_info.requests_crypt_layer() { peer_opts |= 0x02; }
                                        if peer_info.requires_crypt_layer() { peer_opts |= 0x04; }

                                        let hex_user_hash = hex::encode(ans.user_hash);
                                        let updated_src = mule_core::model::PeerSource {
                                            file_hash: file_hash_str.to_string(),
                                            address: address_str.to_string(),
                                            ip: match target_addr {
                                                SocketAddr::V4(v4) => u32::from_le_bytes(v4.ip().octets()),
                                                _ => 0,
                                             },
                                            port: target_addr.port(),
                                            origin: mule_core::model::PeerSourceOrigin::Manual,
                                            discovered_at_unix_ms: connected_ms,
                                            last_seen_unix_ms: connected_ms,
                                            status: mule_core::model::PeerSourceStatus::Candidate,
                                            fail_count: 0,
                                            last_error: None,
                                            cooldown_until_unix_ms: None,
                                            user_hash: Some(hex_user_hash),
                                            crypt_options: peer_opts,
                                        };
                                        let _ = downloads.lock().await.upsert_peer_sources(file_hash_str, &[updated_src]);

                                        for tag in &ans.tags {
                                            if let mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_UDPPORTS) = tag.name {
                                                if let mule_protocol::ed2k::tags::TagValue::UInt32(ports) = tag.value {
                                                    let peer_kad_port = (ports >> 16) as u16;
                                                    if peer_kad_port > 0 {
                                                        tracing::info!("Peer {target_addr} advertises live Kad UDP port {peer_kad_port}");
                                                        if let Some(ref kad) = kad_service {
                                                            let k_clone = Arc::clone(kad);
                                                            let ip_str = target_addr.ip().to_string();
                                                            tokio::spawn(async move {
                                                                let _ = k_clone.bootstrap_from_node(&ip_str, peer_kad_port).await;
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        return Err(PeerError::new(PeerFailureReason::InvalidHandshake, format!("Invalid OP_HELLO_ANSWER: {e}")));
                                    }
                                }
                                received_hello_answer = true;
                                break;
                            }
                        }
                    }
                    Err(e) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Read error during handshake: {e}"))),
                }
            }
        }
    }

    Ok(HandshakeResult {
        stream,
        peer_supports_sx,
        peer_supports_sx2,
        peer_supports_aich,
        peer_sec_ident,
        peer_udp_port,
        peer_udp_version,
        connected_ms,
    })
}
