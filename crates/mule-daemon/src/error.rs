use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CODE_PARSE_ERROR: i32 = -32700;
pub const CODE_INVALID_REQUEST: i32 = -32600;
pub const CODE_METHOD_NOT_FOUND: i32 = -32601;
pub const CODE_INVALID_PARAMS: i32 = -32602;
pub const CODE_INTERNAL_ERROR: i32 = -32603;

// Application & Security specific error codes (-32000 .. -32099)
pub const CODE_AUTH_FAILED: i32 = -32000;
pub const CODE_PROTOCOL_MISMATCH: i32 = -32001;
pub const CODE_FORBIDDEN_IN_INSTALLED_MODE: i32 = -32002;
pub const CODE_UNSAFE_CLEANUP_TARGET: i32 = -32003;
pub const CODE_FIXTURE_DOWNLOAD_MISMATCH: i32 = -32004;
pub const CODE_LOOPBACK_PEER_UNAVAILABLE: i32 = -32005;
pub const CODE_BUSINESS_ERROR: i32 = -32010;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(code: i32, message: impl Into<String>, data: Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(
            CODE_METHOD_NOT_FOUND,
            format!("Method '{method}' not found"),
        )
    }

    pub fn invalid_params(details: &str) -> Self {
        Self::new(CODE_INVALID_PARAMS, format!("Invalid params: {details}"))
    }

    pub fn auth_failed() -> Self {
        Self::new(CODE_AUTH_FAILED, "Authentication failed: invalid token")
    }

    pub fn protocol_mismatch(expected: u32, got: u32) -> Self {
        Self::new(
            CODE_PROTOCOL_MISMATCH,
            format!("Protocol version mismatch: expected {expected}, got {got}"),
        )
    }

    pub fn forbidden_in_installed_mode(msg: impl Into<String>) -> Self {
        Self::new(CODE_FORBIDDEN_IN_INSTALLED_MODE, msg)
    }

    pub fn unsafe_cleanup_target(msg: impl Into<String>) -> Self {
        Self::new(CODE_UNSAFE_CLEANUP_TARGET, msg)
    }

    pub fn fixture_download_mismatch(msg: impl Into<String>) -> Self {
        Self::new(CODE_FIXTURE_DOWNLOAD_MISMATCH, msg)
    }

    pub fn loopback_peer_unavailable(msg: impl Into<String>) -> Self {
        Self::new(CODE_LOOPBACK_PEER_UNAVAILABLE, msg)
    }

    pub fn business(msg: impl Into<String>) -> Self {
        Self::new(CODE_BUSINESS_ERROR, msg)
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(CODE_INTERNAL_ERROR, msg)
    }
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC Error [{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("Another daemon instance is already running on data directory: {0}")]
    AlreadyRunning(std::path::PathBuf),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Core storage error: {0}")]
    Core(#[from] mule_core::CoreError),
    #[error("Session file error: {0}")]
    Session(String),
    #[error("Business error: {0}")]
    Business(String),
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<DaemonError> for JsonRpcError {
    fn from(err: DaemonError) -> Self {
        match err {
            DaemonError::InvalidParams(msg) => JsonRpcError::invalid_params(&msg),
            DaemonError::NotFound(msg) | DaemonError::Business(msg) => JsonRpcError::business(msg),
            DaemonError::AlreadyRunning(p) => {
                JsonRpcError::business(format!("Daemon already running on {}", p.display()))
            }
            DaemonError::Io(e) => JsonRpcError::internal(e.to_string()),
            DaemonError::Core(e) => JsonRpcError::business(e.to_string()),
            DaemonError::Session(e) => JsonRpcError::business(e),
            DaemonError::Internal(e) => JsonRpcError::internal(e),
        }
    }
}
