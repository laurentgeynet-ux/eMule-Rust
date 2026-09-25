// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Tests de la policy assistant : allowlist, capacités, confirmations.

use mule_assistant::policy::{
    check_call, required_capability, requires_confirmation, visible_tools,
};
use mule_daemon::rpc::capabilities::daemon_capabilities;
use mule_mcp::tools::{list_tools, ToolProfile};

/// L'allowlist ne doit référencer que des outils existants — sinon un
/// renommage d'outil MCP passerait inaperçu.
#[test]
fn test_allowlist_references_known_tools() {
    let known: std::collections::HashSet<String> =
        list_tools().iter().map(|t| t.name.clone()).collect();
    let caps: Vec<String> = daemon_capabilities()
        .iter()
        .map(|s| s.to_string())
        .collect();
    for tool in visible_tools(ToolProfile::Full, &caps) {
        assert!(known.contains(&tool.name), "outil inconnu : {}", tool.name);
    }
}

/// Toute capacité référencée par le mapping existe dans
/// `daemon_capabilities()` — sinon un outil serait masqué à tort.
/// Avec toutes les capacités réelles présentes, toute l'allowlist est
/// visible (profil full).
#[test]
fn test_mapped_capabilities_exist_in_daemon() {
    let caps = daemon_capabilities();
    for tool in list_tools() {
        if let Some(cap) = required_capability(&tool.name) {
            assert!(
                caps.contains(&cap),
                "capacité '{cap}' (pour {}) absente de daemon_capabilities()",
                tool.name
            );
        }
    }
    let visible = visible_tools(ToolProfile::Full, &caps_strs(&caps));
    let names: Vec<&str> = visible.iter().map(|t| t.name.as_str()).collect();
    for expected in [
        "emule_status",
        "emule_search",
        "emule_add_link",
        "emule_kad_status",
    ] {
        assert!(names.contains(&expected), "{expected} masqué à tort");
    }
}

/// Les outils dont la capacité est absente sont masqués.
#[test]
fn test_missing_capability_hides_tool() {
    let visible = visible_tools(ToolProfile::Full, &[]);
    // "downloads" absent → les outils downloads sont masqués.
    assert!(!visible.iter().any(|t| t.name == "emule_add_link"));
    // emule_status n'a pas de capacité requise → reste visible.
    assert!(visible.iter().any(|t| t.name == "emule_status"));
}

/// check_call : allowlist, profil et capacités sont tous revérifiés.
#[test]
fn test_check_call_enforces_all_layers() {
    let caps = caps_strs(&daemon_capabilities());

    // Hors allowlist → refus même si l'outil existe et est Read.
    assert!(check_call(ToolProfile::Full, &caps, "emule_clear_history").is_err());

    // Allowlist + profil read → refus des mutations.
    assert!(check_call(ToolProfile::Read, &caps, "emule_add_link").is_err());

    // Allowlist + capacité absente → refus.
    assert!(check_call(ToolProfile::Full, &[], "emule_add_link").is_err());

    // Tout aligné → accepté.
    assert!(check_call(ToolProfile::Full, &caps, "emule_status").is_ok());
    assert!(check_call(ToolProfile::Full, &caps, "emule_add_link").is_ok());
}

/// Confirmations : les outils à effets externes et les destructifs.
#[test]
fn test_requires_confirmation() {
    assert!(requires_confirmation("emule_add_link"));
    assert!(requires_confirmation("emule_connect_server"));
    // Destructif : confirmation même si présent dans un profil full.
    assert!(requires_confirmation("emule_shutdown"));
    // Réversible local : pas de confirmation.
    assert!(!requires_confirmation("emule_pause_download"));
    assert!(!requires_confirmation("emule_status"));
}

fn caps_strs(caps: &[&'static str]) -> Vec<String> {
    caps.iter().map(|s| s.to_string()).collect()
}
