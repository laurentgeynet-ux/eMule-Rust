// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use std::fmt;

/// A peer source discovered through an eD2K link or source exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerSource {
    pub host: String,
    pub port: u16,
    pub crypt_options: Option<u8>,
    pub client_hash: Option<[u8; 16]>,
}

impl PeerSource {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            crypt_options: None,
            client_hash: None,
        }
    }

    /// Parses a source string item, e.g. "192.168.1.10:39735" or "host.example.com:39735"
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        let parts: Vec<&str> = trimmed.split(':').collect();
        if parts.len() < 2 {
            return None;
        }

        let host = parts[0].to_string();
        if host.is_empty() {
            return None;
        }

        let port = parts[1].parse::<u16>().ok()?;

        let mut client_hash = None;
        if parts.len() >= 3 && parts[2].len() == 32 {
            let mut h = [0u8; 16];
            if hex::decode_to_slice(parts[2], &mut h).is_ok() {
                client_hash = Some(h);
            }
        }

        Some(Self {
            host,
            port,
            crypt_options: None,
            client_hash,
        })
    }
}

impl fmt::Display for PeerSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)?;
        if let Some(hash) = &self.client_hash {
            write!(f, ":{}", hex::encode(hash))?;
        }
        Ok(())
    }
}
