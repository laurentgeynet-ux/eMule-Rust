// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Fixtures de pairs pour les tests swarm / corruption : un `PeerSource`
//! candidat minimal découvert via serveur.

use mule_core::model::{PeerSource, PeerSourceOrigin, PeerSourceStatus};

/// Construit un `PeerSource` candidat issu d'une réponse serveur.
pub fn peer_source(file_hash: &str, addr: &str) -> PeerSource {
    PeerSource {
        file_hash: file_hash.to_string(),
        address: addr.to_string(),
        ip: 0,
        port: 0,
        origin: PeerSourceOrigin::Server,
        discovered_at_unix_ms: 0,
        last_seen_unix_ms: 0,
        status: PeerSourceStatus::Candidate,
        fail_count: 0,
        last_error: None,
        cooldown_until_unix_ms: None,
        user_hash: None,
        crypt_options: 0,
        buddy_id: None,
        software: None,
        availability: None,
        server_ip: 0,
        server_port: 0,
        udp_port: 0,
    }
}
