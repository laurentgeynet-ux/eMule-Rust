// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use crate::client::DaemonClient;
use crate::protocol::{ToolCallResult, ToolDefinition};
use serde_json::{json, Value};

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "emule_kad_status".to_string(),
            description: "Get Kad DHT network status: connection state, open/firewalled status, contacts count.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_kad_bootstrap".to_string(),
            description: "Bootstrap the Kad DHT network from a known node IP:port or a nodes.dat URL.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ip": {
                        "type": "string",
                        "description": "Bootstrap node IP (optional)"
                    },
                    "port": {
                        "type": "integer",
                        "description": "Bootstrap node UDP port (optional)"
                    },
                    "url": {
                        "type": "string",
                        "description": "URL to download nodes.dat from (optional)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_kad_start_udp".to_string(),
            description: "Start the Kad DHT UDP socket listener.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "bind_address": {
                        "type": "string",
                        "description": "Socket address to bind (e.g. '0.0.0.0:39737')"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_kad_stop_udp".to_string(),
            description: "Stop the Kad DHT UDP socket listener.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_kad_ping".to_string(),
            description: "Ping a Kad DHT contact node to verify connectivity and measure latency.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["ip", "port"],
                "properties": {
                    "ip": {
                        "type": "string",
                        "description": "Contact node IP"
                    },
                    "port": {
                        "type": "integer",
                        "description": "Contact node UDP port"
                    },
                    "kad_id": {
                        "type": "string",
                        "description": "Optional 32-character hex target Kad ID"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_kad_find_node".to_string(),
            description: "Search Kad DHT routing tree for contacts closest to a 128-bit hex node ID.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["target_id"],
                "properties": {
                    "target_id": {
                        "type": "string",
                        "description": "128-bit hex Kad ID (32 hex characters)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_kad_list_contacts".to_string(),
            description: "List contacts stored in the Kad DHT routing table (IP, port, Kad ID, version).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Max contacts to return (default: 100)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_kad_clear_contacts".to_string(),
            description: "Clear all contacts from the Kad DHT routing table.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_kad_search_sources".to_string(),
            description: "Query Kad DHT specifically for download sources of a file hash.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash"
                    },
                    "file_size": {
                        "type": "integer",
                        "description": "File size in bytes (optional)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_kad_publish_source".to_string(),
            description: "Publish ourselves as a source for a shared file to the Kad DHT.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash", "file_size"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash"
                    },
                    "file_size": {
                        "type": "integer",
                        "description": "File size in bytes"
                    },
                    "tcp_port": {
                        "type": "integer",
                        "description": "TCP listening port (default: 39735)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_kad_publish_keywords".to_string(),
            description: "Publish search keyword index entries for a shared file to the Kad DHT.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash", "file_name", "file_size"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash"
                    },
                    "file_name": {
                        "type": "string",
                        "description": "File name"
                    },
                    "file_size": {
                        "type": "integer",
                        "description": "File size in bytes"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_kad_search_notes".to_string(),
            description: "Query Kad DHT for file notes (user comments and ratings) indexed for a file hash.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash", "file_size"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash"
                    },
                    "file_size": {
                        "type": "integer",
                        "description": "File size in bytes (used to filter matching note entries)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_kad_publish_note".to_string(),
            description: "Publish a comment/rating for a file to the Kad DHT notes index.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash", "file_name", "file_size", "comment"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash"
                    },
                    "file_name": {
                        "type": "string",
                        "description": "File name"
                    },
                    "file_size": {
                        "type": "integer",
                        "description": "File size in bytes"
                    },
                    "comment": {
                        "type": "string",
                        "description": "Comment text (max 512 bytes)"
                    },
                    "rating": {
                        "type": "integer",
                        "description": "Optional rating 0-5"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_kad_inspect_nodes_dat".to_string(),
            description: "Inspect an offline nodes.dat file on disk to examine its Kad contacts before importing.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to nodes.dat file"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_kad_import_nodes_dat".to_string(),
            description: "Import Kad contact nodes from a local nodes.dat file on disk.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to nodes.dat file"
                    }
                }
            }),
        },
    ]
}

pub async fn handle_tool(
    client: &DaemonClient,
    tool_name: &str,
    args: &Value,
) -> Option<ToolCallResult> {
    match tool_name {
        "emule_kad_status" => match client.call("kad.status", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to get Kad status: {e}"
            ))),
        },

        "emule_kad_bootstrap" => {
            if let Some(url) = args.get("url").and_then(|u| u.as_str()) {
                match client
                    .call("kad.bootstrapFromUrl", json!({ "url": url }))
                    .await
                {
                    Ok(res) => Some(ToolCallResult::json(&res)),
                    Err(e) => Some(ToolCallResult::error(format!(
                        "Failed to bootstrap Kad from URL: {e}"
                    ))),
                }
            } else if let (Some(ip), Some(port)) = (
                args.get("ip").and_then(|i| i.as_str()),
                args.get("port").and_then(|p| p.as_u64()),
            ) {
                match client
                    .call("kad.bootstrap", json!({ "ip": ip, "port": port }))
                    .await
                {
                    Ok(res) => Some(ToolCallResult::json(&res)),
                    Err(e) => Some(ToolCallResult::error(format!(
                        "Failed to bootstrap Kad node: {e}"
                    ))),
                }
            } else {
                Some(ToolCallResult::error(
                    "Provide either 'url' or both 'ip' and 'port' to bootstrap Kad",
                ))
            }
        }

        "emule_kad_start_udp" => {
            let mut params = json!({});
            if let Some(b) = args.get("bind_address").and_then(|s| s.as_str()) {
                params["bindAddress"] = json!(b);
            }

            match client.call("kad.startUdp", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to start Kad UDP socket: {e}"
                ))),
            }
        }

        "emule_kad_stop_udp" => match client.call("kad.stopUdp", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to stop Kad UDP socket: {e}"
            ))),
        },

        "emule_kad_ping" => {
            let ip = match args.get("ip").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return Some(ToolCallResult::error("Missing required parameter: 'ip'")),
            };
            let port = match args.get("port").and_then(|p| p.as_u64()) {
                Some(p) => p,
                None => return Some(ToolCallResult::error("Missing required parameter: 'port'")),
            };

            let mut params = json!({ "ip": ip, "port": port });
            if let Some(k) = args.get("kad_id").and_then(|s| s.as_str()) {
                params["kadId"] = json!(k);
            }

            match client.call("kad.ping", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to ping Kad contact: {e}"
                ))),
            }
        }

        "emule_kad_find_node" => {
            let target_id = match args.get("target_id").and_then(|t| t.as_str()) {
                Some(t) => t,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'target_id'",
                    ))
                }
            };

            match client
                .call("kad.findNode", json!({ "targetId": target_id }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to find node in Kad: {e}"
                ))),
            }
        }

        "emule_kad_list_contacts" => {
            let mut params = json!({});
            if let Some(l) = args.get("limit").and_then(|s| s.as_u64()) {
                params["limit"] = json!(l);
            }

            match client.call("kad.listContacts", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to list Kad contacts: {e}"
                ))),
            }
        }

        "emule_kad_clear_contacts" => match client.call("kad.clearContacts", json!({})).await {
            Ok(res) => Some(ToolCallResult::json(&res)),
            Err(e) => Some(ToolCallResult::error(format!(
                "Failed to clear Kad contacts: {e}"
            ))),
        },

        "emule_kad_search_sources" => {
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
                    "kad.searchSources",
                    json!({ "fileHash": hash, "fileSize": size }),
                )
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to search Kad sources: {e}"
                ))),
            }
        }

        "emule_kad_publish_source" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_hash'",
                    ))
                }
            };
            let size = match args.get("file_size").and_then(|s| s.as_u64()) {
                Some(s) => s,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_size'",
                    ))
                }
            };

            let mut params = json!({ "fileHash": hash, "fileSize": size });
            if let Some(t) = args.get("tcp_port").and_then(|s| s.as_u64()) {
                params["tcpPort"] = json!(t);
            }

            match client.call("kad.publishSource", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to publish source to Kad: {e}"
                ))),
            }
        }

        "emule_kad_publish_keywords" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_hash'",
                    ))
                }
            };
            let name = match args.get("file_name").and_then(|n| n.as_str()) {
                Some(n) => n,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_name'",
                    ))
                }
            };
            let size = match args.get("file_size").and_then(|s| s.as_u64()) {
                Some(s) => s,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_size'",
                    ))
                }
            };

            match client
                .call(
                    "kad.publishKeywords",
                    json!({
                        "fileHash": hash,
                        "fileName": name,
                        "fileSize": size
                    }),
                )
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to publish keywords to Kad: {e}"
                ))),
            }
        }

        "emule_kad_search_notes" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_hash'",
                    ))
                }
            };
            let size = match args.get("file_size").and_then(|s| s.as_u64()) {
                Some(s) => s,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_size'",
                    ))
                }
            };

            match client
                .call(
                    "kad.searchNotes",
                    json!({ "fileHash": hash, "fileSize": size }),
                )
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to search Kad notes: {e}"
                ))),
            }
        }

        "emule_kad_publish_note" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_hash'",
                    ))
                }
            };
            let name = match args.get("file_name").and_then(|n| n.as_str()) {
                Some(n) => n,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_name'",
                    ))
                }
            };
            let size = match args.get("file_size").and_then(|s| s.as_u64()) {
                Some(s) => s,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'file_size'",
                    ))
                }
            };
            let comment = match args.get("comment").and_then(|c| c.as_str()) {
                Some(c) => c,
                None => {
                    return Some(ToolCallResult::error(
                        "Missing required parameter: 'comment'",
                    ))
                }
            };

            let mut params = json!({
                "fileHash": hash,
                "fileName": name,
                "fileSize": size,
                "comment": comment
            });
            if let Some(r) = args.get("rating").and_then(|r| r.as_u64()) {
                params["rating"] = json!(r.min(5));
            }

            match client.call("kad.publishNote", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to publish note to Kad: {e}"
                ))),
            }
        }

        "emule_kad_inspect_nodes_dat" => {
            let path = match args.get("path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return Some(ToolCallResult::error("Missing required parameter: 'path'")),
            };

            match client
                .call("kad.inspectNodesDat", json!({ "path": path }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to inspect nodes.dat: {e}"
                ))),
            }
        }

        "emule_kad_import_nodes_dat" => {
            let path = match args.get("path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return Some(ToolCallResult::error("Missing required parameter: 'path'")),
            };

            match client
                .call("kad.importNodesDat", json!({ "path": path }))
                .await
            {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!(
                    "Failed to import nodes.dat: {e}"
                ))),
            }
        }

        _ => None,
    }
}
