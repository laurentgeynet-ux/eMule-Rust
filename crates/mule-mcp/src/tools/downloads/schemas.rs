// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Tool schemas (definitions) for downloads and categories.

use crate::protocol::ToolDefinition;
use crate::tools::{obj_schema, prop};
use serde_json::json;

/// Returns tool definitions for download and category management.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "emule_list_downloads".to_string(),
            description: "List all files currently in the download queue with their ID, hash, filename, progress, state, size, downloaded bytes, and speed.".to_string(),
            input_schema: obj_schema(json!({"state_filter": {
                        "type": "string",
                        "description": "Optional filter: 'all', 'active', 'paused', 'completed', 'error'",
                        "enum": ["all", "active", "paused", "completed", "error"]
                    }}), &[]),
        },
        ToolDefinition {
            name: "emule_get_download_details".to_string(),
            description: "Get in-depth details for a download: completed parts map, part.met status, sources summary, destination, and transfer errors.".to_string(),
            input_schema: obj_schema(json!({"id_or_hash": prop("string", "Download UUID (e.g. 550e8400-...) or 32-character eD2K hex hash")}), &["id_or_hash"]),
        },
        ToolDefinition {
            name: "emule_add_link".to_string(),
            description: "Add an eD2K link (e.g. ed2k://|file|filename.iso|1234567|HASH|/) to the download queue.".to_string(),
            input_schema: obj_schema(json!({"link": prop("string", "The full ed2k:// link to download"),
                    "category_id": prop("string", "Optional category ID to assign this download to"),
                    "destination_dir": prop("string", "Optional custom target directory for completed file")}), &["link"]),
        },
        ToolDefinition {
            name: "emule_pause_download".to_string(),
            description: "Pause an active or pending download using its UUID or file hash.".to_string(),
            input_schema: obj_schema(json!({"id_or_hash": prop("string", "Download UUID or 32-character eD2K hex hash")}), &["id_or_hash"]),
        },
        ToolDefinition {
            name: "emule_resume_download".to_string(),
            description: "Resume or start a paused download using its UUID or file hash.".to_string(),
            input_schema: obj_schema(json!({"id_or_hash": prop("string", "Download UUID or 32-character eD2K hex hash")}), &["id_or_hash"]),
        },
        ToolDefinition {
            name: "emule_remove_download".to_string(),
            description: "Remove or cancel a download from queue, optionally deleting associated files on disk.".to_string(),
            input_schema: obj_schema(json!({"id_or_hash": prop("string", "Download UUID or 32-character eD2K hex hash"),
                    "delete_files": prop("boolean", "Whether to delete on-disk files: .part and .part.met temp files; for completed downloads also the finished shared file in the destination folder (default: false)")}), &["id_or_hash"]),
        },
        ToolDefinition {
            name: "emule_rename_download".to_string(),
            description: "Rename a download's file name (in-progress or completed). For a completed download the file on disk and its share entry are renamed too; the .part temp file keeps its internal name.".to_string(),
            input_schema: obj_schema(json!({"id_or_hash": prop("string", "Download UUID or 32-character eD2K hex hash"),
                    "new_name": prop("string", "New file name (no path separators or Windows-illegal characters)")}), &["id_or_hash", "new_name"]),
        },
        ToolDefinition {
            name: "emule_set_destination".to_string(),
            description: "Change the target directory where the completed file will be moved.".to_string(),
            input_schema: obj_schema(json!({"id_or_hash": prop("string", "Download UUID or 32-character eD2K hex hash"),
                    "destination_dir": prop("string", "Target folder path for completed download")}), &["id_or_hash", "destination_dir"]),
        },
        ToolDefinition {
            name: "emule_download_set_category".to_string(),
            description: "Assign or clear the category for a specific download.".to_string(),
            input_schema: obj_schema(json!({"id_or_hash": prop("string", "Download UUID or 32-character eD2K hex hash"),
                    "category_id": prop("string", "Category ID, or empty string / null to unassign")}), &["id_or_hash"]),
        },
        ToolDefinition {
            name: "emule_retry_download".to_string(),
            description: "Force reconnection and re-query sources for a stalled, error, or waiting download.".to_string(),
            input_schema: obj_schema(json!({"id_or_hash": prop("string", "Download UUID or 32-character eD2K hex hash")}), &["id_or_hash"]),
        },
        ToolDefinition {
            name: "emule_inspect_part_met".to_string(),
            description: "Inspect an existing eMule .part.met file on disk to examine its name, hash, size, parts, and corruption state.".to_string(),
            input_schema: obj_schema(json!({"met_path": prop("string", "Absolute or relative path to the .part.met file"),
                    "part_path": prop("string", "Optional path to the accompanying .part file")}), &["met_path"]),
        },
        ToolDefinition {
            name: "emule_import_part_met".to_string(),
            description: "Import a legacy or foreign .part.met file into the active eMule-Rust download queue.".to_string(),
            input_schema: obj_schema(json!({"met_path": prop("string", "Path to the .part.met file to import"),
                    "part_path": prop("string", "Optional path to the .part data file"),
                    "destination_dir": prop("string", "Optional custom directory for completed file"),
                    "category_id": prop("string", "Optional category ID")}), &["met_path"]),
        },
        ToolDefinition {
            name: "emule_list_categories".to_string(),
            description: "List all download categories with their ID, name, color, destination, and priority.".to_string(),
            input_schema: obj_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: "emule_create_category".to_string(),
            description: "Create a new download category with custom color and default target folder.".to_string(),
            input_schema: obj_schema(json!({"name": prop("string", "Category name (e.g. 'Movies', 'ISO', 'Music')"),
                    "color": prop("string", "Hex color code (e.g. '#2196F3')"),
                    "default_destination": prop("string", "Optional folder path where completed files of this category will be placed"),
                    "default_priority": prop("string", "Optional default priority ('low', 'normal', 'high')")}), &["name"]),
        },
        ToolDefinition {
            name: "emule_update_category".to_string(),
            description: "Update an existing category's properties.".to_string(),
            input_schema: obj_schema(json!({"id": prop("string", "Category ID"),
                    "name": prop("string", "New category name"),
                    "color": prop("string", "New hex color code"),
                    "default_destination": prop("string", "New default destination folder"),
                    "default_priority": prop("string", "New default priority"),
                    "sort_order": prop("integer", "Sort order position index")}), &["id"]),
        },
        ToolDefinition {
            name: "emule_delete_category".to_string(),
            description: "Delete a category. Downloads previously assigned to it are automatically unassigned.".to_string(),
            input_schema: obj_schema(json!({"id": prop("string", "Category ID to delete")}), &["id"]),
        },
        ToolDefinition {
            name: "emule_reorder_categories".to_string(),
            description: "Reorder categories by providing an ordered list of category IDs.".to_string(),
            input_schema: obj_schema(json!({"category_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Ordered array of category IDs"
                    }}), &["category_ids"]),
        },
    ]
}
