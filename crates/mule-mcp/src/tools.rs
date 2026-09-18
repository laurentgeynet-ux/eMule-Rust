use crate::client::DaemonClient;
use crate::protocol::{ToolCallResult, ToolDefinition};
use serde_json::{json, Value};
use tracing::info;

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        // ==========================================
        // 1. SYSTEM & GLOBAL CONTROLS
        // ==========================================
        ToolDefinition {
            name: "emule_status".to_string(),
            description: "Get overall Emule-Sion daemon status, uptime, transfer speeds, active downloads count, server connection, and Kad status.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_get_settings".to_string(),
            description: "Get Emule-Sion daemon configuration, default paths, limits, and retention policy.".to_string(),
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
            description: "Gracefully shut down the Emule-Sion background daemon.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },

        // ==========================================
        // 2. DOWNLOADS QUEUE & FILE TRANSFERS
        // ==========================================
        ToolDefinition {
            name: "emule_list_downloads".to_string(),
            description: "List all files currently in the download queue with their ID, hash, filename, progress, state, size, downloaded bytes, and speed.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "state_filter": {
                        "type": "string",
                        "description": "Optional filter: 'all', 'active', 'paused', 'completed', 'error'",
                        "enum": ["all", "active", "paused", "completed", "error"]
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_get_download_details".to_string(),
            description: "Get in-depth details for a download: completed parts map, part.met status, sources summary, destination, and transfer errors.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["id_or_hash"],
                "properties": {
                    "id_or_hash": {
                        "type": "string",
                        "description": "Download UUID (e.g. 550e8400-...) or 32-character eD2K hex hash"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_add_link".to_string(),
            description: "Add an eD2K link (e.g. ed2k://|file|filename.iso|1234567|HASH|/) to the download queue.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["link"],
                "properties": {
                    "link": {
                        "type": "string",
                        "description": "The full ed2k:// link to download"
                    },
                    "category_id": {
                        "type": "string",
                        "description": "Optional category ID to assign this download to"
                    },
                    "destination_dir": {
                        "type": "string",
                        "description": "Optional custom target directory for completed file"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_pause_download".to_string(),
            description: "Pause an active or pending download using its UUID or file hash.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["id_or_hash"],
                "properties": {
                    "id_or_hash": {
                        "type": "string",
                        "description": "Download UUID or 32-character eD2K hex hash"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_resume_download".to_string(),
            description: "Resume or start a paused download using its UUID or file hash.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["id_or_hash"],
                "properties": {
                    "id_or_hash": {
                        "type": "string",
                        "description": "Download UUID or 32-character eD2K hex hash"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_remove_download".to_string(),
            description: "Remove or cancel a download from queue, optionally deleting partially downloaded data.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["id_or_hash"],
                "properties": {
                    "id_or_hash": {
                        "type": "string",
                        "description": "Download UUID or 32-character eD2K hex hash"
                    },
                    "delete_files": {
                        "type": "boolean",
                        "description": "Whether to delete .part and .part.met files on disk (default: false)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_set_destination".to_string(),
            description: "Change the target directory where the completed file will be moved.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["id_or_hash", "destination_dir"],
                "properties": {
                    "id_or_hash": {
                        "type": "string",
                        "description": "Download UUID or 32-character eD2K hex hash"
                    },
                    "destination_dir": {
                        "type": "string",
                        "description": "Target folder path for completed download"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_download_set_category".to_string(),
            description: "Assign or clear the category for a specific download.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["id_or_hash"],
                "properties": {
                    "id_or_hash": {
                        "type": "string",
                        "description": "Download UUID or 32-character eD2K hex hash"
                    },
                    "category_id": {
                        "type": "string",
                        "description": "Category ID, or empty string / null to unassign"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_retry_download".to_string(),
            description: "Force reconnection and re-query sources for a stalled, error, or waiting download.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["id_or_hash"],
                "properties": {
                    "id_or_hash": {
                        "type": "string",
                        "description": "Download UUID or 32-character eD2K hex hash"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_inspect_part_met".to_string(),
            description: "Inspect an existing eMule .part.met file on disk to examine its name, hash, size, parts, and corruption state.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["met_path"],
                "properties": {
                    "met_path": {
                        "type": "string",
                        "description": "Absolute or relative path to the .part.met file"
                    },
                    "part_path": {
                        "type": "string",
                        "description": "Optional path to the accompanying .part file"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_import_part_met".to_string(),
            description: "Import a legacy or foreign .part.met file into the active Emule-Sion download queue.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["met_path"],
                "properties": {
                    "met_path": {
                        "type": "string",
                        "description": "Path to the .part.met file to import"
                    },
                    "part_path": {
                        "type": "string",
                        "description": "Optional path to the .part data file"
                    },
                    "destination_dir": {
                        "type": "string",
                        "description": "Optional custom directory for completed file"
                    },
                    "category_id": {
                        "type": "string",
                        "description": "Optional category ID"
                    }
                }
            }),
        },

        // ==========================================
        // 3. CATEGORIES MANAGEMENT
        // ==========================================
        ToolDefinition {
            name: "emule_list_categories".to_string(),
            description: "List all download categories with their ID, name, color, destination, and priority.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_create_category".to_string(),
            description: "Create a new download category with custom color and default target folder.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Category name (e.g. 'Movies', 'ISO', 'Music')"
                    },
                    "color": {
                        "type": "string",
                        "description": "Hex color code (e.g. '#2196F3')"
                    },
                    "default_destination": {
                        "type": "string",
                        "description": "Optional folder path where completed files of this category will be placed"
                    },
                    "default_priority": {
                        "type": "string",
                        "description": "Optional default priority ('low', 'normal', 'high')"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_update_category".to_string(),
            description: "Update an existing category's properties.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Category ID"
                    },
                    "name": {
                        "type": "string",
                        "description": "New category name"
                    },
                    "color": {
                        "type": "string",
                        "description": "New hex color code"
                    },
                    "default_destination": {
                        "type": "string",
                        "description": "New default destination folder"
                    },
                    "default_priority": {
                        "type": "string",
                        "description": "New default priority"
                    },
                    "sort_order": {
                        "type": "integer",
                        "description": "Sort order position index"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_delete_category".to_string(),
            description: "Delete a category. Downloads previously assigned to it are automatically unassigned.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Category ID to delete"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_reorder_categories".to_string(),
            description: "Reorder categories by providing an ordered list of category IDs.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["category_ids"],
                "properties": {
                    "category_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Ordered array of category IDs"
                    }
                }
            }),
        },

        // ==========================================
        // 4. HISTORY & STATS
        // ==========================================
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

        // ==========================================
        // 5. SEARCH
        // ==========================================
        ToolDefinition {
            name: "emule_search".to_string(),
            description: "Search for files on connected ED2K servers and Kad DHT network. Automatically waits for and returns search results including synthesized ed2k links ready for download.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Keywords to search for"
                    },
                    "network": {
                        "type": "string",
                        "description": "Network to search on: 'kad', 'server', or 'all' (default: 'kad')",
                        "enum": ["kad", "server", "all"],
                        "default": "kad"
                    },
                    "min_size_mb": {
                        "type": "number",
                        "description": "Minimum file size in Megabytes (optional)"
                    },
                    "max_size_mb": {
                        "type": "number",
                        "description": "Maximum file size in Megabytes (optional)"
                    },
                    "extension": {
                        "type": "string",
                        "description": "File extension filter, e.g. 'iso', 'zip', 'mp4' (optional)"
                    },
                    "wait_seconds": {
                        "type": "integer",
                        "description": "Number of seconds to wait and collect search results (default: 4)",
                        "default": 4
                    }
                }
            }),
        },

        // ==========================================
        // 6. SERVERS MANAGEMENT
        // ==========================================
        ToolDefinition {
            name: "emule_list_servers".to_string(),
            description: "List known ED2K servers (name, IP, port, ping, users, files, connection state, filter status).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_connect_server".to_string(),
            description: "Connect to an ED2K server. If IP and port are omitted, connects to the best available server.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ip": {
                        "type": "string",
                        "description": "Server IP address (optional)"
                    },
                    "port": {
                        "type": "integer",
                        "description": "Server port (optional)"
                    },
                    "address": {
                        "type": "string",
                        "description": "Full server 'ip:port' address (optional)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_disconnect_server".to_string(),
            description: "Disconnect from the currently connected ED2K server.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_update_servers_from_url".to_string(),
            description: "Download and update ED2K server list from a remote server.met URL (e.g. peerates or gruk).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL to server.met (default: 'http://upd.emule-security.org/server.met')"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_inspect_server_met".to_string(),
            description: "Inspect an offline server.met file on disk to view its servers and IP filter compliance before importing.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_path"],
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to server.met file"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_import_server_met".to_string(),
            description: "Import servers from a local server.met file into the server list.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_path"],
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to server.met file"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_set_server_enabled".to_string(),
            description: "Enable or disable an ED2K server in the server list.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["address", "enabled"],
                "properties": {
                    "address": {
                        "type": "string",
                        "description": "Server address (e.g. '176.123.5.4:4661')"
                    },
                    "enabled": {
                        "type": "boolean",
                        "description": "Whether the server is enabled"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_delete_server".to_string(),
            description: "Delete an ED2K server from the server list.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["address"],
                "properties": {
                    "address": {
                        "type": "string",
                        "description": "Server address (e.g. '176.123.5.4:4661')"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_clear_servers".to_string(),
            description: "Clear all servers from the server list.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_query_server_sources".to_string(),
            description: "Ask the connected ED2K server for sources of a specific file hash.".to_string(),
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

        // ==========================================
        // 7. SWARM & PEER SOURCES
        // ==========================================
        ToolDefinition {
            name: "emule_list_sources".to_string(),
            description: "List discovered peer sources and swarm status for a specific file hash.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character eD2K hex hash of the file"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_add_manual_source".to_string(),
            description: "Add a known peer IP:port as a manual download source for a file hash.".to_string(),
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

        // ==========================================
        // 8. SHARES & UPLOADS
        // ==========================================
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
            description: "Unshare a file from the eD2K/Kad network.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_hash"],
                "properties": {
                    "file_hash": {
                        "type": "string",
                        "description": "32-character hex eD2K hash of shared file"
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

        // ==========================================
        // 9. INBOUND LISTENER & UPNP PORT MAPPING
        // ==========================================
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

        // ==========================================
        // 10. KAD DHT ADVANCED CONTROLS
        // ==========================================
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

        // ==========================================
        // 11. NETWORK POLICY & SECURITY ISOLATION
        // ==========================================
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
            description: "Configure network resource budgets (max bandwidth, max sockets).".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["budgets"],
                "properties": {
                    "budgets": {
                        "type": "object",
                        "description": "Resource budgets object"
                    }
                }
            }),
        },

        // ==========================================
        // 12. IP FILTER (FIREWALL & BLOCKLIST)
        // ==========================================
        ToolDefinition {
            name: "emule_get_filter_status".to_string(),
            description: "Get IP filter status: enabled/disabled, total loaded rules, blocked connection attempts.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "emule_import_ip_filter".to_string(),
            description: "Load an IP filter / blocklist file (.dat, .p2p, or text format) into the active firewall.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["file_path"],
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to ipfilter.dat / guardian.p2p file"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "emule_set_filter_enabled".to_string(),
            description: "Enable or disable the IP filter firewall.".to_string(),
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

        // ==========================================
        // 13. IDENTITY & PEER CREDITS
        // ==========================================
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
    ]
}

async fn resolve_download_id(client: &DaemonClient, id_or_hash: &str) -> Result<String, String> {
    // If it has hyphens and parses as UUID, it's definitely a download instance UUID
    if id_or_hash.contains('-') && uuid::Uuid::parse_str(id_or_hash).is_ok() {
        return Ok(id_or_hash.to_string());
    }

    let list_res = client.call("downloads.list", json!({})).await?;
    if let Some(downloads) = list_res.get("downloads").and_then(|d| d.as_array()) {
        for dl in downloads {
            if let Some(id) = dl.get("id").and_then(|i| i.as_str()) {
                // Match by id directly
                if id.eq_ignore_ascii_case(id_or_hash) {
                    return Ok(id.to_string());
                }
                // Match by fileName
                if let Some(name) = dl.get("fileName").and_then(|n| n.as_str()) {
                    if name.eq_ignore_ascii_case(id_or_hash) {
                        return Ok(id.to_string());
                    }
                }
                // Match by ed2kHash in detail
                if let Ok(detail) = client.call("downloads.get", json!({ "id": id })).await {
                    if let Some(hash) = detail.get("ed2kHash").and_then(|h| h.as_str()) {
                        if hash.eq_ignore_ascii_case(id_or_hash) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }
    }

    // Fallback if no match found in active downloads
    if uuid::Uuid::parse_str(id_or_hash).is_ok() {
        return Ok(id_or_hash.to_string());
    }

    Err(format!("No download found matching ID, filename, or file hash '{id_or_hash}'"))
}

pub async fn execute_tool(
    client: &DaemonClient,
    tool_name: &str,
    args: Value,
) -> ToolCallResult {
    info!("Executing MCP tool: {tool_name} with args: {args}");
    match tool_name {
        // ==========================================
        // 1. SYSTEM & GLOBAL CONTROLS
        // ==========================================
        "emule_status" => {
            match client.call("system.status", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to retrieve status: {e}")),
            }
        }

        "emule_get_settings" => {
            match client.call("settings.get", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get settings: {e}")),
            }
        }

        "emule_update_settings" => {
            let mut params = json!({});
            if let Some(d) = args.get("default_download_dir").and_then(|s| s.as_str()) {
                params["defaultDownloadDir"] = json!(d);
            }
            if let Some(r) = args.get("history_retention_days").and_then(|s| s.as_u64()) {
                params["historyRetentionDays"] = json!(r);
            }
            if let Some(g) = args.get("max_global_download_peers").and_then(|s| s.as_u64()) {
                params["maxGlobalDownloadPeers"] = json!(g);
            }
            if let Some(p) = args.get("max_peers_per_download").and_then(|s| s.as_u64()) {
                params["maxPeersPerDownload"] = json!(p);
            }
            if let Some(m) = args.get("min_peers_per_download").and_then(|s| s.as_u64()) {
                params["minPeersPerDownload"] = json!(m);
            }

            match client.call("settings.set", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to update settings: {e}")),
            }
        }

        "emule_reset_download_dir" => {
            match client.call("settings.resetDownloadDir", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to reset download directory: {e}")),
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
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to set rate limits: {e}")),
            }
        }

        "emule_shutdown" => {
            match client.call("system.stop", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to shut down daemon: {e}")),
            }
        }

        // ==========================================
        // 2. DOWNLOADS QUEUE & FILE TRANSFERS
        // ==========================================
        "emule_list_downloads" => {
            match client.call("downloads.list", json!({})).await {
                Ok(mut res) => {
                    let filter = args.get("state_filter")
                        .and_then(|s| s.as_str())
                        .unwrap_or("all");

                    if filter != "all" {
                        if let Some(downloads) = res.get("downloads").and_then(|d| d.as_array()) {
                            let filtered: Vec<Value> = downloads.iter().filter(|d| {
                                let status = d.get("status").and_then(|s| s.as_str()).unwrap_or("");
                                match filter {
                                    "active" => status == "Downloading" || status == "Connecting" || status == "Pending",
                                    "paused" => status == "Paused" || status == "Stopped",
                                    "completed" => status == "Completed" || status == "Complete",
                                    "error" => status == "Error" || status.contains("Error"),
                                    _ => true,
                                }
                            }).cloned().collect();
                            res["downloads"] = json!(filtered);
                            res["filteredBy"] = json!(filter);
                        }
                    }
                    ToolCallResult::json(&res)
                }
                Err(e) => ToolCallResult::error(format!("Failed to list downloads: {e}")),
            }
        }

        "emule_get_download_details" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return ToolCallResult::error("Missing required parameter: 'id_or_hash'"),
            };

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return ToolCallResult::error(e),
            };

            match client.call("downloads.get", json!({ "id": id })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get download details: {e}")),
            }
        }

        "emule_add_link" => {
            let link = match args.get("link").and_then(|l| l.as_str()) {
                Some(l) => l,
                None => return ToolCallResult::error("Missing required parameter: 'link'"),
            };

            let mut params = json!({ "link": link });
            if let Some(cat) = args.get("category_id").and_then(|c| c.as_str()) {
                params["categoryId"] = json!(cat);
            }
            if let Some(dest) = args.get("destination_dir").and_then(|d| d.as_str()) {
                params["targetDirectory"] = json!(dest);
            }

            match client.call("downloads.addEd2kLink", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to add ed2k link: {e}")),
            }
        }

        "emule_pause_download" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return ToolCallResult::error("Missing required parameter: 'id_or_hash'"),
            };

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return ToolCallResult::error(e),
            };

            match client.call("downloads.pause", json!({ "id": id })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to pause download: {e}")),
            }
        }

        "emule_resume_download" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return ToolCallResult::error("Missing required parameter: 'id_or_hash'"),
            };

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return ToolCallResult::error(e),
            };

            match client.call("downloads.resume", json!({ "id": id })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to resume download: {e}")),
            }
        }

        "emule_remove_download" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return ToolCallResult::error("Missing required parameter: 'id_or_hash'"),
            };
            let delete_files = args.get("delete_files").and_then(|d| d.as_bool()).unwrap_or(false);

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return ToolCallResult::error(e),
            };

            match client.call("downloads.remove", json!({ "id": id, "deleteFiles": delete_files })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to remove download: {e}")),
            }
        }

        "emule_set_destination" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return ToolCallResult::error("Missing required parameter: 'id_or_hash'"),
            };
            let destination_dir = match args.get("destination_dir").and_then(|d| d.as_str()) {
                Some(d) => d,
                None => return ToolCallResult::error("Missing required parameter: 'destination_dir'"),
            };

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return ToolCallResult::error(e),
            };

            match client.call("downloads.setDestination", json!({ "id": id, "targetDirectory": destination_dir })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to set destination directory: {e}")),
            }
        }

        "emule_download_set_category" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return ToolCallResult::error("Missing required parameter: 'id_or_hash'"),
            };
            let category_id = args.get("category_id").and_then(|c| c.as_str());

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return ToolCallResult::error(e),
            };

            let mut params = json!({ "id": id });
            if let Some(cat) = category_id {
                if !cat.trim().is_empty() {
                    params["categoryId"] = json!(cat);
                }
            }

            match client.call("downloads.setCategory", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to set download category: {e}")),
            }
        }

        "emule_retry_download" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return ToolCallResult::error("Missing required parameter: 'id_or_hash'"),
            };

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return ToolCallResult::error(e),
            };

            match client.call("downloads.retryConnection", json!({ "id": id })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to retry download connection: {e}")),
            }
        }

        "emule_inspect_part_met" => {
            let met_path = match args.get("met_path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return ToolCallResult::error("Missing required parameter: 'met_path'"),
            };

            let mut params = json!({ "metPath": met_path });
            if let Some(part) = args.get("part_path").and_then(|p| p.as_str()) {
                params["partPath"] = json!(part);
            }

            match client.call("downloads.inspectPartMet", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to inspect .part.met: {e}")),
            }
        }

        "emule_import_part_met" => {
            let met_path = match args.get("met_path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return ToolCallResult::error("Missing required parameter: 'met_path'"),
            };

            let mut params = json!({ "metPath": met_path });
            if let Some(part) = args.get("part_path").and_then(|p| p.as_str()) {
                params["partPath"] = json!(part);
            }
            if let Some(dest) = args.get("destination_dir").and_then(|d| d.as_str()) {
                params["targetDirectory"] = json!(dest);
            }
            if let Some(cat) = args.get("category_id").and_then(|c| c.as_str()) {
                params["categoryId"] = json!(cat);
            }

            match client.call("downloads.importPartMet", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to import .part.met: {e}")),
            }
        }

        // ==========================================
        // 3. CATEGORIES MANAGEMENT
        // ==========================================
        "emule_list_categories" => {
            match client.call("categories.list", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to list categories: {e}")),
            }
        }

        "emule_create_category" => {
            let name = match args.get("name").and_then(|n| n.as_str()) {
                Some(n) => n,
                None => return ToolCallResult::error("Missing required parameter: 'name'"),
            };

            let mut params = json!({ "name": name });
            if let Some(c) = args.get("color").and_then(|s| s.as_str()) {
                params["color"] = json!(c);
            }
            if let Some(d) = args.get("default_destination").and_then(|s| s.as_str()) {
                params["defaultDestination"] = json!(d);
            }
            if let Some(p) = args.get("default_priority").and_then(|s| s.as_str()) {
                params["defaultPriority"] = json!(p);
            }

            match client.call("categories.create", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to create category: {e}")),
            }
        }

        "emule_update_category" => {
            let id = match args.get("id").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return ToolCallResult::error("Missing required parameter: 'id'"),
            };

            let mut params = json!({ "id": id });
            if let Some(n) = args.get("name").and_then(|s| s.as_str()) {
                params["name"] = json!(n);
            }
            if let Some(c) = args.get("color").and_then(|s| s.as_str()) {
                params["color"] = json!(c);
            }
            if let Some(d) = args.get("default_destination").and_then(|s| s.as_str()) {
                params["defaultDestination"] = json!(d);
            }
            if let Some(p) = args.get("default_priority").and_then(|s| s.as_str()) {
                params["defaultPriority"] = json!(p);
            }
            if let Some(o) = args.get("sort_order").and_then(|s| s.as_i64()) {
                params["sortOrder"] = json!(o);
            }

            match client.call("categories.update", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to update category: {e}")),
            }
        }

        "emule_delete_category" => {
            let id = match args.get("id").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return ToolCallResult::error("Missing required parameter: 'id'"),
            };

            match client.call("categories.delete", json!({ "id": id })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to delete category: {e}")),
            }
        }

        "emule_reorder_categories" => {
            let ids = match args.get("category_ids").and_then(|a| a.as_array()) {
                Some(a) => a,
                None => return ToolCallResult::error("Missing required parameter: 'category_ids'"),
            };

            match client.call("categories.reorder", json!({ "categoryIds": ids })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to reorder categories: {e}")),
            }
        }

        // ==========================================
        // 4. HISTORY & STATS
        // ==========================================
        "emule_list_history" => {
            let mut params = json!({});
            if let Some(q) = args.get("query").and_then(|s| s.as_str()) {
                params["query"] = json!(q);
            }
            if let Some(st) = args.get("status").and_then(|s| s.as_str()) {
                params["status"] = json!(st);
            }
            if let Some(l) = args.get("limit").and_then(|s| s.as_u64()) {
                params["limit"] = json!(l);
            }

            match client.call("history.list", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to list history: {e}")),
            }
        }

        "emule_history_stats" => {
            match client.call("history.stats", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get history stats: {e}")),
            }
        }

        "emule_clear_history" => {
            match client.call("history.clear", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to clear history: {e}")),
            }
        }

        "emule_export_history" => {
            let format = args.get("format").and_then(|f| f.as_str()).unwrap_or("json");
            match client.call("history.export", json!({ "format": format })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to export history: {e}")),
            }
        }

        // ==========================================
        // 5. SEARCH
        // ==========================================
        "emule_search" => {
            let query = match args.get("query").and_then(|q| q.as_str()) {
                Some(q) => q,
                None => return ToolCallResult::error("Missing required parameter: 'query'"),
            };

            let network = args.get("network").and_then(|n| n.as_str()).unwrap_or("kad");
            let min_size = args.get("min_size_mb").and_then(|m| m.as_f64()).map(|m| (m * 1024.0 * 1024.0) as u64);
            let max_size = args.get("max_size_mb").and_then(|m| m.as_f64()).map(|m| (m * 1024.0 * 1024.0) as u64);
            let extension = args.get("extension").and_then(|e| e.as_str());
            let wait_secs = args.get("wait_seconds").and_then(|w| w.as_u64()).unwrap_or(4);

            match client.search_and_collect(query, Some(network), min_size, max_size, extension, wait_secs).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Search failed: {e}")),
            }
        }

        // ==========================================
        // 6. SERVERS MANAGEMENT
        // ==========================================
        "emule_list_servers" => {
            match client.call("servers.list", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to list servers: {e}")),
            }
        }

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
                    v.get("servers")
                        .and_then(|s| s.as_array())
                        .and_then(|arr| {
                            arr.iter().find(|s| {
                                s.get("filterStatus").and_then(|f| f.as_str()) == Some("allowed")
                            }).and_then(|s| s.get("address").and_then(|a| a.as_str()).map(String::from))
                        })
                })
            };

            let addr_str = match address {
                Some(a) => a,
                None => return ToolCallResult::error("No server address specified and no available server found in server list"),
            };

            match client.call("servers.connect", json!({ "address": addr_str })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to connect to server '{addr_str}': {e}")),
            }
        }

        "emule_disconnect_server" => {
            match client.call("servers.disconnect", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to disconnect server: {e}")),
            }
        }

        "emule_update_servers_from_url" => {
            let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("http://upd.emule-security.org/server.met");
            match client.call("servers.updateFromUrl", json!({ "url": url })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to update servers from URL: {e}")),
            }
        }

        "emule_inspect_server_met" => {
            let file_path = match args.get("file_path").and_then(|f| f.as_str()) {
                Some(f) => f,
                None => return ToolCallResult::error("Missing required parameter: 'file_path'"),
            };

            match client.call("servers.inspectMet", json!({ "filePath": file_path })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to inspect server.met: {e}")),
            }
        }

        "emule_import_server_met" => {
            let file_path = match args.get("file_path").and_then(|f| f.as_str()) {
                Some(f) => f,
                None => return ToolCallResult::error("Missing required parameter: 'file_path'"),
            };

            match client.call("servers.importMet", json!({ "filePath": file_path })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to import server.met: {e}")),
            }
        }

        "emule_set_server_enabled" => {
            let address = match args.get("address").and_then(|a| a.as_str()) {
                Some(a) => a,
                None => return ToolCallResult::error("Missing required parameter: 'address'"),
            };
            let enabled = match args.get("enabled").and_then(|e| e.as_bool()) {
                Some(e) => e,
                None => return ToolCallResult::error("Missing required parameter: 'enabled'"),
            };

            match client.call("servers.setEnabled", json!({ "address": address, "enabled": enabled })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to set server enabled status: {e}")),
            }
        }

        "emule_delete_server" => {
            let address = match args.get("address").and_then(|a| a.as_str()) {
                Some(a) => a,
                None => return ToolCallResult::error("Missing required parameter: 'address'"),
            };

            match client.call("servers.delete", json!({ "address": address })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to delete server: {e}")),
            }
        }

        "emule_clear_servers" => {
            match client.call("servers.clear", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to clear server list: {e}")),
            }
        }

        "emule_query_server_sources" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return ToolCallResult::error("Missing required parameter: 'file_hash'"),
            };
            let size = args.get("file_size").and_then(|s| s.as_u64()).unwrap_or(0);

            match client.call("servers.querySources", json!({ "fileHash": hash, "fileSize": size })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to query server sources: {e}")),
            }
        }

        // ==========================================
        // 7. SWARM & PEER SOURCES
        // ==========================================
        "emule_list_sources" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return ToolCallResult::error("Missing required parameter: 'file_hash'"),
            };

            match client.call("sources.list", json!({ "fileHash": hash })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to list sources: {e}")),
            }
        }

        "emule_add_manual_source" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return ToolCallResult::error("Missing required parameter: 'file_hash'"),
            };
            let address = match args.get("address").and_then(|a| a.as_str()) {
                Some(a) => a,
                None => return ToolCallResult::error("Missing required parameter: 'address'"),
            };

            let mut params = json!({ "fileHash": hash, "address": address });
            if let Some(uh) = args.get("user_hash").and_then(|u| u.as_str()) {
                params["userHash"] = json!(uh);
            }
            if let Some(co) = args.get("crypt_options").and_then(|c| c.as_u64()) {
                params["cryptOptions"] = json!(co);
            }

            match client.call("sources.addManual", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to add manual source: {e}")),
            }
        }

        "emule_connect_source" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return ToolCallResult::error("Missing required parameter: 'file_hash'"),
            };
            let address = match args.get("address").and_then(|a| a.as_str()) {
                Some(a) => a,
                None => return ToolCallResult::error("Missing required parameter: 'address'"),
            };

            match client.call("sources.connect", json!({ "fileHash": hash, "address": address })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to connect to source: {e}")),
            }
        }

        "emule_disconnect_source" => {
            let mut params = json!({});
            if let Some(a) = args.get("address").and_then(|s| s.as_str()) {
                params["address"] = json!(a);
            }

            match client.call("sources.disconnect", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to disconnect source: {e}")),
            }
        }

        "emule_clear_sources" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return ToolCallResult::error("Missing required parameter: 'file_hash'"),
            };

            match client.call("sources.clear", json!({ "fileHash": hash })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to clear sources: {e}")),
            }
        }

        "emule_swarm_status" => {
            let mut params = json!({});
            if let Some(h) = args.get("file_hash").and_then(|s| s.as_str()) {
                params["fileHash"] = json!(h);
            }

            match client.call("sources.status", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get swarm status: {e}")),
            }
        }

        // ==========================================
        // 8. SHARES & UPLOADS
        // ==========================================
        "emule_list_shares" => {
            match client.call("shares.list", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to list shared files: {e}")),
            }
        }

        "emule_add_share" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return ToolCallResult::error("Missing required parameter: 'file_hash'"),
            };
            let name = match args.get("file_name").and_then(|n| n.as_str()) {
                Some(n) => n,
                None => return ToolCallResult::error("Missing required parameter: 'file_name'"),
            };
            let size = match args.get("file_size").and_then(|s| s.as_u64()) {
                Some(s) => s,
                None => return ToolCallResult::error("Missing required parameter: 'file_size'"),
            };
            let path = args.get("file_path").and_then(|p| p.as_str()).unwrap_or("");

            match client.call("shares.add", json!({
                "fileHash": hash,
                "fileName": name,
                "fileSize": size,
                "filePath": path
            })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to share file: {e}")),
            }
        }

        "emule_remove_share" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return ToolCallResult::error("Missing required parameter: 'file_hash'"),
            };

            match client.call("shares.remove", json!({ "fileHash": hash })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to unshare file: {e}")),
            }
        }

        "emule_set_share_enabled" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return ToolCallResult::error("Missing required parameter: 'file_hash'"),
            };
            let enabled = match args.get("enabled").and_then(|e| e.as_bool()) {
                Some(e) => e,
                None => return ToolCallResult::error("Missing required parameter: 'enabled'"),
            };

            match client.call("shares.setEnabled", json!({ "fileHash": hash, "enabled": enabled })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to set share state: {e}")),
            }
        }

        "emule_list_uploads" => {
            match client.call("uploads.list", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to list uploads: {e}")),
            }
        }

        "emule_cancel_upload" => {
            let client_address = match args.get("client_address").and_then(|c| c.as_str()) {
                Some(c) => c,
                None => return ToolCallResult::error("Missing required parameter: 'client_address'"),
            };

            match client.call("uploads.cancel", json!({ "clientAddress": client_address })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to cancel upload: {e}")),
            }
        }

        // ==========================================
        // 9. INBOUND LISTENER & UPNP PORT MAPPING
        // ==========================================
        "emule_start_listener" => {
            let mut params = json!({});
            if let Some(p) = args.get("port").and_then(|s| s.as_u64()) {
                params["port"] = json!(p);
            }
            if let Some(b) = args.get("bind_ip").and_then(|s| s.as_str()) {
                params["bindIp"] = json!(b);
            }

            match client.call("listener.start", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to start inbound listener: {e}")),
            }
        }

        "emule_stop_listener" => {
            match client.call("listener.stop", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to stop inbound listener: {e}")),
            }
        }

        "emule_listener_status" => {
            match client.call("listener.status", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get listener status: {e}")),
            }
        }

        "emule_upnp_status" => {
            match client.call("upnp.status", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get UPnP status: {e}")),
            }
        }

        "emule_upnp_map_ports" => {
            let mut params = json!({});
            if let Some(t) = args.get("tcp_port").and_then(|s| s.as_u64()) {
                params["tcpPort"] = json!(t);
            }
            if let Some(u) = args.get("udp_port").and_then(|s| s.as_u64()) {
                params["udpPort"] = json!(u);
            }

            match client.call("upnp.mapPorts", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to map UPnP ports: {e}")),
            }
        }

        "emule_upnp_unmap_ports" => {
            match client.call("upnp.unmapPorts", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to unmap UPnP ports: {e}")),
            }
        }

        "emule_upnp_set_enabled" => {
            let enabled = match args.get("enabled").and_then(|e| e.as_bool()) {
                Some(e) => e,
                None => return ToolCallResult::error("Missing required parameter: 'enabled'"),
            };

            match client.call("upnp.setEnabled", json!({ "enabled": enabled })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to set UPnP enabled state: {e}")),
            }
        }

        // ==========================================
        // 10. KAD DHT ADVANCED CONTROLS
        // ==========================================
        "emule_kad_status" => {
            match client.call("kad.status", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get Kad status: {e}")),
            }
        }

        "emule_kad_bootstrap" => {
            if let Some(url) = args.get("url").and_then(|u| u.as_str()) {
                match client.call("kad.bootstrapFromUrl", json!({ "url": url })).await {
                    Ok(res) => ToolCallResult::json(&res),
                    Err(e) => ToolCallResult::error(format!("Failed to bootstrap Kad from URL: {e}")),
                }
            } else if let (Some(ip), Some(port)) = (
                args.get("ip").and_then(|i| i.as_str()),
                args.get("port").and_then(|p| p.as_u64()),
            ) {
                match client.call("kad.bootstrap", json!({ "ip": ip, "port": port })).await {
                    Ok(res) => ToolCallResult::json(&res),
                    Err(e) => ToolCallResult::error(format!("Failed to bootstrap Kad node: {e}")),
                }
            } else {
                ToolCallResult::error("Provide either 'url' or both 'ip' and 'port' to bootstrap Kad")
            }
        }

        "emule_kad_start_udp" => {
            let mut params = json!({});
            if let Some(b) = args.get("bind_address").and_then(|s| s.as_str()) {
                params["bindAddress"] = json!(b);
            }

            match client.call("kad.startUdp", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to start Kad UDP socket: {e}")),
            }
        }

        "emule_kad_stop_udp" => {
            match client.call("kad.stopUdp", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to stop Kad UDP socket: {e}")),
            }
        }

        "emule_kad_ping" => {
            let ip = match args.get("ip").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return ToolCallResult::error("Missing required parameter: 'ip'"),
            };
            let port = match args.get("port").and_then(|p| p.as_u64()) {
                Some(p) => p,
                None => return ToolCallResult::error("Missing required parameter: 'port'"),
            };

            let mut params = json!({ "ip": ip, "port": port });
            if let Some(k) = args.get("kad_id").and_then(|s| s.as_str()) {
                params["kadId"] = json!(k);
            }

            match client.call("kad.ping", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to ping Kad contact: {e}")),
            }
        }

        "emule_kad_find_node" => {
            let target_id = match args.get("target_id").and_then(|t| t.as_str()) {
                Some(t) => t,
                None => return ToolCallResult::error("Missing required parameter: 'target_id'"),
            };

            match client.call("kad.findNode", json!({ "targetId": target_id })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to find node in Kad: {e}")),
            }
        }

        "emule_kad_list_contacts" => {
            let mut params = json!({});
            if let Some(l) = args.get("limit").and_then(|s| s.as_u64()) {
                params["limit"] = json!(l);
            }

            match client.call("kad.listContacts", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to list Kad contacts: {e}")),
            }
        }

        "emule_kad_clear_contacts" => {
            match client.call("kad.clearContacts", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to clear Kad contacts: {e}")),
            }
        }

        "emule_kad_search_sources" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return ToolCallResult::error("Missing required parameter: 'file_hash'"),
            };
            let size = args.get("file_size").and_then(|s| s.as_u64()).unwrap_or(0);

            match client.call("kad.searchSources", json!({ "fileHash": hash, "fileSize": size })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to search Kad sources: {e}")),
            }
        }

        "emule_kad_publish_source" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return ToolCallResult::error("Missing required parameter: 'file_hash'"),
            };
            let size = match args.get("file_size").and_then(|s| s.as_u64()) {
                Some(s) => s,
                None => return ToolCallResult::error("Missing required parameter: 'file_size'"),
            };

            let mut params = json!({ "fileHash": hash, "fileSize": size });
            if let Some(t) = args.get("tcp_port").and_then(|s| s.as_u64()) {
                params["tcpPort"] = json!(t);
            }

            match client.call("kad.publishSource", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to publish source to Kad: {e}")),
            }
        }

        "emule_kad_publish_keywords" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return ToolCallResult::error("Missing required parameter: 'file_hash'"),
            };
            let name = match args.get("file_name").and_then(|n| n.as_str()) {
                Some(n) => n,
                None => return ToolCallResult::error("Missing required parameter: 'file_name'"),
            };
            let size = match args.get("file_size").and_then(|s| s.as_u64()) {
                Some(s) => s,
                None => return ToolCallResult::error("Missing required parameter: 'file_size'"),
            };

            match client.call("kad.publishKeywords", json!({
                "fileHash": hash,
                "fileName": name,
                "fileSize": size
            })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to publish keywords to Kad: {e}")),
            }
        }

        "emule_kad_inspect_nodes_dat" => {
            let path = match args.get("path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return ToolCallResult::error("Missing required parameter: 'path'"),
            };

            match client.call("kad.inspectNodesDat", json!({ "path": path })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to inspect nodes.dat: {e}")),
            }
        }

        "emule_kad_import_nodes_dat" => {
            let path = match args.get("path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return ToolCallResult::error("Missing required parameter: 'path'"),
            };

            match client.call("kad.importNodesDat", json!({ "path": path })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to import nodes.dat: {e}")),
            }
        }

        // ==========================================
        // 11. NETWORK POLICY & SECURITY ISOLATION
        // ==========================================
        "emule_get_network_policy" => {
            match client.call("network.getPolicy", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get network policy: {e}")),
            }
        }

        "emule_set_network_profile" => {
            let profile = match args.get("profile").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return ToolCallResult::error("Missing required parameter: 'profile'"),
            };

            match client.call("network.setProfile", json!({ "profile": profile })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to set network profile: {e}")),
            }
        }

        "emule_network_disconnect_all" => {
            match client.call("network.disconnectAll", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to trigger network disconnect: {e}")),
            }
        }

        "emule_network_enable" => {
            match client.call("network.enable", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to enable network: {e}")),
            }
        }

        "emule_get_audit_log" => {
            let mut params = json!({});
            if let Some(l) = args.get("limit").and_then(|s| s.as_u64()) {
                params["limit"] = json!(l);
            }

            match client.call("network.getAuditLog", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get network audit log: {e}")),
            }
        }

        "emule_set_network_budgets" => {
            let budgets = match args.get("budgets") {
                Some(b) => b,
                None => return ToolCallResult::error("Missing required parameter: 'budgets'"),
            };

            match client.call("network.setBudgets", json!({ "budgets": budgets })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to set network budgets: {e}")),
            }
        }

        // ==========================================
        // 12. IP FILTER (FIREWALL & BLOCKLIST)
        // ==========================================
        "emule_get_filter_status" => {
            match client.call("filter.getStatus", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get IP filter status: {e}")),
            }
        }

        "emule_import_ip_filter" => {
            let file_path = match args.get("file_path").and_then(|f| f.as_str()) {
                Some(f) => f,
                None => return ToolCallResult::error("Missing required parameter: 'file_path'"),
            };

            match client.call("filter.importFile", json!({ "filePath": file_path })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to import IP filter: {e}")),
            }
        }

        "emule_set_filter_enabled" => {
            let enabled = match args.get("enabled").and_then(|e| e.as_bool()) {
                Some(e) => e,
                None => return ToolCallResult::error("Missing required parameter: 'enabled'"),
            };

            match client.call("filter.setEnabled", json!({ "enabled": enabled })).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to set filter enabled state: {e}")),
            }
        }

        "emule_clear_ip_filter" => {
            match client.call("filter.clear", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to clear IP filter: {e}")),
            }
        }

        // ==========================================
        // 13. IDENTITY & PEER CREDITS
        // ==========================================
        "emule_get_identity" => {
            match client.call("identity.status", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get identity status: {e}")),
            }
        }

        "emule_rotate_identity" => {
            match client.call("identity.rotate", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to rotate identity key: {e}")),
            }
        }

        "emule_list_credits" => {
            let mut params = json!({});
            if let Some(l) = args.get("limit").and_then(|s| s.as_u64()) {
                params["limit"] = json!(l);
            }

            match client.call("credits.list", params).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to list peer credits: {e}")),
            }
        }

        "emule_credits_stats" => {
            match client.call("credits.stats", json!({})).await {
                Ok(res) => ToolCallResult::json(&res),
                Err(e) => ToolCallResult::error(format!("Failed to get credits stats: {e}")),
            }
        }

        unknown => ToolCallResult::error(format!("Unknown tool: {unknown}")),
    }
}
