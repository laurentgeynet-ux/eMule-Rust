use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::cancellation::CancellationToken;

/// Functional purpose of a connection or listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConnectionPurpose {
    /// Outbound TCP connection to an eD2K server.
    Ed2kServer,
    /// Outbound TCP connection to an eD2K peer.
    Ed2kPeer,
    /// UDP / Kademlia routing operation.
    KadNode,
    /// Inbound TCP listener socket.
    Listener,
}

/// Shared tracker for active resource counts.
#[derive(Debug, Default)]
pub struct PermitTracker {
    pub active_peer_conns: AtomicUsize,
    pub active_server_conns: AtomicUsize,
    pub active_listeners: AtomicUsize,
}

/// RAII permit authorizing an outbound socket connection under active policy and budgets.
/// Automatically releases its reserved budget upon drop.
#[derive(Debug)]
pub struct SocketPermit {
    id: u64,
    target_addr: SocketAddr,
    purpose: ConnectionPurpose,
    cancel_token: CancellationToken,
    tracker: Arc<PermitTracker>,
}

impl SocketPermit {
    pub(crate) fn new(
        id: u64,
        target_addr: SocketAddr,
        purpose: ConnectionPurpose,
        cancel_token: CancellationToken,
        tracker: Arc<PermitTracker>,
    ) -> Self {
        match purpose {
            ConnectionPurpose::Ed2kPeer | ConnectionPurpose::KadNode => {
                tracker.active_peer_conns.fetch_add(1, Ordering::SeqCst);
            }
            ConnectionPurpose::Ed2kServer => {
                tracker.active_server_conns.fetch_add(1, Ordering::SeqCst);
            }
            ConnectionPurpose::Listener => {
                tracker.active_listeners.fetch_add(1, Ordering::SeqCst);
            }
        }
        Self {
            id,
            target_addr,
            purpose,
            cancel_token,
            tracker,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn target_addr(&self) -> SocketAddr {
        self.target_addr
    }

    pub fn purpose(&self) -> ConnectionPurpose {
        self.purpose
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    pub async fn cancelled(&self) {
        self.cancel_token.cancelled().await;
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }
}

impl Drop for SocketPermit {
    fn drop(&mut self) {
        match self.purpose {
            ConnectionPurpose::Ed2kPeer | ConnectionPurpose::KadNode => {
                self.tracker.active_peer_conns.fetch_sub(1, Ordering::SeqCst);
            }
            ConnectionPurpose::Ed2kServer => {
                self.tracker.active_server_conns.fetch_sub(1, Ordering::SeqCst);
            }
            ConnectionPurpose::Listener => {
                self.tracker.active_listeners.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }
}

/// RAII permit authorizing a bound listener socket under active policy and budgets.
/// Automatically decrements active listener count upon drop.
#[derive(Debug)]
pub struct ListenPermit {
    id: u64,
    bind_addr: SocketAddr,
    cancel_token: CancellationToken,
    tracker: Arc<PermitTracker>,
}

impl ListenPermit {
    pub(crate) fn new(
        id: u64,
        bind_addr: SocketAddr,
        cancel_token: CancellationToken,
        tracker: Arc<PermitTracker>,
    ) -> Self {
        tracker.active_listeners.fetch_add(1, Ordering::SeqCst);
        Self {
            id,
            bind_addr,
            cancel_token,
            tracker,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    pub async fn cancelled(&self) {
        self.cancel_token.cancelled().await;
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }
}

impl Drop for ListenPermit {
    fn drop(&mut self) {
        self.tracker.active_listeners.fetch_sub(1, Ordering::SeqCst);
    }
}
