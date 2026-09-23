// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

pub mod audit;
pub mod budgets;
pub mod cancellation;
pub mod manager;
pub mod permit;
pub mod profile;
pub mod state;
pub mod validator;

pub use audit::{
    redact_socket_addr, AuditDecision, AuditEventType, NetworkAuditEntry, NetworkAuditLog,
};
pub use budgets::{ResourceBudgets, MAX_PEER_CONNECTIONS_CAP};
pub use cancellation::CancellationToken;
pub use manager::{
    emule_ud_ratio, NetworkPolicyManager, PolicyError, PolicyRateLimiters,
};
pub use permit::{ConnectionPurpose, ListenPermit, SocketPermit};
pub use profile::NetworkProfile;
pub use state::NetworkState;
pub use validator::{
    canonicalize_ip, classify_ip, validate_connect_address, validate_listen_address, IpCategory,
    PolicyViolation,
};
