// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Helpers mutualisés pour les tests d'intégration du workspace.
//!
//! Toutes les fonctions de cette crate ne dialoguent qu'en loopback
//! (`127.0.0.1`) et n'émettent aucun trafic réseau sortant, conformément à la
//! règle « tests hors-ligne uniquement » d'AGENTS.md.
//!
//! La crate est destinée aux `dev-dependencies` des autres crates : elle ne
//! doit être utilisée ni par du code de production ni publiée.

pub mod daemon;
pub mod met_fixtures;
pub mod peer_fixtures;
pub mod rpc;
pub mod state;
pub mod ws;

pub use daemon::{
    spawn_test_daemon, spawn_test_daemon_with_mode, spawn_test_daemon_with_temp_dir,
    RunningDaemon,
};
pub use rpc::make_req;
pub use state::{
    download_state, download_state_with_obfuscation, kad_state, rpc_state,
    rpc_state_with_config, rpc_state_with_profile, share_state,
};
pub use ws::{recv_response_by_id, send_json};
