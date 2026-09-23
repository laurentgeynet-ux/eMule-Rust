// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

pub mod events;
pub mod handlers;
pub mod methods;
pub mod server;
pub mod types;

pub use events::DownloadSnapshot;
pub use methods::{DAEMON_VERSION, PROTOCOL_VERSION};
pub use server::create_router;
pub use types::{JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
