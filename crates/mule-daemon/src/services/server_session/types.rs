// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde::{Deserialize, Serialize};

/// Current lifecycle status of an eD2K server connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerConnectionStatus {
    /// No connection active.
    Disconnected,
    /// TCP handshake and login request in progress.
    Connecting,
    /// Successfully logged in and ready for operations.
    Connected,
    /// Disconnection in progress.
    Disconnecting,
}

impl ServerConnectionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Disconnecting => "disconnecting",
        }
    }
}

/// Snapshot of the current eD2K server session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerSessionSnapshot {
    /// Current connection status.
    pub status: ServerConnectionStatus,
    /// Server address ("ip:port") if connected or connecting.
    pub server_address: Option<String>,
    /// Server name if received in ident or hello.
    pub server_name: Option<String>,
    /// Server description.
    pub server_desc: Option<String>,
    /// Server assigned Client ID.
    pub client_id: Option<u32>,
    /// True if client ID >= 16_777_216.
    pub is_high_id: bool,
    /// Server MOTD / greeting message.
    pub welcome_message: Option<String>,
    /// Active users reported by server.
    pub current_users: Option<u32>,
    /// Active files reported by server.
    pub current_files: Option<u32>,
    /// Timestamp when connection completed (unix ms).
    pub connected_at_unix_ms: Option<i64>,
    /// Ping / roundtrip time estimate in milliseconds.
    pub ping_ms: Option<u32>,
}

impl Default for ServerSessionSnapshot {
    fn default() -> Self {
        Self {
            status: ServerConnectionStatus::Disconnected,
            server_address: None,
            server_name: None,
            server_desc: None,
            client_id: None,
            is_high_id: false,
            welcome_message: None,
            current_users: None,
            current_files: None,
            connected_at_unix_ms: None,
            ping_ms: None,
        }
    }
}
