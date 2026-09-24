// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use mule_core::HistoryEntry;
use mule_core::{Database, Download, DownloadCategory, HistoryFilter, HistoryStats};
use std::collections::HashMap;
use uuid::Uuid;

use super::{db_err, types::{now_ms, ActiveDownload}};
use crate::error::DaemonError;

/// eMule `IsRememberingCancelledFiles`/`IsRememberingDownloadedFiles`
/// gate — `true` unless the setting is explicitly `0`/`false`/`off`.
pub fn setting_enabled(db: &Database, key: &str) -> bool {
    db.get_setting(key)
        .ok()
        .flatten()
        .map(|v| !matches!(v.trim().to_lowercase().as_str(), "0" | "false" | "off"))
        .unwrap_or(true)
}

pub fn record_history_event(
    db: &mut Database,
    download: &Download,
    transferred_bytes: u64,
    connection_attempts: u32,
    resume_count: u32,
    event_type: &str,
    verification_result: &str,
    error_message: Option<String>,
) {
    // eMule `RememberCancelledFiles`/`RememberDownloadedFiles`: skip the
    // history entry when the corresponding toggle is disabled.
    let gate = match event_type {
        "cancelled" => Some("remember_cancelled"),
        "completed" => Some("remember_downloaded"),
        _ => None,
    };
    if let Some(key) = gate {
        if !setting_enabled(db, key) {
            return;
        }
    }
    let now = now_ms();
    let duration_secs = ((now - download.created_at) / 1000).max(0) as u64;
    let entry = HistoryEntry {
        id: 0,
        download_id: download.id.to_string(),
        file_name: download.display_name.clone(),
        total_size: download.total_size,
        transferred_bytes,
        ed2k_hash: hex::encode(download.ed2k_hash.0),
        event_type: event_type.to_string(),
        status: download.status.as_str().to_string(),
        duration_secs,
        connection_attempts,
        resume_count,
        verification_result: verification_result.to_string(),
        error_message,
        timestamp_unix_ms: now,
    };
    if let Err(e) = db.insert_history_entry(&entry) {
        tracing::warn!("Failed to insert history entry: {e}");
    }
}

pub fn query_history(
    db: &Database,
    filter: &HistoryFilter,
) -> Result<Vec<HistoryEntry>, DaemonError> {
    db.query_history(filter)
        .map_err(db_err("querying history"))
}

pub fn get_history_stats(
    db: &Database,
    session_start_ms: i64,
) -> Result<HistoryStats, DaemonError> {
    db.get_history_stats(session_start_ms)
        .map_err(db_err("calculating history stats"))
}

pub fn clear_history(db: &mut Database) -> Result<usize, DaemonError> {
    db.clear_history()
        .map_err(db_err("clearing history"))
}

pub fn apply_retention_policy(
    db: &mut Database,
    retention_days: u32,
) -> Result<usize, DaemonError> {
    db.apply_retention_policy(retention_days)
        .map_err(db_err("applying retention policy"))
}

pub fn export_history(
    db: &Database,
    format: &str,
    filter: &HistoryFilter,
) -> Result<String, DaemonError> {
    let entries = query_history(db, filter)?;
    match format.to_ascii_lowercase().as_str() {
        "csv" => {
            let mut out = String::from(
                "id,download_id,file_name,total_size,transferred_bytes,ed2k_hash,event_type,status,duration_secs,connection_attempts,resume_count,verification_result,error_message,timestamp_unix_ms\n",
            );
            for e in entries {
                let escaped_name = e.file_name.replace('"', "\"\"");
                let escaped_err = e
                    .error_message
                    .as_deref()
                    .unwrap_or("")
                    .replace('"', "\"\"");
                out.push_str(&format!(
                    "{},{},\"{}\",{},{},{},{},{},{},{},{},{},\"{}\",{}\n",
                    e.id,
                    e.download_id,
                    escaped_name,
                    e.total_size,
                    e.transferred_bytes,
                    e.ed2k_hash,
                    e.event_type,
                    e.status,
                    e.duration_secs,
                    e.connection_attempts,
                    e.resume_count,
                    e.verification_result,
                    escaped_err,
                    e.timestamp_unix_ms
                ));
            }
            Ok(out)
        }
        "json" => serde_json::to_string_pretty(&entries)
            .map_err(|e| DaemonError::Business(format!("JSON serialization error: {e}"))),
        other => Err(DaemonError::InvalidParams(format!(
            "Unsupported export format: {other}"
        ))),
    }
}

// ── Categories Management ──────────────────────────────────────────────────

pub fn list_categories(db: &Database) -> Result<Vec<DownloadCategory>, DaemonError> {
    db.get_categories()
        .map_err(db_err("listing categories"))
}

pub fn get_category(db: &Database, id: &str) -> Result<Option<DownloadCategory>, DaemonError> {
    db.get_category(id)
        .map_err(db_err("getting category"))
}

pub fn create_category(
    db: &mut Database,
    name: &str,
    color: &str,
    default_destination: Option<String>,
    default_priority: Option<String>,
) -> Result<DownloadCategory, DaemonError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(DaemonError::InvalidParams(
            "Category name cannot be empty".into(),
        ));
    }

    let existing = list_categories(db)?;
    let sort_order = existing.len() as i32;

    let category = DownloadCategory::new(
        name,
        color,
        sort_order,
        default_destination
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        default_priority
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty()),
    );

    db.insert_category(&category)
        .map_err(db_err("creating category"))?;

    Ok(category)
}

pub fn update_category(
    db: &mut Database,
    id: &str,
    name: Option<String>,
    color: Option<String>,
    default_destination: Option<Option<String>>,
    default_priority: Option<Option<String>>,
    sort_order: Option<i32>,
) -> Result<DownloadCategory, DaemonError> {
    let mut cat = get_category(db, id)?
        .ok_or_else(|| DaemonError::Business(format!("Category '{id}' not found")))?;

    if let Some(n) = name {
        let trimmed = n.trim();
        if trimmed.is_empty() {
            return Err(DaemonError::InvalidParams(
                "Category name cannot be empty".into(),
            ));
        }
        cat.name = trimmed.to_string();
    }

    if let Some(c) = color {
        cat.color = c;
    }

    if let Some(dest_opt) = default_destination {
        cat.default_destination = dest_opt
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
    }

    if let Some(prio_opt) = default_priority {
        cat.default_priority = prio_opt
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty());
    }

    if let Some(order) = sort_order {
        cat.sort_order = order;
    }

    cat.updated_at = now_ms();

    db.update_category(&cat)
        .map_err(db_err("updating category"))?;

    Ok(cat)
}

pub fn delete_category(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    id: &str,
) -> Result<usize, DaemonError> {
    let reassigned = db
        .delete_category(id)
        .map_err(db_err("deleting category"))?;

    let now = now_ms();
    for a in active.values_mut() {
        if a.download.category_id.as_deref() == Some(id) {
            a.download.category_id = None;
            a.download.updated_at = now;
        }
    }

    Ok(reassigned)
}

pub fn reorder_categories(db: &mut Database, category_ids: &[String]) -> Result<(), DaemonError> {
    db.reorder_categories(category_ids)
        .map_err(db_err("reordering categories"))
}
