// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use crate::client::DaemonClient;
use crate::protocol::ToolDefinition;
use serde_json::{json, Value};

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "emule_status".to_string(),
            description: "Get overall EMule-Rust daemon status, uptime, transfer speeds, active downloads count, server connection, and Kad status.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_get_settings".to_string(),
            description: "Get EMule-Rust daemon configuration, default paths, limits, and retention policy.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_update_settings".to_string(),
            description: "Update daemon settings: default download folder, history retention, peer connection quotas.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "default_download_dir": {
                        "type": "string",
                        "description": "Absolute path to the new default completed downloads folder"
                    },
                    "history_retention_days": {
                        "type": "integer",
                        "description": "Number of days to keep download history records (0 = forever)"
                    },
                    "max_global_download_peers": {
                        "type": "integer",
                        "description": "Maximum number of simultaneous peer connections across all downloads"
                    },
                    "max_peers_per_download": {
                        "type": "integer",
                        "description": "Maximum active peer connections allowed for a single download"
                    },
                    "min_peers_per_download": {
                        "type": "integer",
                        "description": "Target minimum peers to maintain per active download"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_reset_download_dir".to_string(),
            description: "Reset the default download directory to the default data folder.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_set_limits".to_string(),
            description: "Configure global download and upload rate limits in KB/s (0 = unlimited).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "download_limit_kbps": {
                        "type": "integer",
                        "description": "Max download rate in KB/s (0 for unlimited)"
                    },
                    "upload_limit_kbps": {
                        "type": "integer",
                        "description": "Max upload rate in KB/s (0 for unlimited)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_shutdown".to_string(),
            description: "Gracefully shut down the EMule-Rust background daemon.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_get_network_policy".to_string(),
            description: "Get active network security policy, profile ('standard', 'vpn_only', 'tor_proxy', 'paranoid', 'local_lab'), budgets, and active connection counts.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_set_network_profile".to_string(),
            description: "Switch the network isolation profile ('standard', 'vpn_only', 'tor_proxy', 'paranoid', 'local_lab').".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["profile"],
                "properties": {
                    "profile": {
                        "type": "string",
                        "enum": ["standard", "vpn_only", "tor_proxy", "paranoid", "local_lab"],
                        "description": "Target network profile"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_network_disconnect_all".to_string(),
            description: "Emergency network killswitch: immediately disconnects all server, peer, and listener sockets.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_network_enable".to_string(),
            description: "Re-enable network communications after a killswitch disconnect.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_get_audit_log".to_string(),
            description: "Inspect network security audit log events (blocked connections, profile violations, rate limiting).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Max entries to retrieve (default: 50)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_set_network_budgets".to_string(),
            description: "Set network bandwidth and connection budgets dynamically.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["budgets"],
                "properties": {
                    "budgets": {
                        "type": "object",
                        "description": "Budget definitions for rate limits and concurrent socket limits"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_get_filter_status".to_string(),
            description: "Get IP filter security status, loaded rules count, and blocked connection statistics.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_import_ip_filter".to_string(),
            description: "Import IP blocklist file (ipfilter.dat, guarding.p2p, or peerguardian format) to block malicious peers.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_path"],
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to ipfilter.dat file"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_set_filter_enabled".to_string(),
            description: "Enable or disable IP blocklist filtering dynamically.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["enabled"],
                "properties": {
                    "enabled": {
                        "type": "boolean",
                        "description": "Whether the IP filter is active"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_clear_ip_filter".to_string(),
            description: "Clear all IP filter rules from memory.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}

pub async fn handle_tool(
    client: &DaemonClient,
    tool_name: &str,
    args: &Value,
) -> Option<crate::protocol::ToolCallResult> {
    use crate::protocol::ToolCallResult;
    match tool_name {
        "emule_status" => match client.call("system.status", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to retrieve status: {e}"
            ))),
        },
        "emule_get_settings" => match client.call("settings.get", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to get settings: {e}"
            ))),
        },
        "emule_update_settings" => {
            let mut params = json!({});
            if let Some(d) = args.get("default_download_dir").and_then(|s| s.as_str()) {
                params["defaultDownloadDir"] = json!(d);
            }
            if let Some(r) = args.get("history_retention_days").and_then(|s| s.as_u64()) {
                params["historyRetentionDays"] = json!(r);
            }
            if let Some(g) = args
                .get("max_global_download_peers")
                .and_then(|s| s.as_u64())
            {
                params["maxGlobalDownloadPeers"] = json!(g);
            }
            if let Some(p) = args.get("max_peers_per_download").and_then(|s| s.as_u64()) {
                params["maxPeersPerDownload"] = json!(p);
            }
            if let Some(m) = args.get("min_peers_per_download").and_then(|s| s.as_u64()) {
                params["minPeersPerDownload"] = json!(m);
            }

            match client.call("settings.set", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to update settings: {e}"
                ))),
            }
        }
        "emule_reset_download_dir" => {
            match client.call("settings.resetDownloadDir", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to reset download directory: {e}"
                ))),
            }
        }
        "emule_set_limits" => {
            let mut params = json!({});
            if let Some(dl) = args.get("download_limit_kbps").and_then(|d| d.as_u64()) {
                params["downloadLimit"] = json!(dl);
            }
            if let Some(ul) = args.get("upload_limit_kbps").and_then(|u| u.as_u64()) {
                params["uploadLimit"] = json!(ul);
            }

            match client.call("limits.set", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to set rate limits: {e}"
                ))),
            }
        }
        "emule_shutdown" => match client.call("system.stop", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to shut down daemon: {e}"
            ))),
        },
        "emule_get_network_policy" => match client.call("network.getPolicy", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to get network policy: {e}"
            ))),
        },
        "emule_set_network_profile" => {
            let profile = match args.get("profile").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'profile'",
                    ))
                }
            };

            match client
                .call("network.setProfile", json!({ "profile": profile }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to set network profile: {e}"
                ))),
            }
        }
        "emule_network_disconnect_all" => {
            match client.call("network.disconnectAll", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to trigger network disconnect: {e}"
                ))),
            }
        }
        "emule_network_enable" => match client.call("network.enable", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to enable network: {e}"
            ))),
        },
        "emule_get_audit_log" => {
            let mut params = json!({});
            if let Some(l) = args.get("limit").and_then(|s| s.as_u64()) {
                params["limit"] = json!(l);
            }

            match client.call("network.getAuditLog", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to get network audit log: {e}"
                ))),
            }
        }
        "emule_set_network_budgets" => {
            let budgets = match args.get("budgets") {
                Some(b) => b,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'budgets'",
                    ))
                }
            };

            match client
                .call("network.setBudgets", json!({ "budgets": budgets }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to set network budgets: {e}"
                ))),
            }
        }
        "emule_get_filter_status" => match client.call("filter.getStatus", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to get IP filter status: {e}"
            ))),
        },
        "emule_import_ip_filter" => {
            let file_path = match args.get("file_path").and_then(|f| f.as_str()) {
                Some(f) => f,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_path'",
                    ))
                }
            };

            match client
                .call("filter.importFile", json!({ "filePath": file_path }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to import IP filter: {e}"
                ))),
            }
        }
        "emule_set_filter_enabled" => {
            let enabled = match args.get("enabled").and_then(|e| e.as_bool()) {
                Some(e) => e,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'enabled'",
                    ))
                }
            };

            match client
                .call("filter.setEnabled", json!({ "enabled": enabled }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to set filter enabled state: {e}"
                ))),
            }
        }
        "emule_clear_ip_filter" => match client.call("filter.clear", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to clear IP filter: {e}"
            ))),
        },
        _ => None,
    }
}
