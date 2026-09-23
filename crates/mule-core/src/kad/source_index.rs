// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use crate::kad::KadId;
use std::net::Ipv4Addr;

/// Represents a source indexed for a target file in the Kad network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KadIndexedSource {
    /// 16-byte target file hash (ED2K hash).
    pub file_hash: [u8; 16],
    /// 16-byte Kad identity of the publishing peer.
    pub source_kad_id: KadId,
    /// IPv4 address of the peer offering the file.
    pub ip: Ipv4Addr,
    /// UDP port of the peer.
    pub udp_port: u16,
    /// TCP client port of the peer for ED2K data transfer.
    pub tcp_port: u16,
    /// Total file size reported by the peer.
    pub file_size: u64,
    /// Encryption and feature flags (e.g. Obfuscation support).
    pub encryption: u8,
    /// Expiration timestamp in milliseconds (TTL).
    pub expires_at_unix_ms: u64,
    /// Timestamp when this record was last refreshed.
    pub updated_at_unix_ms: u64,
}
