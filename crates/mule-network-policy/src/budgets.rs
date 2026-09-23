use serde::{Deserialize, Serialize};

/// Resource limits and timeouts enforcing bounded networking operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceBudgets {
    /// Maximum concurrent peer TCP connections (default: 200).
    pub max_active_peer_connections: usize,
    /// Maximum concurrent server TCP connections (default: 2).
    pub max_active_server_connections: usize,
    /// Maximum concurrent listener sockets (default: 4 — inbound TCP,
    /// server UDP and Kad UDP each consume one slot).
    pub max_active_listeners: usize,
    /// Timeout in milliseconds for initiating a TCP connection (default: 10,000 ms).
    pub connect_timeout_ms: u64,
    /// Timeout in milliseconds for protocol handshake / hello exchange (default: 15,000 ms).
    pub handshake_timeout_ms: u64,
    /// Maximum single frame / message payload size in bytes (default: 10 MiB).
    pub max_frame_size_bytes: usize,
    /// Maximum aggregate buffer memory allocated across active network sockets (default: 32 MiB).
    pub max_total_buffer_bytes: usize,
    /// Rate limit on connection attempts per rolling minute (default: 240 —
    /// eMule 0.72a `MaxConnectionsPerFiveSeconds` = 20 ≈ 240/min).
    pub rate_limit_per_min: u32,
}

impl Default for ResourceBudgets {
    fn default() -> Self {
        Self {
            max_active_peer_connections: 500,
            max_active_server_connections: 2,
            max_active_listeners: 4,
            connect_timeout_ms: 10_000,
            handshake_timeout_ms: 15_000,
            max_frame_size_bytes: 10 * 1024 * 1024,   // 10 MiB
            max_total_buffer_bytes: 32 * 1024 * 1024, // 32 MiB
            rate_limit_per_min: 240,
        }
    }
}

/// Borne de sanité pour les connexions pairs simultanées — eMule n'impose
/// aucun plafond fixe (`GetMaxWindowsTCPConnections` = `UNLIMITED` sous
/// Windows NT+), on garde un garde-fou contre les fautes de frappe.
/// En pratique la limite réelle reste `max_total_buffer_bytes`.
pub const MAX_PEER_CONNECTIONS_CAP: usize = 65_000;

impl ResourceBudgets {
    /// Validates that configured budgets stay within safe upper and lower bounds.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.max_active_peer_connections == 0 {
            return Err("max_active_peer_connections must be at least 1");
        }
        if self.max_active_peer_connections > MAX_PEER_CONNECTIONS_CAP {
            return Err("max_active_peer_connections cannot exceed 65000");
        }
        if self.max_active_server_connections == 0 {
            return Err("max_active_server_connections must be at least 1");
        }
        if self.max_active_server_connections > 10 {
            return Err("max_active_server_connections cannot exceed 10");
        }
        if self.connect_timeout_ms < 500 || self.connect_timeout_ms > 120_000 {
            return Err("connect_timeout_ms must be between 500 ms and 120,000 ms");
        }
        if self.handshake_timeout_ms < 500 || self.handshake_timeout_ms > 120_000 {
            return Err("handshake_timeout_ms must be between 500 ms and 120,000 ms");
        }
        if self.max_frame_size_bytes < 1024 || self.max_frame_size_bytes > 64 * 1024 * 1024 {
            return Err("max_frame_size_bytes must be between 1 KiB and 64 MiB");
        }
        if self.max_total_buffer_bytes < self.max_frame_size_bytes {
            return Err("max_total_buffer_bytes must be at least max_frame_size_bytes");
        }
        if self.rate_limit_per_min == 0 || self.rate_limit_per_min > 1200 {
            return Err("rate_limit_per_min must be between 1 and 1200");
        }
        Ok(())
    }
}
