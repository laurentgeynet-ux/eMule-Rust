//! RPC execution handlers for downloads and categories.

use crate::client::DaemonClient;
use crate::protocol::ToolCallResult;
use serde_json::{json, Value};

/// Resolves a download UUID or file hash to an active download instance ID.
pub async fn resolve_download_id(client: &DaemonClient, id_or_hash: &str) -> Result<String, String> {
    if id_or_hash.contains('-') && uuid::Uuid::parse_str(id_or_hash).is_ok() {
        return Ok(id_or_hash.to_string());
    }

    let list_res = client.call("downloads.list", json!({})).await?;
    if let Some(downloads) = list_res.get("downloads").and_then(|d| d.as_array()) {
        for dl in downloads {
            if let Some(id) = dl.get("id").and_then(|i| i.as_str()) {
                if id.eq_ignore_ascii_case(id_or_hash) {
                    return Ok(id.to_string());
                }
                if let Some(name) = dl.get("fileName").and_then(|n| n.as_str()) {
                    if name.eq_ignore_ascii_case(id_or_hash) {
                        return Ok(id.to_string());
                    }
                }
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

    if uuid::Uuid::parse_str(id_or_hash).is_ok() {
        return Ok(id_or_hash.to_string());
    }

    Err(format!("No download found matching ID, filename, or file hash '{id_or_hash}'"))
}

/// Dispatches download and category tool invocations over JSON-RPC.
pub async fn execute(
    client: &DaemonClient,
    tool_name: &str,
    args: &Value,
) -> Option<ToolCallResult> {
    match tool_name {
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
                    Some(ToolCallResult::json(&res))
                }
                Err(e) => Some(ToolCallResult::error(format!("Failed to list downloads: {e}"))),
            }
        }

        "emule_get_download_details" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return Some(ToolCallResult::error("Missing required parameter: 'id_or_hash'")),
            };

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return Some(ToolCallResult::error(e)),
            };

            match client.call("downloads.get", json!({ "id": id })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to get download details: {e}"))),
            }
        }

        "emule_add_link" => {
            let link = match args.get("link").and_then(|l| l.as_str()) {
                Some(l) => l,
                None => return Some(ToolCallResult::error("Missing required parameter: 'link'")),
            };

            let mut params = json!({ "link": link });
            if let Some(cat) = args.get("category_id").and_then(|c| c.as_str()) {
                params["categoryId"] = json!(cat);
            }
            if let Some(dest) = args.get("destination_dir").and_then(|d| d.as_str()) {
                params["targetDirectory"] = json!(dest);
            }

            match client.call("downloads.addEd2kLink", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to add ed2k link: {e}"))),
            }
        }

        "emule_pause_download" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return Some(ToolCallResult::error("Missing required parameter: 'id_or_hash'")),
            };

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return Some(ToolCallResult::error(e)),
            };

            match client.call("downloads.pause", json!({ "id": id })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to pause download: {e}"))),
            }
        }

        "emule_resume_download" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return Some(ToolCallResult::error("Missing required parameter: 'id_or_hash'")),
            };

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return Some(ToolCallResult::error(e)),
            };

            match client.call("downloads.resume", json!({ "id": id })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to resume download: {e}"))),
            }
        }

        "emule_remove_download" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return Some(ToolCallResult::error("Missing required parameter: 'id_or_hash'")),
            };
            let delete_files = args.get("delete_files").and_then(|d| d.as_bool()).unwrap_or(false);

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return Some(ToolCallResult::error(e)),
            };

            match client.call("downloads.remove", json!({ "id": id, "deleteFiles": delete_files })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to remove download: {e}"))),
            }
        }

        "emule_set_destination" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return Some(ToolCallResult::error("Missing required parameter: 'id_or_hash'")),
            };
            let destination_dir = match args.get("destination_dir").and_then(|d| d.as_str()) {
                Some(d) => d,
                None => return Some(ToolCallResult::error("Missing required parameter: 'destination_dir'")),
            };

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return Some(ToolCallResult::error(e)),
            };

            match client.call("downloads.setDestination", json!({ "id": id, "targetDirectory": destination_dir })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to set destination directory: {e}"))),
            }
        }

        "emule_download_set_category" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return Some(ToolCallResult::error("Missing required parameter: 'id_or_hash'")),
            };
            let category_id = args.get("category_id").and_then(|c| c.as_str());

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return Some(ToolCallResult::error(e)),
            };

            let mut params = json!({ "id": id });
            if let Some(cat) = category_id {
                if !cat.trim().is_empty() {
                    params["categoryId"] = json!(cat);
                }
            }

            match client.call("downloads.setCategory", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to set download category: {e}"))),
            }
        }

        "emule_retry_download" => {
            let id_or_hash = match args.get("id_or_hash").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return Some(ToolCallResult::error("Missing required parameter: 'id_or_hash'")),
            };

            let id = match resolve_download_id(client, id_or_hash).await {
                Ok(id) => id,
                Err(e) => return Some(ToolCallResult::error(e)),
            };

            match client.call("downloads.retryConnection", json!({ "id": id })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to retry download connection: {e}"))),
            }
        }

        "emule_inspect_part_met" => {
            let met_path = match args.get("met_path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return Some(ToolCallResult::error("Missing required parameter: 'met_path'")),
            };

            let mut params = json!({ "metPath": met_path });
            if let Some(part) = args.get("part_path").and_then(|p| p.as_str()) {
                params["partPath"] = json!(part);
            }

            match client.call("downloads.inspectPartMet", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to inspect .part.met: {e}"))),
            }
        }

        "emule_import_part_met" => {
            let met_path = match args.get("met_path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return Some(ToolCallResult::error("Missing required parameter: 'met_path'")),
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
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to import .part.met: {e}"))),
            }
        }

        "emule_list_categories" => {
            match client.call("categories.list", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to list categories: {e}"))),
            }
        }

        "emule_create_category" => {
            let name = match args.get("name").and_then(|n| n.as_str()) {
                Some(n) => n,
                None => return Some(ToolCallResult::error("Missing required parameter: 'name'")),
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
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to create category: {e}"))),
            }
        }

        "emule_update_category" => {
            let id = match args.get("id").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return Some(ToolCallResult::error("Missing required parameter: 'id'")),
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
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to update category: {e}"))),
            }
        }

        "emule_delete_category" => {
            let id = match args.get("id").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => return Some(ToolCallResult::error("Missing required parameter: 'id'")),
            };

            match client.call("categories.delete", json!({ "id": id })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to delete category: {e}"))),
            }
        }

        "emule_reorder_categories" => {
            let ids = match args.get("category_ids").and_then(|a| a.as_array()) {
                Some(a) => a,
                None => return Some(ToolCallResult::error("Missing required parameter: 'category_ids'")),
            };

            match client.call("categories.reorder", json!({ "categoryIds": ids })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to reorder categories: {e}"))),
            }
        }

        _ => None,
    }
}
