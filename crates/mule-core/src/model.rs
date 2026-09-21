use mule_format::{AichHash, Ed2kHash};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    Queued,
    WaitingForSources,
    Downloading,
    Verifying,
    Paused,
    Completing,
    Completed,
    Failed,
    Removed,
}

impl DownloadStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::WaitingForSources => "waiting_for_sources",
            Self::Downloading => "downloading",
            Self::Verifying => "verifying",
            Self::Paused => "paused",
            Self::Completing => "completing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Removed => "removed",
        }
    }
}

impl std::str::FromStr for DownloadStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "waiting_for_sources" => Self::WaitingForSources,
            "downloading" => Self::Downloading,
            "verifying" => Self::Verifying,
            "paused" => Self::Paused,
            "completing" => Self::Completing,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "removed" => Self::Removed,
            _ => Self::Queued,
        })
    }
}

/// In-memory representation of an active download task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub id: Uuid,
    pub display_name: String,
    pub total_size: u64,
    pub ed2k_hash: Ed2kHash,
    pub aich_root: Option<AichHash>,
    pub target_path: PathBuf,
    pub temp_path: PathBuf,
    pub status: DownloadStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub category_id: Option<String>,
    /// Download priority: "low", "normal", "high" (eMule file priority —
    /// biases the swarm allocator slot quota for this file).
    pub priority: String,
}

impl Download {
    pub fn new(
        display_name: impl Into<String>,
        total_size: u64,
        ed2k_hash: Ed2kHash,
        aich_root: Option<AichHash>,
        target_path: PathBuf,
        temp_path: PathBuf,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            id: Uuid::new_v4(),
            display_name: display_name.into(),
            total_size,
            ed2k_hash,
            aich_root,
            target_path,
            temp_path,
            status: DownloadStatus::Queued,
            created_at: now,
            updated_at: now,
            category_id: None,
            priority: "normal".to_string(),
        }
    }
}

/// Category metadata for organizing local downloads.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadCategory {
    pub id: String,
    pub name: String,
    pub color: String, // Hex "#RRGGBB"
    pub sort_order: i32,
    pub default_destination: Option<String>,
    pub default_priority: Option<String>, // "low", "normal", "high"
    pub created_at: i64,
    pub updated_at: i64,
}

impl DownloadCategory {
    pub fn new(
        name: impl Into<String>,
        color: impl Into<String>,
        sort_order: i32,
        default_destination: Option<String>,
        default_priority: Option<String>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            color: color.into(),
            sort_order,
            default_destination,
            default_priority,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Single historical event entry for a download transition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub download_id: String,
    pub file_name: String,
    pub total_size: u64,
    pub transferred_bytes: u64,
    pub ed2k_hash: String,
    pub event_type: String, // "completed", "failed", "removed", "verified_pending_promotion", "resumed"
    pub status: String,
    pub duration_secs: u64,
    pub connection_attempts: u32,
    pub resume_count: u32,
    pub verification_result: String, // "verified_ok", "hash_mismatch", "unverified", "skipped"
    pub error_message: Option<String>,
    pub timestamp_unix_ms: i64,
}

/// Query filter for history list.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFilter {
    pub event_type: Option<String>,
    pub status: Option<String>,
    pub from_timestamp_ms: Option<i64>,
    pub to_timestamp_ms: Option<i64>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Aggregated history and transfer statistics.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStats {
    pub total_completed_count: u64,
    pub total_completed_bytes: u64,
    pub total_failed_count: u64,
    pub total_resumes_count: u64,
    pub total_integrity_failures: u64,
    // Time windows
    pub session_completed_bytes: u64,
    pub session_completed_count: u64,
    pub last_24h_completed_bytes: u64,
    pub last_24h_completed_count: u64,
    pub last_7d_completed_bytes: u64,
    pub last_7d_completed_count: u64,
}

/// Filter status for an eD2K server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerFilterStatus {
    Allowed,
    BlockedByFilter,
    Disabled,
}

impl ServerFilterStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::BlockedByFilter => "blocked_by_filter",
            Self::Disabled => "disabled",
        }
    }
}

impl std::str::FromStr for ServerFilterStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "blocked_by_filter" => Self::BlockedByFilter,
            "disabled" => Self::Disabled,
            _ => Self::Allowed,
        })
    }
}

/// Persistent record of an eD2K server in local storage.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Ed2kServer {
    pub address: String, // "ip:port" as primary key
    pub ip: String,
    pub port: u16,
    pub name: String,
    pub description: String,
    pub ping: u32,
    pub preference: u32,
    pub failed_count: u32,
    pub dyn_ip: Option<String>,
    pub version: Option<String>,
    pub filter_status: ServerFilterStatus,
    pub last_seen_unix_ms: Option<i64>,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
}

/// Status of a discovered or contacted peer source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerSourceStatus {
    Candidate,
    Allowed,
    Filtered,
    Rejected,
    Tried,
    Connected,
    Failed,
}

impl PeerSourceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Allowed => "allowed",
            Self::Filtered => "filtered",
            Self::Rejected => "rejected",
            Self::Tried => "tried",
            Self::Connected => "connected",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for PeerSourceStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "allowed" => Self::Allowed,
            "filtered" => Self::Filtered,
            "rejected" => Self::Rejected,
            "tried" => Self::Tried,
            "connected" => Self::Connected,
            "failed" => Self::Failed,
            _ => Self::Candidate,
        })
    }
}

/// Origin / discovery vector of a peer source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerSourceOrigin {
    Server,
    Search,
    Manual,
    Kad,
    SourceExchange,
}

impl PeerSourceOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Search => "search",
            Self::Manual => "manual",
            Self::Kad => "kad",
            Self::SourceExchange => "source_exchange",
        }
    }
}

impl std::str::FromStr for PeerSourceOrigin {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "search" => Self::Search,
            "manual" => Self::Manual,
            "kad" => Self::Kad,
            "source_exchange" | "sx" => Self::SourceExchange,
            _ => Self::Server,
        })
    }
}

/// Normalized failure reasons for peer connection attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerFailureReason {
    IpFilter,
    NetworkPolicy,
    BudgetExhausted,
    ConnectRefused,
    TimeoutConnect,
    TimeoutHandshake,
    TimeoutStatus,
    TimeoutQueue,
    TimeoutFirstData,
    TimeoutInactivity,
    InvalidHandshake,
    HashMismatch,
    InvalidRange,
    CorruptedData,
    FileNotFound,
    /// The peer shares the file but none of its advertised parts is needed
    /// (eMule "No Needed Parts" source state — kept on a long cooldown).
    NoNeededParts,
    KillSwitchAborted,
    MissingUserHashForObfuscation,
    ObfuscationNegotiationFailed,
}

impl PeerFailureReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::IpFilter => "ip_filter",
            Self::NetworkPolicy => "network_policy",
            Self::BudgetExhausted => "budget_exhausted",
            Self::ConnectRefused => "connect_refused",
            Self::TimeoutConnect => "timeout_connect",
            Self::TimeoutHandshake => "timeout_handshake",
            Self::TimeoutStatus => "timeout_status",
            Self::TimeoutQueue => "timeout_queue",
            Self::TimeoutFirstData => "timeout_first_data",
            Self::TimeoutInactivity => "timeout_inactivity",
            Self::InvalidHandshake => "invalid_handshake",
            Self::HashMismatch => "hash_mismatch",
            Self::InvalidRange => "invalid_range",
            Self::CorruptedData => "corrupted_data",
            Self::FileNotFound => "file_not_found",
            Self::NoNeededParts => "no_needed_parts",
            Self::KillSwitchAborted => "kill_switch_aborted",
            Self::MissingUserHashForObfuscation => "missing_user_hash_for_obfuscation",
            Self::ObfuscationNegotiationFailed => "obfuscation_negotiation_failed",
        }
    }
}

/// Persistent record of an eD2K peer source in local storage.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PeerSource {
    pub file_hash: String,
    pub address: String, // "ip:port"
    pub ip: u32,
    pub port: u16,
    pub origin: PeerSourceOrigin,
    pub discovered_at_unix_ms: i64,
    pub last_seen_unix_ms: i64,
    pub status: PeerSourceStatus,
    pub fail_count: u32,
    pub last_error: Option<String>,
    pub cooldown_until_unix_ms: Option<i64>,
    #[serde(default)]
    pub user_hash: Option<String>,
    #[serde(default)]
    pub crypt_options: u8,
    /// Kad buddy ID (hex) for LowID sources reachable only through a buddy
    /// callback (`KADEMLIA_CALLBACK_REQ` relayed by this buddy node).
    #[serde(default)]
    pub buddy_id: Option<String>,
    /// Client software + version learned from the eD2K/eMule hello
    /// (e.g. "eMule 0.70b", "aMule 3.0b").
    #[serde(default)]
    pub software: Option<String>,
    /// Per-part availability bitmask received via `OP_FILESTATUS`
    /// (`ceil(part_count/8)` bytes, bit set = source has the part).
    #[serde(default)]
    pub availability: Option<Vec<u8>>,
}

impl PeerSource {
    /// Returns the 16-byte decoded user hash if valid hex.
    pub fn user_hash_bytes(&self) -> Option<[u8; 16]> {
        self.user_hash.as_deref().and_then(|s| {
            if s.len() != 32 {
                return None;
            }
            let mut bytes = [0u8; 16];
            for i in 0..16 {
                bytes[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
            }
            Some(bytes)
        })
    }

    /// Whether this peer supports protocol obfuscation (bit 0).
    pub fn supports_crypt(&self) -> bool {
        (self.crypt_options & 0x01) != 0
    }

    /// Whether this peer requests protocol obfuscation (bit 1).
    pub fn requests_crypt(&self) -> bool {
        (self.crypt_options & 0x02) != 0
    }

    /// Whether this peer strictly requires protocol obfuscation (bit 2).
    pub fn requires_crypt(&self) -> bool {
        (self.crypt_options & 0x04) != 0
    }

    /// Returns the 16-byte decoded buddy Kad ID if valid hex.
    pub fn buddy_id_bytes(&self) -> Option<[u8; 16]> {
        self.buddy_id.as_deref().and_then(|s| {
            if s.len() != 32 {
                return None;
            }
            let mut bytes = [0u8; 16];
            for i in 0..16 {
                bytes[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
            }
            Some(bytes)
        })
    }
}

/// Explicitly approved shared file available for inbound eD2K upload.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SharedFile {
    pub file_hash: String,
    pub file_name: String,
    pub file_size: u64,
    pub file_path: String,
    /// Cached AICH root hash (SHA-1) of the file content, computed on first
    /// `OP_AICHFILEHASHREQ` so it can be served to peers without rehashing.
    #[serde(default)]
    pub aich_root: Option<[u8; 20]>,
    pub shared_at_unix_ms: i64,
    pub enabled: bool,
}

impl SharedFile {
    pub fn new(
        file_hash: impl Into<String>,
        file_name: impl Into<String>,
        file_size: u64,
        file_path: impl Into<String>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Self {
            file_hash: file_hash.into(),
            file_name: file_name.into(),
            file_size,
            file_path: file_path.into(),
            aich_root: None,
            shared_at_unix_ms: now,
            enabled: true,
        }
    }
}

/// Persistent record of credits granted to / earned from a cryptographically verified peer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PeerCredit {
    /// SHA-256 or MD4 hash of the peer's public key (primary key).
    pub pubkey_hash: String,
    /// Raw public key bytes (hex encoded).
    pub pubkey_hex: String,
    /// Total bytes uploaded to this peer by us.
    pub uploaded_bytes: u64,
    /// Total bytes downloaded from this peer by us.
    pub downloaded_bytes: u64,
    /// Timestamp of first authentication.
    pub first_seen_unix_ms: i64,
    /// Timestamp of most recent transfer or authentication.
    pub last_seen_unix_ms: i64,
}

impl PeerCredit {
    pub fn new(pubkey_hash: impl Into<String>, pubkey_hex: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Self {
            pubkey_hash: pubkey_hash.into(),
            pubkey_hex: pubkey_hex.into(),
            uploaded_bytes: 0,
            downloaded_bytes: 0,
            first_seen_unix_ms: now,
            last_seen_unix_ms: now,
        }
    }

    /// Computes the eMule canonical credit multiplier (clamped between 1.0 and 10.0).
    ///
    /// Multiplier formula:
    /// min(10.0, max(1.0, 2.0 * (downloaded_bytes) / (uploaded_bytes + 1)))
    ///
    /// Semantic definitions:
    /// - `downloaded_bytes`: Bytes received by us from this peer (peer uploaded to us).
    /// - `uploaded_bytes`: Bytes sent by us to this peer.
    pub fn credit_multiplier(&self) -> f64 {
        if self.downloaded_bytes == 0 {
            return 1.0;
        }
        let ratio = (2.0 * self.downloaded_bytes as f64) / (self.uploaded_bytes as f64 + 1.0);
        ratio.clamp(1.0, 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credit_multiplier_canonical_vectors() {
        // Vector 1: Received 0, Sent 0 => 1.0
        let mut credit = PeerCredit::new("hash1", "pub1");
        credit.downloaded_bytes = 0;
        credit.uploaded_bytes = 0;
        assert_eq!(
            credit.credit_multiplier(),
            1.0,
            "Expected floor 1.0 for 0/0"
        );

        // Vector 2: Received 5,000, Sent 0 => 10.0 (capped at 10.0)
        // 2 * 5000 / (0 + 1) = 10000 -> clamped to 10.0
        credit.downloaded_bytes = 5000;
        credit.uploaded_bytes = 0;
        assert_eq!(
            credit.credit_multiplier(),
            10.0,
            "Expected cap 10.0 for 5000/0"
        );

        // Vector 3: Received 1,000, Sent 2,000 => 1.0 (floor)
        // 2 * 1000 / (2000 + 1) = 2000 / 2001 < 1.0 -> clamped to 1.0
        credit.downloaded_bytes = 1000;
        credit.uploaded_bytes = 2000;
        assert_eq!(
            credit.credit_multiplier(),
            1.0,
            "Expected floor 1.0 for 1000/2000"
        );

        // Vector 4: Intermediate ratio (e.g. Received 3,000, Sent 1,000 => ~5.99)
        // 2 * 3000 / (1000 + 1) = 6000 / 1001 = 5.994...
        credit.downloaded_bytes = 3000;
        credit.uploaded_bytes = 999;
        assert!((credit.credit_multiplier() - 6.0).abs() < 0.01);
    }
}

/// Persisted friend identity (eMule `friends.met`): a user hash that is
/// granted a dedicated upload slot regardless of queue pressure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    /// eD2K user hash (hex, lowercase) of the friend.
    pub user_hash: String,
    /// Display name.
    pub name: String,
    /// Timestamp when the friend was added (unix ms).
    pub added_at_unix_ms: i64,
}

/// Secure user identification state for a peer session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecureIdentState {
    None,
    Requested,
    Verified,
    Failed,
}
