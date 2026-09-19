use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::time::SystemTime;

use crate::profile::NetworkProfile;

/// Decision outcome for an audited network operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuditDecision {
    Allowed,
    Denied,
}

/// Category of network event recorded in the audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuditEventType {
    ConnectAttempt,
    ListenAttempt,
    ProfileChanged,
    KillSwitchTriggered,
    NetworkEnabled,
    BudgetExhausted,
    FilterBlocked,
}

/// A single redacted, privacy-preserving network audit event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkAuditEntry {
    pub id: u64,
    pub timestamp_epoch_ms: u64,
    pub event_type: AuditEventType,
    pub redacted_target: Option<String>,
    pub pseudonymized_target: Option<String>,
    pub profile: NetworkProfile,
    pub decision: AuditDecision,
    pub reason: Option<String>,
}

/// Redacts an IP socket address to protect private topology and user identifiers.
pub fn redact_socket_addr(addr: &SocketAddr) -> String {
    let port = addr.port();
    match addr.ip() {
        IpAddr::V4(v4) => {
            if v4.is_loopback() {
                format!("{}:{}", v4, port)
            } else {
                let octets = v4.octets();
                format!("{}.{}.*.*:{}", octets[0], octets[1], port)
            }
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback() {
                format!("[{}]:{}", v6, port)
            } else {
                let segs = v6.segments();
                format!("[{:x}:{:x}:*:*:*:*:*:*]:{}", segs[0], segs[1], port)
            }
        }
    }
}

/// In-memory bounded circular audit log.
#[derive(Debug)]
pub struct NetworkAuditLog {
    max_entries: usize,
    next_id: u64,
    entries: VecDeque<NetworkAuditEntry>,
}

impl NetworkAuditLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries: max_entries.max(10),
            next_id: 1,
            entries: VecDeque::with_capacity(max_entries.min(500)),
        }
    }

    pub fn record(
        &mut self,
        event_type: AuditEventType,
        target: Option<&SocketAddr>,
        pseudonym: Option<String>,
        profile: NetworkProfile,
        decision: AuditDecision,
        reason: Option<String>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        let timestamp_epoch_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let redacted_target = target.map(redact_socket_addr);

        let entry = NetworkAuditEntry {
            id,
            timestamp_epoch_ms,
            event_type,
            redacted_target,
            pseudonymized_target: pseudonym,
            profile,
            decision,
            reason,
        };

        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        id
    }

    /// Returns the most recent entries, up to `limit`.
    pub fn recent(&self, limit: usize) -> Vec<NetworkAuditEntry> {
        let take_count = limit.min(self.entries.len());
        self.entries
            .iter()
            .rev()
            .take(take_count)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
