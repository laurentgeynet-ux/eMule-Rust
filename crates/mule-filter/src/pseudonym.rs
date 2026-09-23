// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::RwLock;

/// Deterministic session pseudonymizer for addresses in audit logs and reports.
/// Transforms `198.51.100.23:39735` into `peer-01:39735` or `server-01:39735` without leaking raw IP.
pub struct SessionPseudonymizer {
    peer_map: RwLock<HashMap<String, String>>,
    server_map: RwLock<HashMap<String, String>>,
}

impl Default for SessionPseudonymizer {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionPseudonymizer {
    pub fn new() -> Self {
        Self {
            peer_map: RwLock::new(HashMap::new()),
            server_map: RwLock::new(HashMap::new()),
        }
    }

    /// Returns a deterministic identifier for a peer address, preserving the port.
    /// Example: `peer-01:39735`.
    pub fn pseudonymize_peer(&self, addr: SocketAddr) -> String {
        let ip_str = addr.ip().to_string();
        let mut map = self.peer_map.write().unwrap();
        let next_id = map.len() + 1;
        let prefix = map
            .entry(ip_str)
            .or_insert_with(|| format!("peer-{:02}", next_id));
        format!("{}:{}", prefix, addr.port())
    }

    /// Returns a deterministic identifier for a server address, preserving the port.
    /// Example: `server-01:4661`.
    pub fn pseudonymize_server(&self, addr: SocketAddr) -> String {
        let ip_str = addr.ip().to_string();
        let mut map = self.server_map.write().unwrap();
        let next_id = map.len() + 1;
        let prefix = map
            .entry(ip_str)
            .or_insert_with(|| format!("server-{:02}", next_id));
        format!("{}:{}", prefix, addr.port())
    }

    pub fn clear(&self) {
        self.peer_map.write().unwrap().clear();
        self.server_map.write().unwrap().clear();
    }
}
