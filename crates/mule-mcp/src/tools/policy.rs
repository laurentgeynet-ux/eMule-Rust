// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Classification de risque et profils de filtrage des outils MCP.
//!
//! Source unique de vérité pour « quel outil peut faire quoi » : les trois
//! tables ci-dessous couvrent l'exhaustivité de [`crate::tools::list_tools`]
//! — `tests/tool_profile_tests.rs` échoue si un outil ajouté n'est pas
//! classé, ce qui force la revue du niveau de risque à chaque nouvel outil.
//!
//! Le filtrage s'applique à DEUX endroits : `tools/list` (visibilité) et
//! `tools/call` (exécution) — filtrer la liste seule est cosmétique, un
//! client peut invoquer un outil non listé par son nom.

use clap::ValueEnum;

/// Niveau de risque d'un outil MCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRisk {
    /// Lecture seule : aucune mutation d'état côté daemon.
    Read,
    /// Mutation d'état réversible ou à effet borné (connexion, pause,
    /// import, publication…).
    Mutate,
    /// Action destructive ou critique : perte de données, arrêt du daemon,
    /// coupure réseau, perte d'identité/credits.
    Destructive,
}

/// Profil de filtrage exposé via `--tool-profile`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum ToolProfile {
    /// Outils de lecture uniquement — exposition sans mutation.
    Read,
    /// Lecture + mutations, sans les actions destructives.
    Safe,
    /// Tous les outils — comportement historique, profil par défaut.
    #[default]
    Full,
}

impl ToolProfile {
    /// Vrai si le profil autorise l'exécution/la présentation d'un outil
    /// de niveau `risk`.
    pub fn allows(self, risk: ToolRisk) -> bool {
        match self {
            ToolProfile::Read => risk == ToolRisk::Read,
            ToolProfile::Safe => risk != ToolRisk::Destructive,
            ToolProfile::Full => true,
        }
    }

    /// Libellé stable utilisé dans les messages d'erreur et les logs.
    pub fn as_str(self) -> &'static str {
        match self {
            ToolProfile::Read => "read",
            ToolProfile::Safe => "safe",
            ToolProfile::Full => "full",
        }
    }
}

/// Outils de lecture seule — aucun effet de bord côté daemon.
const READ_TOOLS: &[&str] = &[
    // system
    "emule_status",
    "emule_get_settings",
    "emule_get_network_policy",
    "emule_get_audit_log",
    "emule_get_filter_status",
    // servers
    "emule_list_servers",
    "emule_inspect_server_met",
    "emule_query_server_sources",
    "emule_search",
    // kad
    "emule_kad_status",
    "emule_kad_ping",
    "emule_kad_find_node",
    "emule_kad_list_contacts",
    "emule_kad_search_sources",
    "emule_kad_search_notes",
    "emule_kad_inspect_nodes_dat",
    // shares / uploads / identité / historique
    "emule_list_sources",
    "emule_swarm_status",
    "emule_list_shares",
    "emule_list_uploads",
    "emule_listener_status",
    "emule_upnp_status",
    "emule_get_identity",
    "emule_list_credits",
    "emule_credits_stats",
    "emule_list_history",
    "emule_history_stats",
    "emule_export_history",
    // downloads
    "emule_list_downloads",
    "emule_get_download_details",
    "emule_inspect_part_met",
    "emule_list_categories",
];

/// Outils destructifs ou critiques : perte irréversible, arrêt, coupure
/// réseau globale, perte d'identité (les credits eD2K sont liés au hash).
const DESTRUCTIVE_TOOLS: &[&str] = &[
    "emule_shutdown",
    "emule_network_disconnect_all",
    "emule_clear_ip_filter",
    "emule_delete_server",
    "emule_clear_servers",
    "emule_kad_clear_contacts",
    "emule_clear_sources",
    "emule_remove_share",
    "emule_cancel_upload",
    "emule_rotate_identity",
    "emule_clear_history",
    "emule_remove_download",
    "emule_delete_category",
];

/// Outils de mutation d'état réversible ou bornée.
const MUTATE_TOOLS: &[&str] = &[
    // system
    "emule_update_settings",
    "emule_reset_download_dir",
    "emule_set_limits",
    "emule_set_network_profile",
    "emule_network_enable",
    "emule_set_network_budgets",
    "emule_import_ip_filter",
    "emule_set_filter_enabled",
    // servers
    "emule_connect_server",
    "emule_disconnect_server",
    "emule_update_servers_from_url",
    "emule_import_server_met",
    "emule_set_server_enabled",
    // kad
    "emule_kad_bootstrap",
    "emule_kad_start_udp",
    "emule_kad_stop_udp",
    "emule_kad_publish_source",
    "emule_kad_publish_keywords",
    "emule_kad_publish_note",
    "emule_kad_import_nodes_dat",
    // shares / uploads / listener / upnp
    "emule_add_manual_source",
    "emule_connect_source",
    "emule_disconnect_source",
    "emule_add_share",
    "emule_rename_share",
    "emule_set_share_enabled",
    "emule_start_listener",
    "emule_stop_listener",
    "emule_upnp_map_ports",
    "emule_upnp_unmap_ports",
    "emule_upnp_set_enabled",
    // downloads
    "emule_add_link",
    "emule_pause_download",
    "emule_resume_download",
    "emule_rename_download",
    "emule_set_destination",
    "emule_download_set_category",
    "emule_retry_download",
    "emule_import_part_met",
    "emule_create_category",
    "emule_update_category",
    "emule_reorder_categories",
];

/// Classification d'un outil par nom. `None` = outil non répertorié :
/// traité comme « refusé » hors profil `Full` (défaut sûr), et détecté
/// par le test d'exhaustivité.
pub fn classify(tool_name: &str) -> Option<ToolRisk> {
    if READ_TOOLS.contains(&tool_name) {
        return Some(ToolRisk::Read);
    }
    if DESTRUCTIVE_TOOLS.contains(&tool_name) {
        return Some(ToolRisk::Destructive);
    }
    if MUTATE_TOOLS.contains(&tool_name) {
        return Some(ToolRisk::Mutate);
    }
    None
}
