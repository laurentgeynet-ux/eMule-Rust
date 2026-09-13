use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CODE_PARSE_ERROR: i32 = -32700;
pub const CODE_INVALID_REQUEST: i32 = -32600;
pub const CODE_METHOD_NOT_FOUND: i32 = -32601;
pub const CODE_INVALID_PARAMS: i32 = -32602;
pub const CODE_INTERNAL_ERROR: i32 = -32603;
pub const CODE_AUTH_FAILED: i32 = -32000;
pub const CODE_PROTOCOL_MISMATCH: i32 = -32001;
pub const CODE_BUSINESS_ERROR: i32 = -32002;

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
        Self::new(CODE_METHOD_NOT_FOUND, format!("Method '{method}' not found"))
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

    pub fn business(msg: impl Into<String>) -> Self {
        Self::new(CODE_BUSINESS_ERROR, msg)
    }
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC Error [{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}
