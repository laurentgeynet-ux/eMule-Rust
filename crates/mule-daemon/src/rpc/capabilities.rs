// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

/// Capabilities advertized by the daemon in `system.hello` and `system.status`.
///
/// Single source of truth: both RPC handlers must report the same set.
pub fn daemon_capabilities() -> Vec<&'static str> {
    vec![
        "downloads",
        "download-events",
        "history",
        "categories",
        "import_part_met",
        "import_known_met",
        "network-policy",
        "ip-filter",
        "rate-limits",
        "servers",
        "server-connection",
        "server-search",
        "server-sources",
        "peer-sources",
        "peer-connection",
        "peer-robustness",
        "swarm-dual-source",
        "shared-files",
        "upload-single-slot",
        "secure-ident",
        "peer-credits",
        "kad-contacts",
        #[cfg(feature = "lab-tools")]
        "lab-tools",
    ]
}
