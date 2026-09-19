//! Inbound connection handling, obfuscation detection, and handshake logic.

use bytes::BytesMut;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::warn;

use mule_protocol::ed2k::{Ed2kDecoder, Ed2kEncoder, Ed2kHelloAnswer, OP_HELLO, PR_ED2K};
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
                remote_str,
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
            if self.obfuscation_required {
                warn!("Rejected unencrypted inbound connection from {} because obfuscation is required", remote_str);
                return;
            }
            crate::services::obfuscation_stream::ObfuscatedStream::plaintext_with_prefix(
                socket,
                vec![b],
            )
        } else {
            if !self.obfuscation_enabled {
                warn!("Rejected obfuscated inbound connection from {} because obfuscation is disabled", remote_str);
                return;
            }
            match crate::services::obfuscation_stream::ObfuscatedStream::accept_peer_incoming(
                socket,
                b,
                &self.user_hash,
                self.obfuscation_padding_max,
            )
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    warn!(
                        "Inbound obfuscation handshake failed from {}: {}",
                        remote_str, e
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
                callback_hello = Some(hello);
                let mut tags = Vec::new();
                // eMule MiscOptions1 — SecIdent=3 (v1|v2) in bits 16-19 when a
                // usable RSA key is loaded, mirroring the outbound HELLO.
                let sec_ident: u32 = if self.sui_pubkey_der.is_some() { 3 } else { 0 };
                let misc1: u32 = (1 << 29)
                    | (1 << 28)
                    | (4 << 24)
                    | (1 << 20)
                    | ((sec_ident & 0x0F) << 16)
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
                let mut misc2: u32 = 0;
                if self.obfuscation_enabled {
                    misc2 |= (1 << 7) | (1 << 8); // CryptLayerSupported | CryptLayerRequested
                }
                if self.obfuscation_required {
                    misc2 |= 1 << 9; // CryptLayerRequires
                }
                if misc2 > 0 {
                    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
                        name: mule_protocol::ed2k::tags::TagName::Id(
                            mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_MISCOPTIONS2,
                        ),
                        value: mule_protocol::ed2k::tags::TagValue::UInt32(misc2),
                    });
                }

                let ans = Ed2kHelloAnswer {
                    user_hash: self.user_hash,
                    client_id: 0x0100007F,
                    tcp_port: actual_listen_addr.port(),
                    tags,
                    unknown_tags: vec![],
                    server_ip: 0,
                    server_port: 0,
                };
                if let Ok(ans_pkt) = encoder.encode_hello_answer(&ans) {
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
                "Inbound connection from {remote_str} matched pending LowID callback (client_id={lowid}, file={file_hash}) — adopting as download session"
            );
            if ps
                .accept_callback(
                    stream,
                    remote_addr,
                    &file_hash,
                    source_address,
                    peer_info.supports_source_exchange(),
                    peer_info.supports_source_exchange2(),
                    peer_info.supports_aich(),
                    peer_info.secure_ident_support(),
                    self.downloads.clone(),
                    cancel_token.clone(),
                )
                .await
                .is_ok()
            {
                return;
            }
            // Adoption refused (limits/dedup): fall through to normal serving.
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

        self.dispatch_requests(
            stream,
            remote_str,
            actual_listen_addr.ip(),
            cancel_token,
            peer_supports_compression,
            peer_user_hash,
        )
        .await;
    }
}
