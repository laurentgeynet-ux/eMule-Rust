// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::BytesMut;
use mule_protocol::lab::*;
use mule_protocol::{PacketCodec, PeerId};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tracing::{debug, info};

pub struct LabSourceService {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    server_handle: JoinHandle<()>,
}

impl LabSourceService {
    pub async fn start(engine: LabSourceEngine) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        let server_handle = tokio::spawn(async move {
            info!(endpoint = %addr, "LabSourceService listening on TCP loopback");
            while !shutdown_clone.load(Ordering::Relaxed) {
                match listener.accept().await {
                    Ok((socket, peer_addr)) => {
                        let engine = engine.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_client(socket, engine).await {
                                debug!(peer = %crate::services::peer_session::log_peer_addr(peer_addr), error = %e, "Lab source client session closed");
                            }
                        });
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            addr,
            shutdown,
            server_handle,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.server_handle.abort();
    }
}

impl Drop for LabSourceService {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn handle_client(
    mut socket: TcpStream,
    engine: LabSourceEngine,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let codec = PacketCodec::new();
    let mut read_buf = BytesMut::new();

    let server_peer_id = PeerId::new([0xBB; 16]);
    let mut sm = PeerStateMachine::new_listener(server_peer_id, 1);

    // 1. Handshake: wait for Hello
    let hello = read_msg(&mut socket, &codec, &mut read_buf).await?;
    let actions = sm.on_message(hello)?;
    for action in actions {
        if let PeerAction::SendMessage(msg) = action {
            send_msg(&mut socket, &codec, &msg).await?;
        }
    }

    // 2. Offer all hosted files
    for hash in engine.hosted_hashes() {
        if let Some(offer) = engine.offer_file(&hash) {
            send_msg(&mut socket, &codec, &offer).await?;
        }
    }

    // 3. Serve block requests
    loop {
        let msg = match read_msg(&mut socket, &codec, &mut read_buf).await {
            Ok(m) => m,
            Err(_) => break, // Client disconnected
        };

        match msg {
            LabMessage::RequestBlock {
                file_hash,
                start_offset,
                length,
            } => {
                let resp = engine.serve_block(&file_hash, start_offset, length);
                send_msg(&mut socket, &codec, &resp).await?;
            }
            LabMessage::Goodbye { .. } => {
                let _ = sm.on_message(msg);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn send_msg(
    stream: &mut TcpStream,
    codec: &PacketCodec,
    msg: &LabMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let raw = msg.to_raw_packet()?;
    let mut buf = BytesMut::new();
    codec.encode(&raw, &mut buf)?;
    stream.write_all(&buf).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_msg(
    stream: &mut TcpStream,
    codec: &PacketCodec,
    buf: &mut BytesMut,
) -> Result<LabMessage, Box<dyn std::error::Error + Send + Sync>> {
    loop {
        if let Some(packet) = codec.decode(buf)? {
            return Ok(LabMessage::from_raw_packet(&packet)?);
        }
        let mut chunk = [0u8; 8192];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err("Unexpected EOF on socket".into());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}
