// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use crate::client::DaemonClient;
use crate::protocol::{ToolCallResult, ToolDefinition};
use super::{obj_schema, prop};
use serde_json::{json, Value};

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "emule_list_servers".to_string(),
            description: "List all known eD2K servers, connection status, IP filter state, ping, and preference.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_connect_server".to_string(),
            description: "Connect to an eD2K server by address 'ip:port' or automatically pick the best allowed server.".to_string(),
            input_schema: obj_schema(json!({"address": prop("string", "Server address as 'ip:port' (optional, auto-picks available server if omitted)"),
                    "ip": prop("string", "Server IP (optional)"),
                    "port": prop("integer", "Server port (optional)")}), &[]),
        },
        ToolDefinition {
            name: "emule_disconnect_server".to_string(),
            description: "Disconnect from the currently connected eD2K server.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_update_servers_from_url".to_string(),
            description: "Download and import server list from a remote server.met URL.".to_string(),
            input_schema: obj_schema(json!({"url": prop("string", "URL to server.met file (default: http://upd.emule-security.org/server.met)")}), &[]),
        },
        ToolDefinition {
            name: "emule_inspect_server_met".to_string(),
            description: "Inspect an offline server.met file on disk to examine its server list and IP filter status.".to_string(),
            input_schema: obj_schema(json!({"file_path": prop("string", "Path to server.met file")}), &["file_path"]),
        },
        ToolDefinition {
            name: "emule_import_server_met".to_string(),
            description: "Import servers from a local server.met file into the daemon database.".to_string(),
            input_schema: obj_schema(json!({"file_path": prop("string", "Path to server.met file")}), &["file_path"]),
        },
        ToolDefinition {
            name: "emule_set_server_enabled".to_string(),
            description: "Enable or disable a server in the local server list.".to_string(),
            input_schema: obj_schema(json!({"address": prop("string", "Server address as 'ip:port'"),
                    "enabled": prop("boolean", "Whether server is enabled")}), &["address", "enabled"]),
        },
        ToolDefinition {
            name: "emule_delete_server".to_string(),
            description: "Delete an eD2K server from the local server list.".to_string(),
            input_schema: obj_schema(json!({"address": prop("string", "Server address as 'ip:port'")}), &["address"]),
        },
        ToolDefinition {
            name: "emule_clear_servers".to_string(),
            description: "Clear all servers from the local server list.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_query_server_sources".to_string(),
            description: "Send a GetSources request to the connected server for a file hash.".to_string(),
            input_schema: obj_schema(json!({"file_hash": prop("string", "32-character hex eD2K hash"),
                    "file_size": prop("integer", "File size in bytes (optional)")}), &["file_hash"]),
        },
        ToolDefinition {
            name: "emule_search".to_string(),
            description: "Search for files on connected ED2K servers and Kad DHT network. Automatically waits for and returns search results including synthesized ed2k links ready for download.".to_string(),
            input_schema: obj_schema(json!({"query": prop("string", "Search keyword query (e.g. 'ubuntu iso')"),
                    "network": {
                        "type": "string",
                        "enum": ["server", "kad", "both"],
                        "description": "Network to search on ('server', 'kad', or 'both', default: 'kad')"
                    },
                    "min_size_mb": prop("number", "Minimum file size in megabytes"),
                    "max_size_mb": prop("number", "Maximum file size in megabytes"),
                    "extension": prop("string", "Filter by file extension without dot (e.g. 'iso', 'zip')"),
                    "wait_seconds": prop("integer", "Seconds to wait for incoming search results (default: 4)")}), &["query"]),
        },
    ]
}

pub async fn handle_tool(
    client: &DaemonClient,
    tool_name: &str,
    args: &Value,
) -> Option<ToolCallResult> {
    match tool_name {
        "emule_search" => {
            let query = match args.get("query").and_then(|q| q.as_str()) {
                Some(q) => q,
                None => return Some(ToolCallResult::error("Missing required parameter: 'query'")),
            };

            let network = args
                .get("network")
                .and_then(|n| n.as_str())
                .unwrap_or("kad");
            let min_size = args
                .get("min_size_mb")
                .and_then(|m| m.as_f64())
                .map(|m| (m * 1024.0 * 1024.0) as u64);
            let max_size = args
                .get("max_size_mb")
                .and_then(|m| m.as_f64())
                .map(|m| (m * 1024.0 * 1024.0) as u64);
            let extension = args.get("extension").and_then(|e| e.as_str());
            let wait_secs = args
                .get("wait_seconds")
                .and_then(|w| w.as_u64())
                .unwrap_or(4);

            match client
                .search_and_collect(
                    query,
                    Some(network),
                    min_size,
                    max_size,
                    extension,
                    wait_secs,
                )
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Search failed: {e}"))),
            }
        }

        "emule_list_servers" => match client.call("servers.list", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to list servers: {e}"
            ))),
        },

        "emule_connect_server" => {
            let address = if let Some(addr) = args.get("address").and_then(|a| a.as_str()) {
                Some(addr.to_string())
            } else if let (Some(ip), Some(port)) = (
                args.get("ip").and_then(|i| i.as_str()),
                args.get("port").and_then(|p| p.as_u64()),
            ) {
                Some(format!("{ip}:{port}"))
            } else {
                // Auto-pick the first allowed server from servers.list
                let s_list = client.call("servers.list", json!({})).await.ok();
                s_list.and_then(|v| {
                    v.get("servers").and_then(|s| s.as_array()).and_then(|arr| {
                        arr.iter()
                            .find(|s| {
                                s.get("filterStatus").and_then(|f| f.as_str()) == Some("allowed")
                            })
                            .and_then(|s| {
                                s.get("address").and_then(|a| a.as_str()).map(String::from)
                            })
                    })
                })
            };

            let addr_str =
                match address {
                    Some(a) => a,
                    None => return Some(ToolCallResult::error(
                        "No server address specified and no available server found in server list",
                    )),
                };

            match client
                .call("servers.connect", json!({ "address": addr_str }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to connect to server '{addr_str}': {e}"
                ))),
            }
        }

        "emule_disconnect_server" => match client.call("servers.disconnect", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to disconnect server: {e}"
            ))),
        },

        "emule_update_servers_from_url" => {
            let url = args
                .get("url")
                .and_then(|u| u.as_str())
                .unwrap_or("http://upd.emule-security.org/server.met");
            match client
                .call("servers.updateFromUrl", json!({ "url": url }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to update servers from URL: {e}"
                ))),
            }
        }

        "emule_inspect_server_met" => {
            let file_path = match args.get("file_path").and_then(|f| f.as_str()) {
                Some(f) => f,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_path'",
                    ))
                }
            };

            match client
                .call("servers.inspectMet", json!({ "filePath": file_path }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to inspect server.met: {e}"
                ))),
            }
        }

        "emule_import_server_met" => {
            let file_path = match args.get("file_path").and_then(|f| f.as_str()) {
                Some(f) => f,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_path'",
                    ))
                }
            };

            match client
                .call("servers.importMet", json!({ "filePath": file_path }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to import server.met: {e}"
                ))),
            }
        }

        "emule_set_server_enabled" => {
            let address = match args.get("address").and_then(|a| a.as_str()) {
                Some(a) => a,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'address'",
                    ))
                }
            };
            let enabled = match args.get("enabled").and_then(|e| e.as_bool()) {
                Some(e) => e,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'enabled'",
                    ))
                }
            };

            match client
                .call(
                    "servers.setEnabled",
                    json!({ "address": address, "enabled": enabled }),
                )
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to set server enabled status: {e}"
                ))),
            }
        }

        "emule_delete_server" => {
            let address = match args.get("address").and_then(|a| a.as_str()) {
                Some(a) => a,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'address'",
                    ))
                }
            };

            match client
                .call("servers.delete", json!({ "address": address }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to delete server: {e}"
                ))),
            }
        }

        "emule_clear_servers" => match client.call("servers.clear", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to clear server list: {e}"
            ))),
        },

        "emule_query_server_sources" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_hash'",
                    ))
                }
            };
            let size = args.get("file_size").and_then(|s| s.as_u64()).unwrap_or(0);

            match client
                .call(
                    "servers.querySources",
                    json!({ "fileHash": hash, "fileSize": size }),
                )
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to query server sources: {e}"
                ))),
            }
        }

        _ => None,
    }
}
