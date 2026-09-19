//! Tool schemas (definitions) for downloads and categories.

use crate::protocol::ToolDefinition;
use serde_json::json;

/// Returns tool definitions for download and category management.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
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
    ]
}
