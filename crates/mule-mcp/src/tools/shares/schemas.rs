//! Tool definitions (schemas) for shares, sources, history, uploads, and listeners.

use crate::protocol::ToolDefinition;
use serde_json::json;

/// Returns the schema definitions for all share/source/network tools.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "emule_list_sources".to_string(),
            description: "List all known peer sources for a specific file hash.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_add_manual_source".to_string(),
            description: "Add a manual peer source by address 'ip:port' for a file hash.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash", "address"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash"
                    },
                    "address": {
                        "type": "string",
                        "description": "Peer address as 'ip:port' (e.g. '192.168.1.50:4662')"
                    },
                    "user_hash": {
                        "type": "string",
                        "description": "Optional 32-character hex user hash"
                    },
                    "crypt_options": {
                        "type": "integer",
                        "description": "Optional encryption options flags"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_connect_source".to_string(),
            description: "Force immediate connection to a specific peer source for a file.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash", "address"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash"
                    },
                    "address": {
                        "type": "string",
                        "description": "Peer address as 'ip:port'"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_disconnect_source".to_string(),
            description: "Disconnect from a specific peer source or the active peer.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "address": {
                        "type": "string",
                        "description": "Peer address to disconnect (optional, disconnects current peer if omitted)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_clear_sources".to_string(),
            description: "Clear all discovered peer sources for a file hash.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_swarm_status".to_string(),
            description: "Get full swarm status: active peers count, in-flight blocks, chunk distribution.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "Optional file hash to get swarm status for"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_list_shares".to_string(),
            description: "List all files currently shared with the eD2K/Kad network.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_add_share".to_string(),
            description: "Share a file with other eD2K/Kad peers.".to_string(),
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
                        "description": "Name of the file"
                    },
                    "file_size": {
                        "type": "integer",
                        "description": "File size in bytes"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Optional disk path to the file"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_remove_share".to_string(),
            description: "Unshare a file from the eD2K/Kad network, optionally deleting the file on disk.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash of shared file"
                    },
                    "delete_file": {
                        "type": "boolean",
                        "description": "Also delete the file on disk (for a completed download this removes the download entry too). Rejected for partial shares of in-progress downloads (default: false)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_rename_share".to_string(),
            description: "Rename a fully shared file: the file on disk and its share entry are renamed (and the linked completed download when one exists).".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash", "new_name"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash of shared file"
                    },
                    "new_name": {
                        "type": "string",
                        "description": "New file name (no path separators or Windows-illegal characters)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_set_share_enabled".to_string(),
            description: "Enable or disable sharing for a file without removing it from the share list.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash", "enabled"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash"
                    },
                    "enabled": {
                        "type": "boolean",
                        "description": "Whether file sharing is enabled"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_list_uploads".to_string(),
            description: "List active and queued upload transfers to remote peers.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_cancel_upload".to_string(),
            description: "Cancel/release an active upload transfer slot for a remote client address.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["client_address"],
                "properties": {
                    "client_address": {
                        "type": "string",
                        "description": "Remote client address as 'ip:port'"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_start_listener".to_string(),
            description: "Start the TCP inbound peer connection listener.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "port": {
                        "type": "integer",
                        "description": "TCP listening port (default: 39735)"
                    },
                    "bind_ip": {
                        "type": "string",
                        "description": "IP interface to bind (default: '127.0.0.1')"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_stop_listener".to_string(),
            description: "Stop the TCP inbound peer connection listener.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_listener_status".to_string(),
            description: "Get TCP inbound listener status, bound address, and active connection count.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_upnp_status".to_string(),
            description: "Get UPnP automatic router port forwarding status, active mappings, and external IP.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_upnp_map_ports".to_string(),
            description: "Request UPnP port forwarding on your router for TCP and UDP eMule ports.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "tcp_port": {
                        "type": "integer",
                        "description": "TCP port to forward (optional)"
                    },
                    "udp_port": {
                        "type": "integer",
                        "description": "UDP port to forward (optional)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_upnp_unmap_ports".to_string(),
            description: "Remove UPnP port forwarding rules from the router.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_upnp_set_enabled".to_string(),
            description: "Enable or disable automatic UPnP port forwarding.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["enabled"],
                "properties": {
                    "enabled": {
                        "type": "boolean",
                        "description": "Whether UPnP is enabled"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_get_identity".to_string(),
            description: "Get local user hash, secure identification status, and RSA public key fingerprint.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_rotate_identity".to_string(),
            description: "Generate a new cryptographic user identity (user hash and RSA keypair).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_list_credits".to_string(),
            description: "List earned peer credit records, showing remote clients, uploaded/downloaded bytes, and credit multipliers.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Max credit records to return (default: 50)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_credits_stats".to_string(),
            description: "Get overall credits statistics: total credited peers, total upload/download volume.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_list_history".to_string(),
            description: "Query download history of completed or cancelled downloads.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Filter by filename substring"
                    },
                    "status": {
                        "type": "string",
                        "description": "Filter by status: 'completed', 'cancelled', 'error'"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of history records to return"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_history_stats".to_string(),
            description: "Get aggregate historical stats: total files completed, total bytes downloaded and uploaded.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_clear_history".to_string(),
            description: "Clear all past download history entries.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_export_history".to_string(),
            description: "Export download history to JSON or CSV format string.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "format": {
                        "type": "string",
                        "enum": ["json", "csv"],
                        "description": "Export format: 'json' or 'csv' (default: 'json')"
                    }
                }
            }),
        },
    ]
}
