use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use thiserror::Error;

use crate::audit::{AuditDecision, AuditEventType, NetworkAuditEntry, NetworkAuditLog};
use crate::budgets::ResourceBudgets;
use crate::cancellation::CancellationToken;
use crate::permit::{ConnectionPurpose, ListenPermit, PermitTracker, SocketPermit};
use crate::profile::NetworkProfile;
use crate::state::NetworkState;
use crate::validator::{validate_connect_address, validate_listen_address, PolicyViolation};

/// Errors encountered when requesting network permits or updating policy.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PolicyError {
    #[error("Network is disabled (Kill Switch active)")]
    NetworkDisabled,

    #[error("Address policy violation: {0}")]
    AddressViolation(#[from] PolicyViolation),

    #[error("Resource budget exhausted: {0}")]
    BudgetExhausted(&'static str),

    #[error("Invalid budget configuration: {0}")]
    InvalidBudgets(&'static str),

    #[error("Kademlia UDP operations not permitted in profile {0}")]
    KadNotAllowedInProfile(NetworkProfile),

    #[error("Network subsystem in shutting down state")]
    ShuttingDown,
}

#[derive(Debug)]
struct PolicyManagerInner {
    profile: NetworkProfile,
    state: NetworkState,
    budgets: ResourceBudgets,
    audit_log: NetworkAuditLog,
    next_permit_id: u64,
}

/// Central network policy coordinator for Emule-Sion.
/// Mandatory single choke point for validating addresses, issuing RAII permits,
/// enforcing resource budgets, and triggering atomic global disconnection.
#[derive(Debug, Clone)]
pub struct NetworkPolicyManager {
    inner: Arc<RwLock<PolicyManagerInner>>,
    tracker: Arc<PermitTracker>,
    cancel_token: CancellationToken,
}

impl NetworkPolicyManager {
    /// Creates a new manager with the specified initial profile and resource budgets.
    pub fn new(profile: NetworkProfile, budgets: ResourceBudgets) -> Self {
        Self {
            inner: Arc::new(RwLock::new(PolicyManagerInner {
                profile,
                state: NetworkState::PreviewActive,
                budgets,
                audit_log: NetworkAuditLog::new(500),
                next_permit_id: 1,
            })),
            tracker: Arc::new(PermitTracker::default()),
            cancel_token: CancellationToken::new(),
        }
    }

    /// Returns the currently active network profile.
    pub fn profile(&self) -> NetworkProfile {
        self.inner.read().unwrap().profile
    }

    /// Updates the network profile, recording the transition in the audit log.
    pub fn set_profile(&self, new_profile: NetworkProfile) {
        let mut inner = self.inner.write().unwrap();
        let old = inner.profile;
        if old != new_profile {
            inner.profile = new_profile;
            inner.audit_log.record(
                AuditEventType::ProfileChanged,
                None,
                new_profile,
                AuditDecision::Allowed,
                Some(format!("Switched profile from {old} to {new_profile}")),
            );
            tracing::info!(from = %old, to = %new_profile, "Network profile updated");
        }
    }

    /// Returns the current dynamic network state.
    pub fn state(&self) -> NetworkState {
        let inner = self.inner.read().unwrap();
        match inner.state {
            NetworkState::Disabled => NetworkState::Disabled,
            NetworkState::ShuttingDown => NetworkState::ShuttingDown,
            NetworkState::Error => NetworkState::Error,
            NetworkState::PreviewActive | NetworkState::Connected => {
                let total_active = self.tracker.active_peer_conns.load(Ordering::SeqCst)
                    + self.tracker.active_server_conns.load(Ordering::SeqCst);
                if total_active > 0 {
                    NetworkState::Connected
                } else {
                    NetworkState::PreviewActive
                }
            }
        }
    }

    /// Returns the configured resource budgets.
    pub fn budgets(&self) -> ResourceBudgets {
        self.inner.read().unwrap().budgets.clone()
    }

    /// Updates resource budgets after validating parameter bounds.
    pub fn set_budgets(&self, budgets: ResourceBudgets) -> Result<(), PolicyError> {
        budgets.validate().map_err(PolicyError::InvalidBudgets)?;
        let mut inner = self.inner.write().unwrap();
        inner.budgets = budgets;
        Ok(())
    }

    /// Requests an outbound TCP socket permit for a target address and purpose.
    /// Fails if the network is disabled, the address violates profile constraints,
    /// or active connection budgets are exhausted.
    pub fn request_tcp_connect(
        &self,
        addr: SocketAddr,
        purpose: ConnectionPurpose,
    ) -> Result<SocketPermit, PolicyError> {
        let mut inner = self.inner.write().unwrap();
        let profile = inner.profile;

        // 1. Check lifecycle state
        if inner.state == NetworkState::Disabled {
            inner.audit_log.record(
                AuditEventType::ConnectAttempt,
                Some(&addr),
                profile,
                AuditDecision::Denied,
                Some("Network disabled by Kill Switch".to_string()),
            );
            return Err(PolicyError::NetworkDisabled);
        }
        if inner.state == NetworkState::ShuttingDown {
            return Err(PolicyError::ShuttingDown);
        }

        // 2. Check protocol capability (Kademlia UDP in preview)
        if purpose == ConnectionPurpose::KadNode && !profile.allows_udp_kad() {
            let reason = format!("Kademlia UDP not allowed in profile {profile}");
            inner.audit_log.record(
                AuditEventType::ConnectAttempt,
                Some(&addr),
                profile,
                AuditDecision::Denied,
                Some(reason),
            );
            return Err(PolicyError::KadNotAllowedInProfile(profile));
        }

        // 3. Validate destination address
        if let Err(violation) = validate_connect_address(&addr, profile) {
            let reason = violation.to_string();
            inner.audit_log.record(
                AuditEventType::ConnectAttempt,
                Some(&addr),
                profile,
                AuditDecision::Denied,
                Some(reason),
            );
            return Err(PolicyError::AddressViolation(violation));
        }

        // 4. Validate resource budgets
        match purpose {
            ConnectionPurpose::Ed2kPeer | ConnectionPurpose::KadNode => {
                let active = self.tracker.active_peer_conns.load(Ordering::SeqCst);
                if active >= inner.budgets.max_active_peer_connections {
                    let reason = format!(
                        "Active peer connections limit reached ({active}/{})",
                        inner.budgets.max_active_peer_connections
                    );
                    inner.audit_log.record(
                        AuditEventType::BudgetExhausted,
                        Some(&addr),
                        profile,
                        AuditDecision::Denied,
                        Some(reason),
                    );
                    return Err(PolicyError::BudgetExhausted("max_active_peer_connections"));
                }
            }
            ConnectionPurpose::Ed2kServer => {
                let active = self.tracker.active_server_conns.load(Ordering::SeqCst);
                if active >= inner.budgets.max_active_server_connections {
                    let reason = format!(
                        "Active server connections limit reached ({active}/{})",
                        inner.budgets.max_active_server_connections
                    );
                    inner.audit_log.record(
                        AuditEventType::BudgetExhausted,
                        Some(&addr),
                        profile,
                        AuditDecision::Denied,
                        Some(reason),
                    );
                    return Err(PolicyError::BudgetExhausted("max_active_server_connections"));
                }
            }
            ConnectionPurpose::Listener => {
                let active = self.tracker.active_listeners.load(Ordering::SeqCst);
                if active >= inner.budgets.max_active_listeners {
                    return Err(PolicyError::BudgetExhausted("max_active_listeners"));
                }
            }
        }

        // 5. Issue permit
        let permit_id = inner.next_permit_id;
        inner.next_permit_id = inner.next_permit_id.wrapping_add(1);

        inner.audit_log.record(
            AuditEventType::ConnectAttempt,
            Some(&addr),
            profile,
            AuditDecision::Allowed,
            Some(format!("Authorized permit #{permit_id} for {purpose:?}")),
        );

        let permit = SocketPermit::new(
            permit_id,
            addr,
            purpose,
            self.cancel_token.clone(),
            self.tracker.clone(),
        );

        Ok(permit)
    }

    /// Requests a listener socket permit for binding to an address.
    pub fn request_tcp_listen(
        &self,
        addr: SocketAddr,
        purpose: ConnectionPurpose,
    ) -> Result<ListenPermit, PolicyError> {
        let mut inner = self.inner.write().unwrap();
        let profile = inner.profile;

        if inner.state == NetworkState::Disabled {
            return Err(PolicyError::NetworkDisabled);
        }
        if inner.state == NetworkState::ShuttingDown {
            return Err(PolicyError::ShuttingDown);
        }

        if let Err(violation) = validate_listen_address(&addr, profile) {
            let reason = violation.to_string();
            inner.audit_log.record(
                AuditEventType::ListenAttempt,
                Some(&addr),
                profile,
                AuditDecision::Denied,
                Some(reason),
            );
            return Err(PolicyError::AddressViolation(violation));
        }

        let active = self.tracker.active_listeners.load(Ordering::SeqCst);
        if active >= inner.budgets.max_active_listeners {
            return Err(PolicyError::BudgetExhausted("max_active_listeners"));
        }

        let permit_id = inner.next_permit_id;
        inner.next_permit_id = inner.next_permit_id.wrapping_add(1);

        inner.audit_log.record(
            AuditEventType::ListenAttempt,
            Some(&addr),
            profile,
            AuditDecision::Allowed,
            Some(format!("Authorized listener permit #{permit_id} for {purpose:?}")),
        );

        let permit = ListenPermit::new(
            permit_id,
            addr,
            self.cancel_token.clone(),
            self.tracker.clone(),
        );

        Ok(permit)
    }

    /// Global Kill Switch ("Disconnect All").
    /// Atomically marks the network as Disabled, triggers cancellation of all outstanding
    /// socket tasks, and writes an audit event.
    pub fn disconnect_all(&self) {
        let profile = {
            let mut inner = self.inner.write().unwrap();
            inner.state = NetworkState::ShuttingDown;
            let profile = inner.profile;
            inner.audit_log.record(
                AuditEventType::KillSwitchTriggered,
                None,
                profile,
                AuditDecision::Allowed,
                Some("Kill switch activated: tearing down all connections".to_string()),
            );
            tracing::warn!("Kill switch activated: cancelling all network operations");
            profile
        };

        // Trigger cancellation to all active permits
        self.cancel_token.cancel();

        // Finalize state to Disabled
        let mut inner = self.inner.write().unwrap();
        inner.state = NetworkState::Disabled;
        let _ = profile;
    }

    /// Re-enables the network from Disabled state.
    pub fn enable(&self) {
        let mut inner = self.inner.write().unwrap();
        if inner.state == NetworkState::Disabled || inner.state == NetworkState::ShuttingDown {
            self.cancel_token.reset();
            inner.state = NetworkState::PreviewActive;
            let profile = inner.profile;
            inner.audit_log.record(
                AuditEventType::NetworkEnabled,
                None,
                profile,
                AuditDecision::Allowed,
                Some("Network policy re-enabled".to_string()),
            );
            tracing::info!("Network policy re-enabled");
        }
    }

    /// Retrieves the most recent network audit log entries.
    pub fn audit_entries(&self, limit: usize) -> Vec<NetworkAuditEntry> {
        let inner = self.inner.read().unwrap();
        inner.audit_log.recent(limit)
    }

    /// Returns the cancellation token associated with the network manager.
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    /// Returns the number of currently active peer connections.
    pub fn active_peer_connections_count(&self) -> usize {
        self.tracker.active_peer_conns.load(Ordering::SeqCst)
    }

    /// Returns the number of currently active server connections.
    pub fn active_server_connections_count(&self) -> usize {
        self.tracker.active_server_conns.load(Ordering::SeqCst)
    }

    /// Returns the number of currently active listeners.
    pub fn active_listeners_count(&self) -> usize {
        self.tracker.active_listeners.load(Ordering::SeqCst)
    }
}
