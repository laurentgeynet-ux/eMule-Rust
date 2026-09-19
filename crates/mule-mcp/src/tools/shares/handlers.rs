//! RPC execution handlers for shares, sources, history, uploads, and listeners.

use crate::client::DaemonClient;
use crate::protocol::ToolCallResult;
use serde_json::{json, Value};

/// Dispatches share/source/network tool invocations over JSON-RPC.
pub async fn execute(
    client: &DaemonClient,
    tool_name: &str,
    args: &Value,
) -> Option<ToolCallResult> {
    match tool_name {
        "emule_list_sources" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return Some(ToolCallResult::error("Missing required parameter: 'file_hash'")),
            };

            match client.call("sources.list", json!({ "fileHash": hash })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to list sources: {e}"))),
            }
        }

        "emule_add_manual_source" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return Some(ToolCallResult::error("Missing required parameter: 'file_hash'")),
            };
            let address = match args.get("address").and_then(|a| a.as_str()) {
                Some(a) => a,
                None => return Some(ToolCallResult::error("Missing required parameter: 'address'")),
            };

            let mut params = json!({ "fileHash": hash, "address": address });
            if let Some(uh) = args.get("user_hash").and_then(|u| u.as_str()) {
                params["userHash"] = json!(uh);
            }
            if let Some(co) = args.get("crypt_options").and_then(|c| c.as_u64()) {
                params["cryptOptions"] = json!(co);
            }

            match client.call("sources.addManual", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to add manual source: {e}"))),
            }
        }

        "emule_connect_source" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return Some(ToolCallResult::error("Missing required parameter: 'file_hash'")),
            };
            let address = match args.get("address").and_then(|a| a.as_str()) {
                Some(a) => a,
                None => return Some(ToolCallResult::error("Missing required parameter: 'address'")),
            };

            match client.call("sources.connect", json!({ "fileHash": hash, "address": address })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to connect to source: {e}"))),
            }
        }

        "emule_disconnect_source" => {
            let mut params = json!({});
            if let Some(a) = args.get("address").and_then(|s| s.as_str()) {
                params["address"] = json!(a);
            }

            match client.call("sources.disconnect", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to disconnect source: {e}"))),
            }
        }

        "emule_clear_sources" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return Some(ToolCallResult::error("Missing required parameter: 'file_hash'")),
            };

            match client.call("sources.clear", json!({ "fileHash": hash })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to clear sources: {e}"))),
            }
        }

        "emule_swarm_status" => {
            let mut params = json!({});
            if let Some(h) = args.get("file_hash").and_then(|s| s.as_str()) {
                params["fileHash"] = json!(h);
            }

            match client.call("sources.status", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to get swarm status: {e}"))),
            }
        }

        "emule_list_shares" => {
            match client.call("shares.list", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to list shared files: {e}"))),
            }
        }

        "emule_add_share" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return Some(ToolCallResult::error("Missing required parameter: 'file_hash'")),
            };
            let name = match args.get("file_name").and_then(|n| n.as_str()) {
                Some(n) => n,
                None => return Some(ToolCallResult::error("Missing required parameter: 'file_name'")),
            };
            let size = match args.get("file_size").and_then(|s| s.as_u64()) {
                Some(s) => s,
                None => return Some(ToolCallResult::error("Missing required parameter: 'file_size'")),
            };
            let path = args.get("file_path").and_then(|p| p.as_str()).unwrap_or("");

            match client.call("shares.add", json!({
                "fileHash": hash,
                "fileName": name,
                "fileSize": size,
                "filePath": path
            })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to share file: {e}"))),
            }
        }

        "emule_remove_share" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return Some(ToolCallResult::error("Missing required parameter: 'file_hash'")),
            };

            match client.call("shares.remove", json!({ "fileHash": hash })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to unshare file: {e}"))),
            }
        }

        "emule_set_share_enabled" => {
            let hash = match args.get("file_hash").and_then(|h| h.as_str()) {
                Some(h) => h,
                None => return Some(ToolCallResult::error("Missing required parameter: 'file_hash'")),
            };
            let enabled = match args.get("enabled").and_then(|e| e.as_bool()) {
                Some(e) => e,
                None => return Some(ToolCallResult::error("Missing required parameter: 'enabled'")),
            };

            match client.call("shares.setEnabled", json!({ "fileHash": hash, "enabled": enabled })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to set share state: {e}"))),
            }
        }

        "emule_list_uploads" => {
            match client.call("uploads.list", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to list uploads: {e}"))),
            }
        }

        "emule_cancel_upload" => {
            let client_address = match args.get("client_address").and_then(|c| c.as_str()) {
                Some(c) => c,
                None => return Some(ToolCallResult::error("Missing required parameter: 'client_address'")),
            };

            match client.call("uploads.cancel", json!({ "clientAddress": client_address })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to cancel upload: {e}"))),
            }
        }

        "emule_start_listener" => {
            let mut params = json!({});
            if let Some(p) = args.get("port").and_then(|s| s.as_u64()) {
                params["port"] = json!(p);
            }
            if let Some(b) = args.get("bind_ip").and_then(|s| s.as_str()) {
                params["bindIp"] = json!(b);
            }

            match client.call("listener.start", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to start inbound listener: {e}"))),
            }
        }

        "emule_stop_listener" => {
            match client.call("listener.stop", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to stop inbound listener: {e}"))),
            }
        }

        "emule_listener_status" => {
            match client.call("listener.status", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to get listener status: {e}"))),
            }
        }

        "emule_upnp_status" => {
            match client.call("upnp.status", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to get UPnP status: {e}"))),
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
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to map UPnP ports: {e}"))),
            }
        }

        "emule_upnp_unmap_ports" => {
            match client.call("upnp.unmapPorts", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to unmap UPnP ports: {e}"))),
            }
        }

        "emule_upnp_set_enabled" => {
            let enabled = match args.get("enabled").and_then(|e| e.as_bool()) {
                Some(e) => e,
                None => return Some(ToolCallResult::error("Missing required parameter: 'enabled'")),
            };

            match client.call("upnp.setEnabled", json!({ "enabled": enabled })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to set UPnP enabled state: {e}"))),
            }
        }

        "emule_get_identity" => {
            match client.call("identity.status", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to get identity status: {e}"))),
            }
        }

        "emule_rotate_identity" => {
            match client.call("identity.rotate", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to rotate identity key: {e}"))),
            }
        }

        "emule_list_credits" => {
            let mut params = json!({});
            if let Some(l) = args.get("limit").and_then(|s| s.as_u64()) {
                params["limit"] = json!(l);
            }

            match client.call("credits.list", params).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to list peer credits: {e}"))),
            }
        }

        "emule_credits_stats" => {
            match client.call("credits.stats", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to get credits stats: {e}"))),
            }
        }

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
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to list history: {e}"))),
            }
        }

        "emule_history_stats" => {
            match client.call("history.stats", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to get history stats: {e}"))),
            }
        }

        "emule_clear_history" => {
            match client.call("history.clear", json!({})).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to clear history: {e}"))),
            }
        }

        "emule_export_history" => {
            let format = args.get("format").and_then(|f| f.as_str()).unwrap_or("json");
            match client.call("history.export", json!({ "format": format })).await {
                Ok(res) => Some(ToolCallResult::json(&res)),
                Err(e) => Some(ToolCallResult::error(format!("Failed to export history: {e}"))),
            }
        }

        _ => None,
    }
}
