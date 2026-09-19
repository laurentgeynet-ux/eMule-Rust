//! Inbound connection handling, obfuscation detection, and handshake logic.

use std::net::SocketAddr;
use std::time::Duration;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::warn;

use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kHelloAnswer, OP_HELLO, PR_ED2K,
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
            warn!("Rejected inbound peer {} by IP filter rule: {}", remote_str, rule.description);
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
            crate::services::obfuscation_stream::ObfuscatedStream::plaintext_with_prefix(socket, vec![b])
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
                    warn!("Inbound obfuscation handshake failed from {}: {}", remote_str, e);
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
                        let _hello = decoder
                            .decode_hello(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                            .map_err(|e| format!("Invalid OP_HELLO payload: {e}"))?;
                        return Ok(());
                    }
                }
            }
        })
        .await;

        match handshake_res {
            Ok(Ok(())) => {
                let mut tags = Vec::new();
                let mut misc2: u32 = 0;
                if self.obfuscation_enabled {
                    misc2 |= (1 << 7) | (1 << 8); // CryptLayerSupported | CryptLayerRequested
                }
                if self.obfuscation_required {
                    misc2 |= 1 << 9; // CryptLayerRequires
                }
                if misc2 > 0 {
                    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
                        name: mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_MISCOPTIONS2),
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

        self.dispatch_requests(stream, remote_str, cancel_token).await;
    }
}
