// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

pub mod client_udp;
pub mod downloads;
pub mod events;
pub mod identity;
pub mod inbound_listener;
pub mod incoming_share;
pub mod kad;
pub mod kad_udp;
pub mod obfuscation_stream;
pub mod peer_session;
pub mod server_session;
pub mod server_udp;
pub mod swarm_allocator;
pub mod swarm_coordinator;
pub mod upload_queue;
pub mod upnp;
pub mod uss;
mod uss_icmp;

pub use client_udp::{ClientUdpService, UdpReaskResult, UdpReaskTarget};
pub use events::DaemonEvent;
pub use identity::{IdentityService, IdentityStatus};
pub use kad::{KadContactEntryDto, KadService, KadStatusSnapshot, NodesDatInspectReport};
pub use kad_udp::{KadUdpService, KadUdpSnapshot, UdpFirewallStatus};
pub use obfuscation_stream::ObfuscatedStream;
pub use upnp::{UpnpService, UpnpStatusSnapshot};

pub use downloads::{
    DownloadDetail, DownloadService, DownloadSnapshot, ImportPartMetResult, PartMetInspection,
    PartProgressSummary, SourceAvailabilitySummary,
};
pub use inbound_listener::{InboundListenerService, InboundListenerSnapshot};
pub use peer_session::{PeerConnectionStatus, PeerSessionManager, PeerSessionSnapshot};
pub use server_session::{ServerConnectionStatus, ServerSessionManager, ServerSessionSnapshot};
pub use swarm_allocator::{FileDemand, SwarmAllocationConfig, SwarmAllocator};
pub use swarm_coordinator::SwarmCoordinator;
pub use upload_queue::{
    UploadManager, UploadSessionSnapshot, UploadSlotState, MAX_ACTIVE_UPLOADS, MAX_UPLOAD_QUEUE,
};
pub use uss::{UssConfig, UssService, UssSnapshot};
