// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Tool definitions (schemas) for shares, sources, history, uploads, and listeners.

use crate::protocol::ToolDefinition;
use crate::tools::{obj_schema, prop};
use serde_json::json;

/// Returns the schema definitions for all share/source/network tools.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "emule_list_sources".to_string(),
            description: "List all known peer sources for a specific file hash.".to_string(),
            input_schema: obj_schema(json!({"file_hash": prop("string", "32-character hex eD2K hash")}), &["file_hash"]),
        },
        ToolDefinition {
            name: "emule_add_manual_source".to_string(),
            description: "Add a manual peer source by address 'ip:port' for a file hash.".to_string(),
            input_schema: obj_schema(json!({"file_hash": prop("string", "32-character hex eD2K hash"),
                    "address": prop("string", "Peer address as 'ip:port' (e.g. '192.168.1.50:49662')"),
                    "user_hash": prop("string", "Optional 32-character hex user hash"),
                    "crypt_options": prop("integer", "Optional encryption options flags")}), &["file_hash", "address"]),
        },
        ToolDefinition {
            name: "emule_connect_source".to_string(),
            description: "Force immediate connection to a specific peer source for a file.".to_string(),
            input_schema: obj_schema(json!({"file_hash": prop("string", "32-character hex eD2K hash"),
                    "address": prop("string", "Peer address as 'ip:port'")}), &["file_hash", "address"]),
        },
        ToolDefinition {
            name: "emule_disconnect_source".to_string(),
            description: "Disconnect from a specific peer source or the active peer.".to_string(),
            input_schema: obj_schema(json!({"address": prop("string", "Peer address to disconnect (optional, disconnects current peer if omitted)")}), &[]),
        },
        ToolDefinition {
            name: "emule_clear_sources".to_string(),
            description: "Clear all discovered peer sources for a file hash.".to_string(),
            input_schema: obj_schema(json!({"file_hash": prop("string", "32-character hex eD2K hash")}), &["file_hash"]),
        },
        ToolDefinition {
            name: "emule_swarm_status".to_string(),
            description: "Get full swarm status: active peers count, in-flight blocks, chunk distribution.".to_string(),
            input_schema: obj_schema(json!({"file_hash": prop("string", "Optional file hash to get swarm status for")}), &[]),
        },
        ToolDefinition {
            name: "emule_list_shares".to_string(),
            description: "List all files currently shared with the eD2K/Kad network.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_add_share".to_string(),
            description: "Share a file with other eD2K/Kad peers.".to_string(),
            input_schema: obj_schema(json!({"file_hash": prop("string", "32-character hex eD2K hash"),
                    "file_name": prop("string", "Name of the file"),
                    "file_size": prop("integer", "File size in bytes"),
                    "file_path": prop("string", "Optional disk path to the file")}), &["file_hash", "file_name", "file_size"]),
        },
        ToolDefinition {
            name: "emule_remove_share".to_string(),
            description: "Unshare a file from the eD2K/Kad network, optionally deleting the file on disk.".to_string(),
            input_schema: obj_schema(json!({"file_hash": prop("string", "32-character hex eD2K hash of shared file"),
                    "delete_file": prop("boolean", "Also delete the file on disk (for a completed download this removes the download entry too). Rejected for partial shares of in-progress downloads (default: false)")}), &["file_hash"]),
        },
        ToolDefinition {
            name: "emule_rename_share".to_string(),
            description: "Rename a fully shared file: the file on disk and its share entry are renamed (and the linked completed download when one exists).".to_string(),
            input_schema: obj_schema(json!({"file_hash": prop("string", "32-character hex eD2K hash of shared file"),
                    "new_name": prop("string", "New file name (no path separators or Windows-illegal characters)")}), &["file_hash", "new_name"]),
        },
        ToolDefinition {
            name: "emule_set_share_enabled".to_string(),
            description: "Enable or disable sharing for a file without removing it from the share list.".to_string(),
            input_schema: obj_schema(json!({"file_hash": prop("string", "32-character hex eD2K hash"),
                    "enabled": prop("boolean", "Whether file sharing is enabled")}), &["file_hash", "enabled"]),
        },
        ToolDefinition {
            name: "emule_list_uploads".to_string(),
            description: "List active and queued upload transfers to remote peers.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_cancel_upload".to_string(),
            description: "Cancel/release an active upload transfer slot for a remote client address.".to_string(),
            input_schema: obj_schema(json!({"client_address": prop("string", "Remote client address as 'ip:port'")}), &["client_address"]),
        },
        ToolDefinition {
            name: "emule_start_listener".to_string(),
            description: "Start the TCP inbound peer connection listener.".to_string(),
            input_schema: obj_schema(json!({"port": prop("integer", "TCP listening port (default: 39735)"),
                    "bind_ip": prop("string", "IP interface to bind (default: '127.0.0.1')")}), &[]),
        },
        ToolDefinition {
            name: "emule_stop_listener".to_string(),
            description: "Stop the TCP inbound peer connection listener.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_listener_status".to_string(),
            description: "Get TCP inbound listener status, bound address, and active connection count.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_upnp_status".to_string(),
            description: "Get UPnP automatic router port forwarding status, active mappings, and external IP.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_upnp_map_ports".to_string(),
            description: "Request UPnP port forwarding on your router for TCP and UDP eMule ports.".to_string(),
            input_schema: obj_schema(json!({"tcp_port": prop("integer", "TCP port to forward (optional)"),
                    "udp_port": prop("integer", "UDP port to forward (optional)")}), &[]),
        },
        ToolDefinition {
            name: "emule_upnp_unmap_ports".to_string(),
            description: "Remove UPnP port forwarding rules from the router.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_upnp_set_enabled".to_string(),
            description: "Enable or disable automatic UPnP port forwarding.".to_string(),
            input_schema: obj_schema(json!({"enabled": prop("boolean", "Whether UPnP is enabled")}), &["enabled"]),
        },
        ToolDefinition {
            name: "emule_get_identity".to_string(),
            description: "Get local user hash, secure identification status, and RSA public key fingerprint.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_rotate_identity".to_string(),
            description: "Generate a new cryptographic user identity (user hash and RSA keypair).".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_list_credits".to_string(),
            description: "List earned peer credit records, showing remote clients, uploaded/downloaded bytes, and credit multipliers.".to_string(),
            input_schema: obj_schema(json!({"limit": prop("integer", "Max credit records to return (default: 50)")}), &[]),
        },
        ToolDefinition {
            name: "emule_credits_stats".to_string(),
            description: "Get overall credits statistics: total credited peers, total upload/download volume.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_list_history".to_string(),
            description: "Query download history of completed or cancelled downloads.".to_string(),
            input_schema: obj_schema(json!({"query": prop("string", "Filter by filename substring"),
                    "status": prop("string", "Filter by status: 'completed', 'cancelled', 'error'"),
                    "limit": prop("integer", "Maximum number of history records to return")}), &[]),
        },
        ToolDefinition {
            name: "emule_history_stats".to_string(),
            description: "Get aggregate historical stats: total files completed, total bytes downloaded and uploaded.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_clear_history".to_string(),
            description: "Clear all past download history entries.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_export_history".to_string(),
            description: "Export download history to JSON or CSV format string.".to_string(),
            input_schema: obj_schema(json!({"format": {
                        "type": "string",
                        "enum": ["json", "csv"],
                        "description": "Export format: 'json' or 'csv' (default: 'json')"
                    }}), &[]),
        },
    ]
}
