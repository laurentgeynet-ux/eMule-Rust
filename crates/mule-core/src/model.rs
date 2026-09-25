// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

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
    /// Per-file source limit imported from `.part.met` `FT_MAXSOURCES`
    /// (eMule `CPartFile::GetMaxSources`). `None` = use the global limit.
    pub max_sources: Option<u32>,
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
            max_sources: None,
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

    /// Sémantique eMule `srclist` : une source dont la connexion échoue,
    /// qui répond « fichier absent » ou qui est bloquée par le filtre IP
    /// sort de la liste affichée et part en deadlist — cf.
    /// `CUpDownClient::Disconnected` (`RemoveSource` + `AddDeadSource`).
    /// Les entrées `Failed`/`Rejected`/`Filtered` restent persistées pour
    /// le backoff de re-essai, mais ne sont ni affichées ni comptées dans
    /// `GetSourceCount()`.
    pub fn is_srclist_visible(&self) -> bool {
        !matches!(self, Self::Failed | Self::Rejected | Self::Filtered)
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
    /// eD2K server (IPv4, little-endian u32) the source is connected to —
    /// eMule `m_dwServerIP`. Decoded from source-exchange entries and filled
    /// with the answering server for `OP_FOUNDSOURCES` results. 0 = unknown.
    /// LowIDs are only unique per server: a `lowid:` source is callable via
    /// `OP_CALLBACKREQUEST` only when this matches our connected server
    /// (eMule `CServerConnect::IsLocalServer`).
    #[serde(default)]
    pub server_ip: u32,
    /// eD2K server port of the source — eMule `m_nServerPort`. 0 = unknown.
    #[serde(default)]
    pub server_port: u16,
    /// Peer's Kad UDP port — eMule `GetKadPort()`. Carried by Kad search
    /// source entries (`KadSourceWire.udp_port`); required to send an
    /// `OP_DIRECTCALLBACKREQ` to a TCP-firewalled/UDP-open source
    /// (`BaseClient.cpp:1414`). 0 = unknown.
    #[serde(default)]
    pub udp_port: u16,
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
    /// Upload priority (eMule `PR_*` of the shared file): "verylow", "low",
    /// "normal", "high", "veryhigh" (= powershare). Used as a score factor
    /// in the upload queue (`GetFilePrioAsNumber` semantics).
    #[serde(default = "default_up_priority")]
    pub up_priority: String,
    /// `true` when this entry is an in-progress download (eMule `CPartFile`):
    /// only `completed_parts` may be served to peers. Transient — never
    /// persisted in the `shared_files` table.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_partial: bool,
    /// Indices of fully-complete eD2K parts (9 728 000 B) for a partial
    /// share; empty for regular shared files (all parts available).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub completed_parts: Vec<u16>,
    /// Total cumulatif des octets uploadés pour ce fichier — persisté dans
    /// `share_upload_stats`, renseigné par `shares.list` (eMule
    /// `CKnownFile::statistic.GetTransferred()`).
    #[serde(default)]
    pub uploaded_bytes: u64,
    /// Débit d'upload courant agrégé sur les sessions actives de ce
    /// fichier (B/s) — mesuré, 0 quand aucun slot actif.
    #[serde(default)]
    pub upload_rate_bps: u64,
    /// Catégorie associée (`categories.id`) — `None` = non classé. Pour
    /// les partages synthétisés depuis un download (partiel ou complété
    /// en DB), reflète `downloads.category_id` ; pour les partages
    /// autonomes, persisté dans `shared_files.category_id`.
    #[serde(default)]
    pub category_id: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn default_up_priority() -> String {
    "normal".to_string()
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
            up_priority: "normal".to_string(),
            is_partial: false,
            completed_parts: Vec::new(),
            uploaded_bytes: 0,
            upload_rate_bps: 0,
            category_id: None,
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

    /// Computes the eMule 0.72a canonical credit multiplier
    /// (`CClientCredits::GetScoreRatio`, `ClientCredits.cpp`), clamped to
    /// [1.0, 10.0].
    ///
    /// Three candidate values, the *smallest* wins:
    /// - `2 * downloaded / uploaded` (ratio, or 10.0 when nothing uploaded)
    /// - `sqrt(downloaded / 1 MiB + 2)` (exponential cap: 9.2 MiB → ~3.34,
    ///   100 MiB → ~10.0)
    /// - linear cap for the first chunks: `(downloaded - 1 MiB) / 8 598 323
    ///   * 2.34 + 1.0` below ~9.2 MiB, else 10.0
    ///
    /// Peers that never sent us anything (< 1 MiB) get exactly 1.0.
    ///
    /// Semantic definitions:
    /// - `downloaded_bytes`: bytes received by us from this peer.
    /// - `uploaded_bytes`: bytes sent by us to this peer.
    pub fn credit_multiplier(&self) -> f64 {
        const ONE_MIB: f64 = 1_048_576.0;
        if self.downloaded_bytes < ONE_MIB as u64 {
            return 1.0;
        }
        let dl = self.downloaded_bytes as f64;
        let ratio = if self.uploaded_bytes > 0 {
            (dl * 2.0) / self.uploaded_bytes as f64
        } else {
            10.0
        };
        let sqrt_cap = (dl / ONE_MIB + 2.0).sqrt();
        let linear_cap = if dl < 9_646_899.0 {
            (dl - ONE_MIB) / 8_598_323.0 * 2.34 + 1.0
        } else {
            10.0
        };
        ratio.min(sqrt_cap.min(linear_cap)).clamp(1.0, 10.0)
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

        // Vector 2 (eMule formula): Received 5 MiB, Sent 0 →
        // min(10.0, sqrt(5+2)≈2.65, linear≈2.14) = ~2.14 — the linear cap
        // dominates below ~9.2 MiB. NOT 10.0 like the naive ratio.
        credit.downloaded_bytes = 5 * 1_048_576;
        credit.uploaded_bytes = 0;
        let m = credit.credit_multiplier();
        assert!(
            (m - 2.1415).abs() < 0.01,
            "Expected ~2.14 for 5MiB/0, got {m}"
        );

        // Vector 3: Received < 1 MiB => 1.0 floor regardless of ratio.
        credit.downloaded_bytes = 1000;
        credit.uploaded_bytes = 2000;
        assert_eq!(credit.credit_multiplier(), 1.0);

        // Vector 4: 100 MiB received, 5 MiB sent →
        // ratio = 2*100/5 = 40 → capped by sqrt(100+2)≈10.1 → min = 10.0
        credit.downloaded_bytes = 100 * 1_048_576;
        credit.uploaded_bytes = 5 * 1_048_576;
        assert_eq!(credit.credit_multiplier(), 10.0);

        // Vector 5: symmetric exchange 10 MiB / 10 MiB →
        // min(2.0, sqrt(12)≈3.46, 10.0) = 2.0
        credit.downloaded_bytes = 10 * 1_048_576;
        credit.uploaded_bytes = 10 * 1_048_576;
        let m = credit.credit_multiplier();
        assert!(
            (m - 2.0).abs() < 0.01,
            "Expected ~2.0 for 10MiB/10MiB, got {m}"
        );
    }

    #[test]
    fn peer_source_status_srclist_visibility_matches_emule() {
        // Sources vivantes (srclist eMule) : en attente, file d'attente /
        // re-demande, connexion en cours, connectée.
        for s in [
            PeerSourceStatus::Candidate,
            PeerSourceStatus::Allowed,
            PeerSourceStatus::Tried,
            PeerSourceStatus::Connected,
        ] {
            assert!(s.is_srclist_visible(), "{s:?} doit rester visible");
        }
        // Deadlist eMule : échec de connexion, FNF, filtre IP — persistées
        // pour le backoff mais masquées de la liste.
        for s in [
            PeerSourceStatus::Failed,
            PeerSourceStatus::Rejected,
            PeerSourceStatus::Filtered,
        ] {
            assert!(!s.is_srclist_visible(), "{s:?} doit être masqué");
        }
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
