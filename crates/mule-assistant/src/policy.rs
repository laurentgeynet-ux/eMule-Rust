// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Garde-fous propres à l'IA : quels outils le modèle peut voir et
//! invoquer, et lesquels exigent une confirmation humaine.
//!
//! Trois filtres cumulatifs, revérifiés juste avant chaque exécution —
//! les capacités ou la connexion peuvent changer pendant une session :
//! 1. [`TOOL_ALLOWLIST`] : sous-ensemble curé présenté au modèle
//!    (plafond statique — la fiabilité du tool calling d'un modèle
//!    local se mesure, elle ne se présume pas universelle) ;
//! 2. [`ToolProfile`] : plafond de risque (`mule-mcp::tools::policy`) ;
//! 3. Capacités réelles du daemon (`system.status` → `capabilities`) —
//!    l'intersection masque les outils dont la capacité est absente.

use mule_mcp::protocol::ToolDefinition;
use mule_mcp::tools::{classify, list_tools_for, ToolProfile, ToolRisk};

use crate::error::AssistantError;

/// Sous-ensemble curé d'outils présentés au modèle : opérations usuelles
/// d'un client eD2K/Kad. Ajuster après mesure du taux de bonne sélection
/// sur les modèles cibles.
const TOOL_ALLOWLIST: &[&str] = &[
    // État général
    "emule_status",
    "emule_get_settings",
    // Recherche et connexion réseau
    "emule_search",
    "emule_list_servers",
    "emule_connect_server",
    "emule_disconnect_server",
    "emule_kad_status",
    // Téléchargements
    "emule_list_downloads",
    "emule_get_download_details",
    "emule_add_link",
    "emule_pause_download",
    "emule_resume_download",
    // Partage et sources
    "emule_list_shares",
    "emule_swarm_status",
    "emule_list_sources",
];

/// Outils de l'allowlist exigeant une confirmation utilisateur affichant
/// les arguments exacts — effets externes ou difficilement réversibles.
/// `emule_pause_download`/`emule_resume_download` restent libres :
/// réversibles et purement locaux. Tout outil classé
/// [`ToolRisk::Destructive`] demande aussi confirmation (défense en
/// profondeur si le profil `full` laisse passer un destructif).
const CONFIRM_REQUIRED: &[&str] = &[
    "emule_add_link",
    "emule_connect_server",
    "emule_disconnect_server",
];

/// Capacité daemon requise par outil — préfixes évalués dans l'ordre,
/// premier match gagne ; aucun match = toujours permis. La nomenclature
/// des capacités (`daemon_capabilities()` côté mule-daemon) est
/// par domaine, pas par outil — ce mapping est la seule traduction.
const TOOL_CAPABILITY_PREFIXES: &[(&str, &str)] = &[
    ("emule_kad_", "kad-contacts"),
    ("emule_list_categor", "categories"),
    ("emule_create_categor", "categories"),
    ("emule_update_categor", "categories"),
    ("emule_delete_categor", "categories"),
    ("emule_reorder_categor", "categories"),
    ("emule_download_set_category", "categories"),
    ("emule_list_history", "history"),
    ("emule_history_stats", "history"),
    ("emule_export_history", "history"),
    ("emule_clear_history", "history"),
    ("emule_get_network_policy", "network-policy"),
    ("emule_set_network_profile", "network-policy"),
    ("emule_network_", "network-policy"),
    ("emule_set_network_budgets", "network-policy"),
    ("emule_get_filter_status", "ip-filter"),
    ("emule_import_ip_filter", "ip-filter"),
    ("emule_set_filter_enabled", "ip-filter"),
    ("emule_clear_ip_filter", "ip-filter"),
    ("emule_set_limits", "rate-limits"),
    ("emule_list_credits", "peer-credits"),
    ("emule_credits_stats", "peer-credits"),
    ("emule_get_identity", "secure-ident"),
    ("emule_rotate_identity", "secure-ident"),
    ("emule_list_shares", "shared-files"),
    ("emule_add_share", "shared-files"),
    ("emule_remove_share", "shared-files"),
    ("emule_rename_share", "shared-files"),
    ("emule_set_share_enabled", "shared-files"),
    ("emule_list_servers", "servers"),
    ("emule_update_servers_from_url", "servers"),
    ("emule_inspect_server_met", "servers"),
    ("emule_import_server_met", "servers"),
    ("emule_set_server_enabled", "servers"),
    ("emule_delete_server", "servers"),
    ("emule_clear_servers", "servers"),
    ("emule_connect_server", "server-connection"),
    ("emule_disconnect_server", "server-connection"),
    ("emule_search", "server-search"),
    ("emule_query_server_sources", "server-sources"),
    ("emule_list_sources", "peer-sources"),
    ("emule_add_manual_source", "peer-sources"),
    ("emule_clear_sources", "peer-sources"),
    ("emule_connect_source", "peer-connection"),
    ("emule_disconnect_source", "peer-connection"),
    ("emule_swarm_status", "swarm-dual-source"),
    ("emule_list_uploads", "upload-single-slot"),
    ("emule_cancel_upload", "upload-single-slot"),
    ("emule_list_downloads", "downloads"),
    ("emule_get_download_details", "downloads"),
    ("emule_add_link", "downloads"),
    ("emule_pause_download", "downloads"),
    ("emule_resume_download", "downloads"),
    ("emule_remove_download", "downloads"),
    ("emule_rename_download", "downloads"),
    ("emule_set_destination", "downloads"),
    ("emule_retry_download", "downloads"),
    ("emule_inspect_part_met", "downloads"),
    ("emule_import_part_met", "downloads"),
];

/// Capacité requise pour `tool_name` — `None` = pas de capacité dédiée.
/// Publique pour le test qui vérifie la table contre
/// `daemon_capabilities()`.
pub fn required_capability(tool_name: &str) -> Option<&'static str> {
    TOOL_CAPABILITY_PREFIXES
        .iter()
        .find(|(prefix, _)| tool_name.starts_with(prefix))
        .map(|(_, cap)| *cap)
}

/// Outils visibles pour le modèle : `list_tools_for(profile)` filtrée
/// par l'allowlist puis par les capacités réelles du daemon.
pub fn visible_tools(profile: ToolProfile, capabilities: &[String]) -> Vec<ToolDefinition> {
    list_tools_for(profile)
        .into_iter()
        .filter(|t| TOOL_ALLOWLIST.contains(&t.name.as_str()))
        .filter(|t| {
            required_capability(&t.name)
                .map(|cap| capabilities.iter().any(|c| c == cap))
                .unwrap_or(true)
        })
        .collect()
}

/// Vérification d'exécution — appelée juste avant `execute_tool`, pas
/// seulement à l'établissement de la liste : le modèle peut demander un
/// outil hors allowlist et les capacités peuvent avoir changé.
pub fn check_call(
    profile: ToolProfile,
    capabilities: &[String],
    tool_name: &str,
) -> Result<(), AssistantError> {
    if !TOOL_ALLOWLIST.contains(&tool_name) {
        return Err(AssistantError::Config(format!(
            "outil '{tool_name}' hors de l'allowlist assistant"
        )));
    }
    let risk = classify(tool_name).ok_or_else(|| {
        AssistantError::Config(format!("outil '{tool_name}' non classé dans la policy MCP"))
    })?;
    if !profile.allows(risk) {
        return Err(AssistantError::Config(format!(
            "outil '{tool_name}' bloqué par le profil '{}'",
            profile.as_str()
        )));
    }
    if let Some(cap) = required_capability(tool_name) {
        if !capabilities.iter().any(|c| c == cap) {
            return Err(AssistantError::Config(format!(
                "capacité '{cap}' absente du daemon pour '{tool_name}'"
            )));
        }
    }
    Ok(())
}

/// Vrai si l'outil exige une confirmation utilisateur avant exécution :
/// membres explicites de [`CONFIRM_REQUIRED`] plus tout destructif
/// (dernier rempart côté assistant — le daemon reste l'autorité).
pub fn requires_confirmation(tool_name: &str) -> bool {
    CONFIRM_REQUIRED.contains(&tool_name) || classify(tool_name) == Some(ToolRisk::Destructive)
}
