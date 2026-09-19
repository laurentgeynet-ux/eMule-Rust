use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, Mutex};

use mule_protocol::kad::KadMessage;

/// Transaction timeout in seconds.
pub const KAD_TRANSACTION_TIMEOUT_SECS: u64 = 3;

/// Maximum number of concurrent in-flight transactions to prevent memory exhaustion.
pub const MAX_IN_FLIGHT_TRANSACTIONS: usize = 200;

/// Maximum requests per second allowed per source IP (anti-amplification / rate limiting).
pub const MAX_KAD_REQUESTS_PER_SEC_PER_IP: u32 = 20;

/// Simple per-IP sliding rate limiter.
#[derive(Debug, Default)]
pub struct IpRateLimiter {
    counts: HashMap<IpAddr, (u32, Instant)>,
}

impl IpRateLimiter {
    pub fn new() -> Self {
        Self { counts: HashMap::new() }
    }

    pub fn check_and_record(&mut self, ip: IpAddr) -> bool {
        let now = Instant::now();
        if self.counts.len() > 1000 {
            self.counts.retain(|_, (_, ts)| now.duration_since(*ts) < Duration::from_secs(10));
        }

        let entry = self.counts.entry(ip).or_insert((0, now));
        if now.duration_since(entry.1) >= Duration::from_secs(1) {
            *entry = (1, now);
            true
        } else if entry.0 < MAX_KAD_REQUESTS_PER_SEC_PER_IP {
            entry.0 += 1;
            true
        } else {
            false
        }
    }
}

/// Expected response variant for a pending in-flight transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KadExpectedResponse {
    Pong,
    HelloRes,
    BootstrapRes,
    FindNodeRes,
    SearchSourceRes,
    SearchKeywordRes,
    PublishSourceRes,
    PublishKeywordRes,
}

/// Pending in-flight transaction state.
pub struct InFlightTransaction {
    pub target: SocketAddr,
    pub expected_response: KadExpectedResponse,
    pub expected_target_hash: Option<[u8; 16]>,
    pub _created_at: Instant,
    pub sender: oneshot::Sender<Result<KadMessage, String>>,
}

/// Statistics snapshot of the UDP Kad service.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KadUdpSnapshot {
    pub is_active: bool,
    pub bind_address: Option<String>,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub active_transactions: usize,
}

pub type InFlightMap = Arc<Mutex<HashMap<u32, InFlightTransaction>>>;
