// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::JsonRpcError;
use crate::rpc::methods::DAEMON_VERSION;
use crate::state::AppState;

/// Clés camelCase reconnues par `settings.set` — toute autre clé est
/// ignorée (un warning est émis pour faciliter le diagnostic des typos).
const KNOWN_SETTINGS_KEYS: &[&str] = &[
    "defaultDownloadDir",
    "historyRetentionDays",
    "maxGlobalDownloadPeers",
    "maxPeersPerDownload",
    "minPeersPerDownload",
    "shareDownloadDir",
    "nickname",
    "obfuscationEnabled",
    "obfuscationRequired",
    "maxConnections",
    "maxConnPerMin",
    "maxUploadQueue",
    "ussEnabled",
    "ed2kPort",
    "kadPort",
    "udpEnabled",
    "upnpEnabled",
    "capacityDownloadKbs",
    "capacityUploadKbs",
    "minUploadKbs",
    "ichEnabled",
    "rememberCancelled",
    "rememberDownloaded",
    "serverReconnect",
    "serverAutoconnect",
    "serverAutoremoveDead",
    "serverAutoupdateList",
    "serverAutoupdatePath",
    "safeConnect",
    "autoPriorityNewDownloads",
    "autoPriorityUploads",
    "swarmHealing",
    "tempDir",
    "identityRotationHours",
    "dailyUploadQuotaGb",
    "paranoidMode",
];

pub async fn handle_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let dl_count = state.downloads.lock().await.list_downloads().len();
    let capabilities = vec![
        "downloads",
        "download-events",
        "history",
        "categories",
        "import_part_met",
        "network-policy",
        "ip-filter",
        "rate-limits",
        "servers",
        "server-connection",
        "server-search",
        "server-sources",
        "peer-sources",
        "peer-connection",
        "peer-robustness",
        "swarm-dual-source",
        "shared-files",
        "upload-single-slot",
        "secure-ident",
        "peer-credits",
        "kad-contacts",
        #[cfg(feature = "lab-tools")]
        "lab-tools",
    ];

    let default_dl_dir = {
        state
            .downloads
            .lock()
            .await
            .get_setting("default_download_directory")
            .ok()
            .flatten()
            .unwrap_or_else(|| {
                state
                    .config
                    .default_download_dir
                    .to_string_lossy()
                    .to_string()
            })
    };

    Ok(json!({
        "version": DAEMON_VERSION,
        "status": "healthy",
        "downloadsCount": dl_count,
        "instanceId": state.instance_id.to_string(),
        "capabilities": capabilities,
        "workdirMode": format!("{:?}", state.config.workdir_mode),
        "workdirPath": state.config.data_dir.to_string_lossy(),
        "defaultDownloadDir": default_dl_dir,
        "networkState": state.network_policy.state().as_str(),
        "networkProfile": state.network_policy.profile().to_string(),
        "logLevel": state.current_log_level(),
    }))
}

pub async fn handle_stop(state: &AppState) -> Result<Value, JsonRpcError> {
    let notify = state.shutdown_notify.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        notify.notify_waiters();
    });
    Ok(json!({ "stopping": true }))
}

/// `stats.resetLifetimeTotals` — remet à zéro les compteurs cumulés
/// « depuis l'installation » (download + upload) : les compteurs en
/// mémoire alimentant `statsTick` ET les valeurs persistées. Les totaux
/// de session ne sont pas touchés ; les octets émis après le reset
/// recommencent à s'accumuler à partir de zéro.
pub async fn handle_stats_reset_lifetime_totals(state: &AppState) -> Result<Value, JsonRpcError> {
    state
        .lifetime_downloaded_bytes
        .store(0, std::sync::atomic::Ordering::SeqCst);
    state
        .lifetime_uploaded_bytes
        .store(0, std::sync::atomic::Ordering::SeqCst);
    let mut dl = state.downloads.lock().await;
    dl.set_setting("lifetime_downloaded_bytes", "0")?;
    dl.set_setting("lifetime_uploaded_bytes", "0")?;
    Ok(json!({ "reset": true }))
}

pub async fn handle_settings_get(state: &AppState) -> Result<Value, JsonRpcError> {
    let (
        custom_dir,
        retention_days,
        max_global,
        max_per_file,
        min_per_file,
        share_dir,
        nickname,
        obf_enabled,
        obf_required,
        max_conn,
        conn_per_min,
        max_queue,
    ) = {
        let dls = state.downloads.lock().await;
        let dir = dls.get_setting("default_download_directory")?;
        let ret = dls
            .get_setting("history_retention_days")?
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(30);
        let max_g = dls
            .get_setting("max_global_download_peers")?
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(state.config.max_global_download_peers);
        let max_f = dls
            .get_setting("max_peers_per_download")?
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(state.config.max_peers_per_download);
        let min_f = dls
            .get_setting("min_peers_per_download")?
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(state.config.min_peers_per_download);
        let share = dls
            .get_setting("share_download_dir")?
            .map(|v| !matches!(v.as_str(), "0" | "false" | "off"))
            .unwrap_or(true);
        let nickname = dls
            .get_setting("nickname")?
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "eMule-Rust".to_string());
        let bool_setting = |key: &str, default: bool| {
            dls.get_setting(key)
                .ok()
                .flatten()
                .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "on"))
                .unwrap_or(default)
        };
        let obf_enabled = bool_setting("obfuscation_enabled", state.config.obfuscation_enabled);
        let obf_required = bool_setting("obfuscation_required", state.config.obfuscation_required);
        let max_conn = dls
            .get_setting("max_connections")?
            .and_then(|s| s.parse::<usize>().ok())
            .map(|v| v.clamp(1, mule_network_policy::MAX_PEER_CONNECTIONS_CAP))
            .unwrap_or(state.network_policy.budgets().max_active_peer_connections);
        let conn_per_min = dls
            .get_setting("max_conn_per_min")?
            .and_then(|s| s.parse::<u32>().ok())
            .map(|v| v.clamp(1, 1200))
            .unwrap_or(state.network_policy.budgets().rate_limit_per_min);
        let max_queue = dls
            .get_setting("max_upload_queue")?
            .and_then(|s| s.parse::<usize>().ok())
            .map(|v| v.clamp(10, 5000))
            .unwrap_or(crate::services::upload_queue::MAX_UPLOAD_QUEUE);
        (
            dir,
            ret,
            max_g,
            max_f,
            min_f,
            share,
            nickname,
            obf_enabled,
            obf_required,
            max_conn,
            conn_per_min,
            max_queue,
        )
    };
    let uss_enabled = state.uss.config().enabled;
    let upnp_enabled = state.upnp.snapshot().await.enabled;
    let (cap_dl, cap_ul) = {
        let dls = state.downloads.lock().await;
        (
            dls.get_setting("capacity_download_kbs")?
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0),
            dls.get_setting("capacity_upload_kbs")?
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0),
        )
    };
    let bools = {
        let dls = state.downloads.lock().await;
        let read = |key: &str, default: bool| {
            dls.get_setting(key)
                .ok()
                .flatten()
                .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "on"))
                .unwrap_or(default)
        };
        let u64_setting = |key: &str| {
            dls.get_setting(key)
                .ok()
                .flatten()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0)
        };
        let min_up = dls
            .get_setting("min_upload_kbs")?
            .and_then(|s| s.parse::<u64>().ok())
            // eMule 0.72a `MinUpload` default = 1 Ko/s (clamped >= 1).
            .unwrap_or(1);
        json!({
            "minUploadKbs": min_up,
            "ichEnabled": read("ich_enabled", true),
            "rememberCancelled": read("remember_cancelled", true),
            "rememberDownloaded": read("remember_downloaded", true),
            "serverReconnect": read("server_reconnect", true),
            "serverAutoconnect": read("server_autoconnect", false),
            "serverAutoremoveDead": read("server_autoremove_dead", true),
            "serverAutoupdateList": read("server_autoupdate_list", false),
            "safeConnect": read("safe_connect", false),
            // eMule `DAPPref`/`UAPPref` — défauts `true` dans les deux cas.
            "autoPriorityNewDownloads": read("auto_priority_new_downloads", true),
            "autoPriorityUploads": read("auto_priority_uploads", true),
            // Options propres à eMule-Rust (pas d'équivalent eMule).
            "identityRotationHours": u64_setting("identity_rotation_hours"),
            "dailyUploadQuotaGb": u64_setting("daily_upload_quota_gb"),
            "uploadQuotaUsedBytes": u64_setting("upload_quota_used_bytes"),
            "paranoidMode": read("paranoid_mode", false),
            "swarmHealing": read("swarm_healing", false),
            "serverAutoupdatePath": dls
                .get_setting("server_autoupdate_path")
                .ok()
                .flatten()
                .unwrap_or_default(),
            "tempDir": dls
                .get_setting("temp_dir")
                .ok()
                .flatten()
                .unwrap_or_default(),
        })
    };

    let current_dir = custom_dir
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| state.config.default_download_dir.clone());

    let portable_default =
        crate::workdir::WorkDirectoryResolver::resolve(None, None).default_download_dir;
    let mut result = json!({
        "defaultDownloadDir": current_dir.to_string_lossy(),
        "portableDownloadDir": portable_default.to_string_lossy(),
        "isCustom": custom_dir.is_some(),
        "historyRetentionDays": retention_days,
        "maxGlobalDownloadPeers": max_global,
        "maxPeersPerDownload": max_per_file,
        "minPeersPerDownload": min_per_file,
        "shareDownloadDir": share_dir,
        "nickname": nickname,
        "obfuscationEnabled": obf_enabled,
        "obfuscationRequired": obf_required,
        "maxConnections": max_conn,
        "maxConnPerMin": conn_per_min,
        "maxUploadQueue": max_queue,
        "ussEnabled": uss_enabled,
        "ed2kPort": state.ed2k_port(),
        "kadPort": state.kad_port(),
        "udpEnabled": state.udp_enabled(),
        "upnpEnabled": upnp_enabled,
        "capacityDownloadKbs": cap_dl,
        "capacityUploadKbs": cap_ul,
        // Runtime uniquement — reflète la spec EnvFilter effective, pas
        // un setting persisté.
        "logLevel": state.current_log_level(),
    });
    if let (Some(out), Some(extra)) = (result.as_object_mut(), bools.as_object()) {
        out.extend(extra.clone());
    }
    Ok(result)
}

pub async fn handle_settings_set(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let mut updated_dir = None;
    let mut updated_retention = None;
    let mut updated_max_global = None;
    let mut updated_max_per_file = None;
    let mut updated_min_per_file = None;
    let mut updated_share_dir = None;
    let mut keys: Vec<&str> = Vec::new();

    if let Some(dir_str) = params["defaultDownloadDir"].as_str() {
        let dir = std::path::PathBuf::from(dir_str);
        // Validate directory is creatable & writable
        std::fs::create_dir_all(&dir)
            .map_err(|e| JsonRpcError::business(format!("Cannot create directory: {e}")))?;

        let probe = dir.join(format!(".mule_write_probe_{}", Uuid::new_v4()));
        match std::fs::write(&probe, b"probe") {
            Ok(_) => {
                let _ = std::fs::remove_file(&probe);
            }
            Err(e) => {
                return Err(JsonRpcError::business(format!(
                    "Directory is not writable: {e}"
                )));
            }
        }

        state
            .downloads
            .lock()
            .await
            .set_setting("default_download_directory", dir_str)?;
        updated_dir = Some(dir_str.to_string());
    }

    if let Some(ret_days_val) = params.get("historyRetentionDays") {
        let ret_days = ret_days_val.as_u64().ok_or_else(|| {
            JsonRpcError::invalid_params("Invalid 'historyRetentionDays' parameter")
        })? as u32;
        let mut dls = state.downloads.lock().await;
        dls.set_setting("history_retention_days", &ret_days.to_string())?;
        dls.apply_retention_policy(ret_days)?;
        updated_retention = Some(ret_days);
    }

    if let Some(val) = params.get("maxGlobalDownloadPeers") {
        let max_g = val.as_u64().ok_or_else(|| {
            JsonRpcError::invalid_params("Invalid 'maxGlobalDownloadPeers' parameter")
        })? as usize;
        let mut dls = state.downloads.lock().await;
        dls.set_setting("max_global_download_peers", &max_g.to_string())?;
        updated_max_global = Some(max_g);
    }

    if let Some(val) = params.get("maxPeersPerDownload") {
        let max_f = val.as_u64().ok_or_else(|| {
            JsonRpcError::invalid_params("Invalid 'maxPeersPerDownload' parameter")
        })? as usize;
        let mut dls = state.downloads.lock().await;
        dls.set_setting("max_peers_per_download", &max_f.to_string())?;
        updated_max_per_file = Some(max_f);
    }

    if let Some(val) = params.get("minPeersPerDownload") {
        let min_f = val.as_u64().ok_or_else(|| {
            JsonRpcError::invalid_params("Invalid 'minPeersPerDownload' parameter")
        })? as usize;
        let mut dls = state.downloads.lock().await;
        dls.set_setting("min_peers_per_download", &min_f.to_string())?;
        updated_min_per_file = Some(min_f);
    }

    if let Some(val) = params.get("shareDownloadDir") {
        let share = val
            .as_bool()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'shareDownloadDir' parameter"))?;
        state
            .downloads
            .lock()
            .await
            .set_setting("share_download_dir", if share { "1" } else { "0" })?;
        updated_share_dir = Some(share);
    }

    if let Some(val) = params.get("nickname") {
        let nick = val
            .as_str()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'nickname' parameter"))?
            .trim();
        if nick.is_empty() || nick.len() > 64 {
            return Err(JsonRpcError::invalid_params(
                "'nickname' must be 1..64 characters",
            ));
        }
        state.downloads.lock().await.set_setting("nickname", nick)?;
        keys.push("nickname");
    }

    let mut obf_changed = false;
    if let Some(val) = params.get("obfuscationEnabled") {
        let v = val
            .as_bool()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'obfuscationEnabled'"))?;
        state
            .downloads
            .lock()
            .await
            .set_setting("obfuscation_enabled", if v { "1" } else { "0" })?;
        obf_changed = true;
        keys.push("obfuscationEnabled");
    }
    if let Some(val) = params.get("obfuscationRequired") {
        let v = val
            .as_bool()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'obfuscationRequired'"))?;
        state
            .downloads
            .lock()
            .await
            .set_setting("obfuscation_required", if v { "1" } else { "0" })?;
        obf_changed = true;
        keys.push("obfuscationRequired");
    }
    if obf_changed {
        let (enabled, required) = state.effective_obfuscation().await;
        state.inbound_listener.set_obfuscation(enabled, required);
        // Propager la préférence à la session serveur (eMule
        // IsServerCryptLayerConnection) — sinon le réglage n'agit que sur
        // les pairs entrants.
        state
            .server_session
            .set_obfuscation_preference(enabled || required, state.config.obfuscation_padding_max);
        // Update the buddy-relay connect-options byte announced over Kad.
        let connect_opts: u8 = if enabled { 0x03 } else { 0 } | if required { 0x04 } else { 0 };
        state
            .kad
            .udp()
            .set_buddy_tcp_info(state.ed2k_port(), connect_opts);
    }

    let mut budgets = state.network_policy.budgets();
    let mut budgets_changed = false;
    if let Some(val) = params.get("maxConnections") {
        // Même borne qu'au démarrage (`state.rs`) : le budget réseau est
        // plafonné à [1, 65000] — eMule ne borne pas MaxConnections sous
        // NT+, on garde un garde-fou anti-faute de frappe. On clampe avant
        // de persister pour que la valeur stockée reste identique à celle
        // réellement appliquée — sinon `set_budgets` rejetait après
        // l'écriture en base.
        let v = val
            .as_u64()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'maxConnections'"))?
            .clamp(1, mule_network_policy::MAX_PEER_CONNECTIONS_CAP as u64)
            as usize;
        state
            .downloads
            .lock()
            .await
            .set_setting("max_connections", &v.to_string())?;
        budgets.max_active_peer_connections = v;
        budgets_changed = true;
        keys.push("maxConnections");
    }
    if let Some(val) = params.get("maxConnPerMin") {
        let v = val
            .as_u64()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'maxConnPerMin'"))?
            .clamp(1, 1200) as u32;
        state
            .downloads
            .lock()
            .await
            .set_setting("max_conn_per_min", &v.to_string())?;
        budgets.rate_limit_per_min = v;
        budgets_changed = true;
        keys.push("maxConnPerMin");
    }
    if budgets_changed {
        state
            .network_policy
            .set_budgets(budgets)
            .map_err(|e| JsonRpcError::business(format!("{e}")))?;
    }

    if let Some(val) = params.get("maxUploadQueue") {
        // Clamp identique à `UploadManager::set_max_queue` ([10, 5000],
        // eMule QueueSize) avant persistance : stockée = appliquée.
        let v = val
            .as_u64()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'maxUploadQueue'"))?
            .clamp(10, 5000) as usize;
        state
            .downloads
            .lock()
            .await
            .set_setting("max_upload_queue", &v.to_string())?;
        state.inbound_listener.upload_manager().set_max_queue(v);
        keys.push("maxUploadQueue");
    }

    if let Some(val) = params.get("ussEnabled") {
        let v = val
            .as_bool()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'ussEnabled'"))?;
        state.uss.update_config(|c| c.enabled = v).await;
        keys.push("ussEnabled");
    }

    // --- Ports & UDP (eMule Preferences → Connexion) ---
    let mut ports_changed = false;
    if let Some(val) = params.get("ed2kPort") {
        let v = val
            .as_u64()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'ed2kPort'"))?;
        if v == 0 || v > 65535 {
            return Err(JsonRpcError::invalid_params("'ed2kPort' must be 1..65535"));
        }
        state
            .downloads
            .lock()
            .await
            .set_setting("ed2k_port", &v.to_string())?;
        ports_changed = true;
        keys.push("ed2kPort");
    }
    if let Some(val) = params.get("kadPort") {
        let v = val
            .as_u64()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'kadPort'"))?;
        if v == 0 || v > 65535 {
            return Err(JsonRpcError::invalid_params("'kadPort' must be 1..65535"));
        }
        state
            .downloads
            .lock()
            .await
            .set_setting("kad_port", &v.to_string())?;
        ports_changed = true;
        keys.push("kadPort");
    }
    if let Some(val) = params.get("udpEnabled") {
        let v = val
            .as_bool()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'udpEnabled'"))?;
        state
            .downloads
            .lock()
            .await
            .set_setting("udp_enabled", if v { "1" } else { "0" })?;
        state.set_udp_enabled(v);
        apply_udp_state(state, v).await;
        keys.push("udpEnabled");
    }
    if let Some(val) = params.get("upnpEnabled") {
        let v = val
            .as_bool()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'upnpEnabled'"))?;
        state
            .downloads
            .lock()
            .await
            .set_setting("upnp_enabled", if v { "1" } else { "0" })?;
        state.upnp.set_enabled(v).await;
        let upnp = state.upnp.clone();
        let (tcp, udp) = (state.ed2k_port(), state.kad_port());
        tokio::spawn(async move {
            if v {
                let _ = upnp.map_ports(tcp, udp).await;
            } else {
                let _ = upnp.unmap_ports().await;
            }
        });
        keys.push("upnpEnabled");
    }
    for key in ["capacityDownloadKbs", "capacityUploadKbs"] {
        if let Some(val) = params.get(key) {
            let v = val
                .as_u64()
                .ok_or_else(|| JsonRpcError::invalid_params("Invalid capacity value"))?;
            let snake = if key == "capacityDownloadKbs" {
                "capacity_download_kbs"
            } else {
                "capacity_upload_kbs"
            };
            state
                .downloads
                .lock()
                .await
                .set_setting(snake, &v.to_string())?;
            keys.push(key);
        }
    }
    if let Some(v) = params.get("capacityUploadKbs").and_then(|v| v.as_u64()) {
        // Upload capacity = reference line speed used by USS as its
        // baseline manual limit (eMule uses it for slot computations).
        state.uss.note_manual_limit(v.saturating_mul(1024));
    }
    if params.get("capacityDownloadKbs").is_some() || params.get("capacityUploadKbs").is_some() {
        // eMule borne les limites effectives à la capacité déclarée :
        // une baisse de capacité réduit la limite en conséquence.
        crate::rpc::handlers::network::apply_capacity_clamp(state).await?;
    }
    if let Some(val) = params.get("minUploadKbs") {
        let v = val
            .as_u64()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'minUploadKbs'"))?;
        if v > 100_000 {
            return Err(JsonRpcError::invalid_params(
                "'minUploadKbs' must be 0..100000",
            ));
        }
        state
            .downloads
            .lock()
            .await
            .set_setting("min_upload_kbs", &v.to_string())?;
        state
            .inbound_listener
            .upload_manager()
            .set_min_upload_rate(v.saturating_mul(1024));
        keys.push("minUploadKbs");
    }
    for (camel, snake) in [
        ("ichEnabled", "ich_enabled"),
        ("rememberCancelled", "remember_cancelled"),
        ("rememberDownloaded", "remember_downloaded"),
        ("serverReconnect", "server_reconnect"),
        ("serverAutoconnect", "server_autoconnect"),
        ("serverAutoremoveDead", "server_autoremove_dead"),
        ("serverAutoupdateList", "server_autoupdate_list"),
        ("safeConnect", "safe_connect"),
        ("autoPriorityNewDownloads", "auto_priority_new_downloads"),
        ("autoPriorityUploads", "auto_priority_uploads"),
        ("swarmHealing", "swarm_healing"),
    ] {
        if let Some(val) = params.get(camel) {
            let v = val
                .as_bool()
                .ok_or_else(|| JsonRpcError::invalid_params(&format!("Invalid '{camel}'")))?;
            state
                .downloads
                .lock()
                .await
                .set_setting(snake, if v { "1" } else { "0" })?;
            keys.push(camel);
        }
    }
    if let Some(val) = params.get("serverAutoupdatePath") {
        let p = val
            .as_str()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'serverAutoupdatePath'"))?;
        if p.len() > 512 {
            return Err(JsonRpcError::invalid_params(
                "'serverAutoupdatePath' is too long",
            ));
        }
        state
            .downloads
            .lock()
            .await
            .set_setting("server_autoupdate_path", p)?;
        keys.push("serverAutoupdatePath");
    }
    if let Some(val) = params.get("tempDir") {
        let dir_str = val
            .as_str()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'tempDir'"))?
            .trim();
        if !dir_str.is_empty() {
            let dir = std::path::Path::new(dir_str);
            std::fs::create_dir_all(dir)
                .map_err(|e| JsonRpcError::business(format!("Cannot create temp dir: {e}")))?;
            let probe = dir.join(format!(".mule_write_probe_{}", Uuid::new_v4()));
            match std::fs::write(&probe, b"probe") {
                Ok(_) => {
                    let _ = std::fs::remove_file(&probe);
                }
                Err(e) => {
                    return Err(JsonRpcError::business(format!(
                        "Temp dir is not writable: {e}"
                    )));
                }
            }
        }
        state
            .downloads
            .lock()
            .await
            .set_setting("temp_dir", dir_str)?;
        keys.push("tempDir");
    }
    // --- Options propres à eMule-Rust ---
    if let Some(val) = params.get("identityRotationHours") {
        let v = val
            .as_u64()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'identityRotationHours'"))?;
        if v > 8760 {
            return Err(JsonRpcError::invalid_params(
                "'identityRotationHours' must be 0..8760",
            ));
        }
        state
            .downloads
            .lock()
            .await
            .set_setting("identity_rotation_hours", &v.to_string())?;
        keys.push("identityRotationHours");
    }
    if let Some(val) = params.get("dailyUploadQuotaGb") {
        let v = val
            .as_u64()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'dailyUploadQuotaGb'"))?;
        if v > 1_048_576 {
            return Err(JsonRpcError::invalid_params(
                "'dailyUploadQuotaGb' must be 0..1048576",
            ));
        }
        state
            .downloads
            .lock()
            .await
            .set_setting("daily_upload_quota_gb", &v.to_string())?;
        keys.push("dailyUploadQuotaGb");
    }
    if let Some(val) = params.get("paranoidMode") {
        let v = val
            .as_bool()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'paranoidMode'"))?;
        state
            .downloads
            .lock()
            .await
            .set_setting("paranoid_mode", if v { "1" } else { "0" })?;
        state.inbound_listener.set_paranoid_mode(v);
        keys.push("paranoidMode");
    }
    // Niveau de log à chaud — runtime uniquement, jamais persisté (un
    // `trace` survivant à un reboot remplirait les fichiers). Accepte un
    // niveau simple (« debug ») ou une spec EnvFilter complète.
    if let Some(val) = params.get("logLevel") {
        let spec = val
            .as_str()
            .ok_or_else(|| JsonRpcError::invalid_params("Invalid 'logLevel'"))?;
        let control = state
            .log_control
            .as_ref()
            .ok_or_else(|| JsonRpcError::internal("Log control unavailable"))?;
        control
            .set(spec)
            .map_err(|e| JsonRpcError::invalid_params(&e))?;
        tracing::info!("[LOG] Niveau de log changé à chaud: {spec}");
        keys.push("logLevel");
    }
    if ports_changed {
        apply_port_change(state).await?;
    }

    if updated_max_global.is_some() || updated_max_per_file.is_some() {
        let (current_max_g, current_max_f) = {
            let dls = state.downloads.lock().await;
            let g = dls
                .get_setting("max_global_download_peers")
                .ok()
                .flatten()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(state.config.max_global_download_peers);
            let f = dls
                .get_setting("max_peers_per_download")
                .ok()
                .flatten()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(state.config.max_peers_per_download);
            (g, f)
        };
        state
            .peer_session
            .set_swarm_limits(current_max_f, current_max_g)
            .await;

        let mut b = state.network_policy.budgets();
        if b.max_active_peer_connections < current_max_g {
            b.max_active_peer_connections =
                current_max_g.min(mule_network_policy::MAX_PEER_CONNECTIONS_CAP);
            let _ = state.network_policy.set_budgets(b);
        }
    }

    if updated_dir.is_some() {
        keys.push("defaultDownloadDir");
    }
    if updated_retention.is_some() {
        keys.push("historyRetentionDays");
    }
    if updated_max_global.is_some() {
        keys.push("maxGlobalDownloadPeers");
    }
    if updated_max_per_file.is_some() {
        keys.push("maxPeersPerDownload");
    }
    if updated_min_per_file.is_some() {
        keys.push("minPeersPerDownload");
    }
    if updated_share_dir.is_some() {
        keys.push("shareDownloadDir");
    }
    if let Some(obj) = params.as_object() {
        for k in obj.keys() {
            if !KNOWN_SETTINGS_KEYS.contains(&k.as_str()) {
                tracing::warn!("settings.set: clé inconnue ignorée '{k}'");
            }
        }
    }

    if !keys.is_empty() {
        state.emit_event(|sequence| crate::services::DaemonEvent::SettingsChanged {
            sequence,
            keys: keys.iter().map(|k| k.to_string()).collect(),
        });
    }

    Ok(json!({
        "success": true,
        "defaultDownloadDir": updated_dir,
        "historyRetentionDays": updated_retention,
        "maxGlobalDownloadPeers": updated_max_global,
        "maxPeersPerDownload": updated_max_per_file,
        "minPeersPerDownload": updated_min_per_file,
        "shareDownloadDir": updated_share_dir,
        "nickname": params.get("nickname").and_then(|v| v.as_str()),
        "obfuscationEnabled": params.get("obfuscationEnabled").and_then(|v| v.as_bool()),
        "obfuscationRequired": params.get("obfuscationRequired").and_then(|v| v.as_bool()),
        "maxConnections": params.get("maxConnections").and_then(|v| v.as_u64()),
        "maxConnPerMin": params.get("maxConnPerMin").and_then(|v| v.as_u64()),
        "maxUploadQueue": params.get("maxUploadQueue").and_then(|v| v.as_u64()),
        "ussEnabled": params.get("ussEnabled").and_then(|v| v.as_bool()),
    }))
}

pub async fn handle_settings_set_retention_days(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params object"))?;
    let retention_days = params["retentionDays"]
        .as_u64()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'retentionDays' parameter"))?
        as u32;

    let mut dls = state.downloads.lock().await;
    dls.set_setting("history_retention_days", &retention_days.to_string())?;
    let pruned_count = dls.apply_retention_policy(retention_days)?;
    drop(dls);
    state.emit_event(|sequence| crate::services::DaemonEvent::SettingsChanged {
        sequence,
        keys: vec!["historyRetentionDays".to_string()],
    });
    Ok(json!({
        "success": true,
        "retentionDays": retention_days,
        "prunedCount": pruned_count
    }))
}

pub async fn handle_settings_reset_download_dir(state: &AppState) -> Result<Value, JsonRpcError> {
    state
        .downloads
        .lock()
        .await
        .remove_setting("default_download_directory")?;

    let portable_default =
        crate::workdir::WorkDirectoryResolver::resolve(None, None).default_download_dir;
    state.emit_event(|sequence| crate::services::DaemonEvent::SettingsChanged {
        sequence,
        keys: vec!["defaultDownloadDir".to_string()],
    });
    Ok(json!({
        "success": true,
        "defaultDownloadDir": portable_default.to_string_lossy(),
        "isCustom": false
    }))
}

/// Applies a TCP/UDP port change at runtime (eMule « Port client »):
/// persisted values are reloaded into the effective-port atomics, the
/// inbound TCP listener is rebound if it was listening, the Kad UDP socket
/// is re-bound on the new port (when UDP is on), and buddy/UPnP advertised
/// ports are updated.
async fn apply_port_change(state: &AppState) -> Result<(), JsonRpcError> {
    let (tcp, udp) = {
        let dl = state.downloads.lock().await;
        let tcp = dl
            .get_setting("ed2k_port")?
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or_else(|| state.ed2k_port());
        let udp = dl
            .get_setting("kad_port")?
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or_else(|| state.kad_port());
        (tcp, udp)
    };
    state.set_effective_ports(tcp, udp);
    state.kad.set_ed2k_tcp_port(tcp);
    let (obf_e, obf_r) = state.effective_obfuscation().await;
    let opts: u8 = if obf_e { 0x03 } else { 0 } | if obf_r { 0x04 } else { 0 };
    state.kad.udp().set_buddy_tcp_info(tcp, opts);
    state.upnp.set_ports(tcp, udp).await;

    let profile = state.network_policy.profile();
    let bind_ip = if profile.is_loopback_only() {
        "127.0.0.1"
    } else {
        "0.0.0.0"
    };

    // Rebind the inbound TCP listener when it was already listening.
    if state.inbound_listener.snapshot().await.is_listening {
        state.inbound_listener.stop().await;
        let addr: std::net::SocketAddr = format!("{bind_ip}:{tcp}")
            .parse()
            .map_err(|e| JsonRpcError::invalid_params(&format!("Invalid bind addr: {e}")))?;
        state
            .inbound_listener
            .start(addr)
            .await
            .map_err(|e| JsonRpcError::business(format!("TCP rebind failed: {e}")))?;
    }

    // Rebind Kad UDP on the new port when UDP is enabled and allowed.
    if state.udp_enabled() && profile.allows_udp_kad() {
        let _ = state.kad.udp().stop().await;
        if let Ok(addr) = format!("{bind_ip}:{udp}").parse() {
            if let Err(e) = state.kad.udp().start(addr).await {
                tracing::warn!("Kad UDP rebind failed on {addr}: {e}");
            }
        }
    }
    Ok(())
}

/// Starts/stops the UDP services per the `udp_enabled` toggle (eMule
/// « UDP désactivé »). Kad UDP + server UDP share the toggle; the client
/// UDP layer rides the Kad socket so it follows automatically.
pub(crate) async fn apply_udp_state(state: &AppState, enabled: bool) {
    let profile = state.network_policy.profile();
    if enabled && profile.allows_udp_kad() {
        let ip = if profile.is_loopback_only() {
            "127.0.0.1"
        } else {
            "0.0.0.0"
        };
        if let Ok(addr) = format!("{ip}:{}", state.kad_port()).parse() {
            if let Err(e) = state.kad.udp().start(addr).await {
                tracing::warn!("Kad UDP start failed on {addr}: {e}");
            }
        }
        if let Ok(ip_addr) = ip.parse() {
            let _ = state.server_udp.start(ip_addr).await;
        }
    } else {
        let _ = state.kad.udp().stop().await;
        state.server_udp.stop().await;
    }
}
