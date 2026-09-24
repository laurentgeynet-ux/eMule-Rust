// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Helpers d'E/S loopback lab : encodage/décodage `LabMessage` via `PacketCodec`.

use bytes::BytesMut;
use mule_protocol::lab::LabMessage;
use mule_protocol::PacketCodec;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub(crate) async fn send_msg(
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

pub(crate) async fn read_msg(
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
