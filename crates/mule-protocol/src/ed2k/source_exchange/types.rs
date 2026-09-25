// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Data structures and constant definitions for eMule Source Exchange (SX/SX2).

/// Default version of Source Exchange 2 implemented by modern eMule clients.
pub const SOURCE_EXCHANGE2_VERSION: u8 = 4;

/// Maximum version advertised by eMule through the SX1 (`ET_SOURCEEXCHANGE`)
/// hello tag — eMule 0.72a sends `CTag(ET_SOURCEEXCHANGE, 3)`
/// (`BaseClient.cpp:715`). SX1 v3+ entries carry the user hash and use the
/// little-endian "hybrid" IP format.
pub const SOURCE_EXCHANGE1_MAX_VERSION: u8 = 3;

/// Maximum number of sources allowed in a single SX response packet to prevent DoS.
pub const MAX_SX_SOURCES_PER_PACKET: usize = 1000;

/// Request for sources of a given file hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kRequestSources {
    /// 16-byte MD4 hash of the requested file.
    pub file_hash: [u8; 16],
    /// Requested SX version (e.g. 4 for SX2, or 1 for legacy).
    pub version: u8,
    /// Requested SX options (reserved, typically 0).
    pub options: u16,
}

impl Ed2kRequestSources {
    /// Creates a standard SX2 source request for a file hash.
    pub fn new_v2(file_hash: [u8; 16]) -> Self {
        Self {
            file_hash,
            version: SOURCE_EXCHANGE2_VERSION,
            options: 0,
        }
    }

    /// Creates a legacy SX1 source request for a file hash.
    pub fn new_v1(file_hash: [u8; 16]) -> Self {
        Self {
            file_hash,
            version: 1,
            options: 0,
        }
    }
}

/// Representation of a single peer source transmitted over Source Exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SxSource {
    /// IPv4 address as 4 raw octets [A, B, C, D] in standard network order.
    pub ip_bytes: [u8; 4],
    /// Little-endian integer representation of IPv4 address matching the eD2K standard.
    pub ip: u32,
    /// Client TCP listening port.
    pub port: u16,
    /// Connected server IPv4 address as u32.
    pub server_ip: u32,
    /// Connected server port.
    pub server_port: u16,
    /// Remote user hash MD4, if available (SX v2+).
    pub user_hash: Option<[u8; 16]>,
    /// Protocol obfuscation and encryption options (SX v4):
    /// - bit 0: supports protocol encryption layer
    /// - bit 1: requests protocol encryption layer
    /// - bit 2: requires protocol encryption layer
    pub crypt_options: u8,
}

impl SxSource {
    /// Creates a source entry with IPv4 octets, port, user hash, and crypt options.
    pub fn new(
        ip_bytes: [u8; 4],
        port: u16,
        user_hash: Option<[u8; 16]>,
        crypt_options: u8,
    ) -> Self {
        let ip = u32::from_le_bytes(ip_bytes);
        Self {
            ip_bytes,
            ip,
            port,
            server_ip: 0,
            server_port: 0,
            user_hash,
            crypt_options,
        }
    }

    /// Returns the IPv4 address string (e.g. "192.168.1.1").
    pub fn ip_string(&self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.ip_bytes[0], self.ip_bytes[1], self.ip_bytes[2], self.ip_bytes[3]
        )
    }

    /// Returns the socket address string (e.g. "192.168.1.1:49662").
    pub fn address_string(&self) -> String {
        format!("{}:{}", self.ip_string(), self.port)
    }

    /// Returns true if this source is a LowID (< 16_777_216).
    pub fn is_low_id(&self) -> bool {
        self.ip < 16_777_216
    }
}

/// Response containing sources for a given file hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kAnswerSources {
    /// SX version of the packet (e.g. 4 for SX2 v4, or 1..3).
    pub version: u8,
    /// 16-byte MD4 hash of the file.
    pub file_hash: [u8; 16],
    /// Discovered or shared peer sources.
    pub sources: Vec<SxSource>,
}

impl Ed2kAnswerSources {
    /// Creates a new SX2 answer payload.
    pub fn new(file_hash: [u8; 16], sources: Vec<SxSource>) -> Self {
        Self {
            version: SOURCE_EXCHANGE2_VERSION,
            file_hash,
            sources,
        }
    }

    /// Creates an answer payload with an explicit SX version, matching eMule
    /// `byUsedVersion = min(byRequestedVersion, SOURCEEXCHANGE2_VERSION)` for
    /// SX2 or `GetSourceExchange1Version()` for SX1 (`PartFile.cpp:3631-3646`).
    pub fn new_v(version: u8, file_hash: [u8; 16], sources: Vec<SxSource>) -> Self {
        Self {
            version,
            file_hash,
            sources,
        }
    }
}
