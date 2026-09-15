use serde::{Deserialize, Serialize};

/// High-level lifecycle state of the network subsystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkState {
    /// Network is completely disabled (Kill Switch engaged, or not initialized).
    /// No sockets are active and no new connection attempts are allowed.
    Disabled,
    /// Policy is active and ready in preview mode; no active connections currently open.
    PreviewActive,
    /// One or more connections are actively open and operating within policy.
    Connected,
    /// Kill switch or graceful shutdown is actively in progress.
    ShuttingDown,
    /// Network subsystem encountered a fatal error or unrecoverable budget failure.
    Error,
}

impl Default for NetworkState {
    fn default() -> Self {
        Self::PreviewActive
    }
}

impl NetworkState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::PreviewActive => "preview-active",
            Self::Connected => "connected",
            Self::ShuttingDown => "shutting-down",
            Self::Error => "error",
        }
    }

    /// Whether this state allows requesting new socket permits.
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::PreviewActive | Self::Connected)
    }
}

impl std::fmt::Display for NetworkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
