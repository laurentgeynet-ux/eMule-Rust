// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Tests du profil de filtrage `--tool-profile` (mule-mcp::tools::policy).

use mule_mcp::tools::policy::{classify, ToolProfile, ToolRisk};
use mule_mcp::tools::{execute_tool_for, list_tools, list_tools_for};
use mule_mcp::DaemonClient;
use mule_test_support::spawn_test_daemon;
use serde_json::json;

/// Exhaustivité de la classification : tout outil exposé doit avoir un
/// niveau de risque — c'est ce test qui force la revue de risque à
/// l'ajout d'un nouvel outil.
#[test]
fn test_every_tool_is_classified() {
    for tool in list_tools() {
        assert!(
            classify(&tool.name).is_some(),
            "outil non classé dans policy.rs : {}",
            tool.name
        );
    }
}

/// Le profil `read` n'expose que des outils de lecture.
#[test]
fn test_read_profile_exposes_only_read_tools() {
    let tools = list_tools_for(ToolProfile::Read);
    assert!(!tools.is_empty());
    for tool in &tools {
        assert_eq!(
            classify(&tool.name),
            Some(ToolRisk::Read),
            "outil non-Read exposé en profil read : {}",
            tool.name
        );
    }
    // Repères connus : une lecture présente, une mutation et un
    // destructif absents.
    assert!(tools.iter().any(|t| t.name == "emule_status"));
    assert!(!tools.iter().any(|t| t.name == "emule_add_link"));
    assert!(!tools.iter().any(|t| t.name == "emule_shutdown"));
}

/// Le profil `safe` exclut les destructifs mais garde les mutations.
#[test]
fn test_safe_profile_excludes_destructive_tools() {
    let tools = list_tools_for(ToolProfile::Safe);
    for tool in &tools {
        assert_ne!(
            classify(&tool.name),
            Some(ToolRisk::Destructive),
            "outil destructif exposé en profil safe : {}",
            tool.name
        );
    }
    assert!(tools.iter().any(|t| t.name == "emule_status"));
    assert!(tools.iter().any(|t| t.name == "emule_add_link"));
    assert!(!tools.iter().any(|t| t.name == "emule_shutdown"));
    assert!(!tools.iter().any(|t| t.name == "emule_remove_download"));
}

/// Le profil `full` reproduit la liste complète.
#[test]
fn test_full_profile_exposes_everything() {
    assert_eq!(list_tools_for(ToolProfile::Full).len(), list_tools().len());
}

/// Le refus d'exécution a lieu AVANT tout contact daemon : un outil hors
/// profil est rejeté même avec un client valide.
#[tokio::test]
async fn test_execute_tool_blocked_by_profile() {
    let (addr, token, _dir) =
        spawn_test_daemon("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef").await;
    let client = DaemonClient::connect(Some(format!("ws://{addr}/rpc")), Some(token), None, None)
        .await
        .expect("DaemonClient connection should succeed");

    // Mutation refusée en profil read
    let res = execute_tool_for(
        ToolProfile::Read,
        &client,
        "emule_add_link",
        json!({ "link": "ed2k://|file|x.bin|1|31D6CFE0D16AE931B73C59D7E0C089C0|/" }),
    )
    .await;
    assert!(res.is_error);
    assert!(res.content[0].text.contains("blocked by profile 'read'"));

    // Destructif refusé en profil safe
    let res = execute_tool_for(ToolProfile::Safe, &client, "emule_shutdown", json!({})).await;
    assert!(res.is_error);
    assert!(res.content[0].text.contains("blocked by profile 'safe'"));

    // Outil inconnu : refus quel que soit le profil
    let res = execute_tool_for(ToolProfile::Full, &client, "emule_nope", json!({})).await;
    assert!(res.is_error);
    assert!(res.content[0].text.contains("Unknown tool"));

    // Lecture autorisée en profil read : l'appel atteint le daemon
    let res = execute_tool_for(ToolProfile::Read, &client, "emule_status", json!({})).await;
    assert!(!res.is_error, "status blocked unexpectedly: {:?}", res);
}
