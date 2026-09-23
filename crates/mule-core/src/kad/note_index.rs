// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Kad notes index model (file comments / ratings, storage-node role).

/// A note entry indexed for a target file, mirroring eMule `CEntry` with
/// `m_bSource = false`. Tags are kept as their wire-encoded blob
/// (`[tag_count:u8][tags]`) so arbitrary/unknown tags survive a restart;
/// the protocol layer re-decodes them on hydrate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KadIndexedNote {
    /// 32-char lowercase hex of the target file hash.
    pub file_hash: String,
    /// 32-char lowercase hex of the publisher's Kad ID (`m_uSourceID`).
    pub source_id: String,
    /// Publisher IPv4 (dotted string).
    pub publisher_ip: String,
    /// Publisher Kad UDP port.
    pub publisher_udp_port: u16,
    /// Wire-encoded tag list blob (bounded, display-only data).
    pub tags_blob: Vec<u8>,
    /// Timestamp when first stored.
    pub discovered_at_unix_ms: i64,
    /// Expiration timestamp (TTL ~5h, like eMule `KADEMLIAREPUBLISHTIMEN`).
    pub expires_at_unix_ms: i64,
}
