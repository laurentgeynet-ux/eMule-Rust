use std::net::Ipv4Addr;
use serde::{Deserialize, Serialize};
use super::kad_id::KadId;

/// Represents a known Kad contact/node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KadContact {
    pub kad_id: KadId,
    pub ip: Ipv4Addr,
    pub udp_port: u16,
    pub tcp_port: Option<u16>,
    pub version: u8,
    pub verified: bool,
    pub fail_count: u32,
    pub first_seen_unix_ms: u64,
    pub last_seen_unix_ms: u64,
}

impl KadContact {
    pub fn new(
        kad_id: KadId,
        ip: Ipv4Addr,
        udp_port: u16,
        tcp_port: Option<u16>,
        version: u8,
        now_ms: u64,
    ) -> Self {
        Self {
            kad_id,
            ip,
            udp_port,
            tcp_port,
            version,
            verified: false,
            fail_count: 0,
            first_seen_unix_ms: now_ms,
            last_seen_unix_ms: now_ms,
        }
    }

    pub fn mark_seen(&mut self, now_ms: u64) {
        self.last_seen_unix_ms = now_ms;
        self.fail_count = 0;
        self.verified = true;
    }

    pub fn mark_failed(&mut self) {
        self.fail_count = self.fail_count.saturating_add(1);
    }
}
