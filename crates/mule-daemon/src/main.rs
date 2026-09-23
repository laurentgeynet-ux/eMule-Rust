use mule_daemon::{
    create_router, AppState, DaemonConfig, DaemonSession, DirectoryLock, DownloadService,
};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut data_dir_arg: Option<PathBuf> = None;
    let mut downloads_dir_arg: Option<PathBuf> = None;
    let mut mode_override: Option<mule_daemon::WorkDirMode> = None;
    let mut bind_addr_arg: Option<std::net::SocketAddr> = None;
    let mut auth_token_arg: Option<String> = None;
    let mut token_file_arg: Option<PathBuf> = None;
    let mut parent_pid_arg: Option<u32> = None;
    let mut profile_arg: Option<mule_network_policy::NetworkProfile> = None;
    let mut ed2k_port_arg: Option<u16> = None;
    let mut kad_port_arg: Option<u16> = None;
    let mut upnp_arg: Option<bool> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--workdir" if i + 1 < args.len() => {
                data_dir_arg = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--downloads-dir" | "--download-dir" if i + 1 < args.len() => {
                downloads_dir_arg = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--mode" | "--workdir-mode" if i + 1 < args.len() => {
                mode_override = match args[i + 1].to_lowercase().as_str() {
                    "portable" => Some(mule_daemon::WorkDirMode::Portable),
                    "installeddefault" | "installed" => {
                        Some(mule_daemon::WorkDirMode::InstalledDefault)
                    }
                    "environment" | "env" => Some(mule_daemon::WorkDirMode::Environment),
                    "explicitcli" | "cli" => Some(mule_daemon::WorkDirMode::ExplicitCli),
                    _ => None,
                };
                i += 2;
            }
            "--profile" | "--network-profile" if i + 1 < args.len() => {
                if let Ok(p) = args[i + 1].parse() {
                    profile_arg = Some(p);
                }
                i += 2;
            }
            "--ed2k-port" if i + 1 < args.len() => {
                if let Ok(port) = args[i + 1].parse::<u16>() {
                    ed2k_port_arg = Some(port);
                }
                i += 2;
            }
            "--kad-port" if i + 1 < args.len() => {
                if let Ok(port) = args[i + 1].parse::<u16>() {
                    kad_port_arg = Some(port);
                }
                i += 2;
            }
            "--upnp" if i + 1 < args.len() => {
                upnp_arg = match args[i + 1].to_lowercase().as_str() {
                    "true" | "1" | "yes" | "enable" => Some(true),
                    "false" | "0" | "no" | "disable" => Some(false),
                    _ => None,
                };
                i += 2;
            }
            "--bind" if i + 1 < args.len() => {
                if let Ok(addr) = args[i + 1].parse() {
                    bind_addr_arg = Some(addr);
                }
                i += 2;
            }
            "--token" if i + 1 < args.len() => {
                auth_token_arg = Some(args[i + 1].clone());
                i += 2;
            }
            "--token-file" if i + 1 < args.len() => {
                token_file_arg = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--parent-pid" if i + 1 < args.len() => {
                if let Ok(pid) = args[i + 1].parse::<u32>() {
                    parent_pid_arg = Some(pid);
                }
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    let mut resolved_workdir =
        mule_daemon::WorkDirectoryResolver::resolve(data_dir_arg, downloads_dir_arg);
    if let Some(mode) = mode_override {
        resolved_workdir.mode = mode;
    } else if resolved_workdir.mode == mule_daemon::WorkDirMode::ExplicitCli {
        // If an explicit workdir path was passed, check if it lives in a portable bundle root
        // (i.e. <bundle_root>/portable.flag exists)
        if let Some(parent) = resolved_workdir.workdir.parent() {
            if parent.join(mule_daemon::PORTABLE_FLAG_FILENAME).exists() {
                resolved_workdir.mode = mule_daemon::WorkDirMode::Portable;
            }
        }
    }
    let data_dir = resolved_workdir.workdir;
    let default_download_dir = resolved_workdir.default_download_dir;
    println!(
        "==> Workdir mode: {:?} ({})",
        resolved_workdir.mode,
        data_dir.display()
    );
    println!(
        "==> Default download dir: {}",
        default_download_dir.display()
    );
    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(&default_download_dir)?;

    // Configure file appender for daemon.log (reset on each startup)
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(data_dir.join("daemon.log"))
        .ok();

    let timer = tracing_subscriber::fmt::time::OffsetTime::local_rfc_3339().unwrap_or_else(|_| {
        tracing_subscriber::fmt::time::OffsetTime::new(
            time::UtcOffset::UTC,
            time::format_description::well_known::Rfc3339,
        )
    });

    let file_layer = log_file.map(|f| {
        tracing_subscriber::fmt::layer()
            .with_writer(std::sync::Arc::new(f))
            .with_ansi(false)
            .with_timer(timer.clone())
    });

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_timer(timer))
        .with(file_layer)
        .init();

    // 1. Acquire exclusive directory lock
    let _lock = DirectoryLock::acquire(&data_dir).map_err(|e| {
        eprintln!("==> ERROR: {e}");
        e
    })?;

    let db_path = data_dir.join("mule.db");
    let temp_dir = data_dir.join("temp");
    std::fs::create_dir_all(&temp_dir)?;

    // 2. Resolve authentication token
    let auth_token = if let Some(tok) = auth_token_arg {
        DaemonSession::write_token(&data_dir, &tok)?;
        tok
    } else if let Some(token_file) = token_file_arg {
        let tok = std::fs::read_to_string(token_file)?.trim().to_string();
        DaemonSession::write_token(&data_dir, &tok)?;
        tok
    } else if let Ok(tok) = std::env::var("MULE_AUTH_TOKEN") {
        DaemonSession::write_token(&data_dir, &tok)?;
        tok
    } else if let Some(existing) = DaemonSession::read_token(&data_dir) {
        existing
    } else {
        let tok = DaemonSession::generate_token();
        DaemonSession::write_token(&data_dir, &tok)?;
        println!("==> Generated session auth token stored in auth.token (redacted)");
        tok
    };

    // 3. Resolve socket bind address with automatic ephemeral fallback if port is busy
    let default_bind: std::net::SocketAddr = "127.0.0.1:4660".parse().unwrap();
    let requested_bind = bind_addr_arg
        .or_else(|| {
            std::env::var("MULE_BIND_ADDR")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(default_bind);

    let listener = match tokio::net::TcpListener::bind(requested_bind).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && bind_addr_arg.is_none() => {
            eprintln!(
                "==> Warning: Port {requested_bind} is in use. Falling back to ephemeral port..."
            );
            let fallback_addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
            tokio::net::TcpListener::bind(fallback_addr).await?
        }
        Err(e) => return Err(e.into()),
    };

    let actual_addr = listener.local_addr()?;
    let endpoint = format!("ws://{actual_addr}/rpc");

    let mut downloads = DownloadService::new(&db_path, temp_dir)?;
    let initial_download_dir = downloads
        .get_setting("default_download_directory")
        .ok()
        .flatten()
        .map(PathBuf::from)
        .unwrap_or(default_download_dir);

    // eMule loads every .part.met found in the temp directory at startup:
    // orphans (lost DB state or files dropped in from eMule) are recovered
    // as paused downloads via the sandboxed import path.
    let recovered = downloads.recover_orphan_part_mets(&initial_download_dir);
    if recovered > 0 {
        println!("==> Recovered {recovered} orphaned .part.met download(s) from temp directory");
    }

    let ed2k_port = ed2k_port_arg.unwrap_or(39735);
    let kad_port = kad_port_arg.unwrap_or(39737);
    let upnp_enabled = upnp_arg.unwrap_or(true);
    let network_profile =
        profile_arg.unwrap_or(mule_network_policy::NetworkProfile::PublicEd2kKadPreview);

    let config = DaemonConfig::new(data_dir.clone(), auth_token)
        .with_bind_addr(actual_addr)
        .with_workdir_mode(resolved_workdir.mode)
        .with_default_download_dir(initial_download_dir)
        .with_network_profile(network_profile)
        .with_upnp_enabled(upnp_enabled)
        .with_ports(ed2k_port, kad_port);
    let state = AppState::new(config, downloads);
    // eMule `Autoconnect`/`Reconnect`/`AutoDeadServer`: background watcher
    // reconnecting to the last server target (or best known server) when
    // `server_reconnect`/`server_autoconnect` settings are enabled.
    state.spawn_server_watcher();
    // Emule-Sion : rotation périodique d'identité (`identityRotationHours`)
    // et quota journalier d'upload (`dailyUploadQuotaGb`).
    state.spawn_identity_rotator();
    state.spawn_quota_watcher();

    // eMule "Incoming" folder sharing: hash every file sitting in the
    // download directory and register it as shared (background task —
    // hashing can be slow on large files). Opt-out via the
    // `share_download_dir` setting.
    {
        let dl = std::sync::Arc::clone(&state.downloads);
        let dir = state.config.default_download_dir.clone();
        tokio::spawn(async move {
            mule_daemon::services::incoming_share::scan_download_dir_shares(&dl, dir).await;
        });
    }

    // 4. Atomically write session file
    let session = DaemonSession::new(1, state.instance_id, &endpoint);
    session.write_atomic(&data_dir)?;

    println!("==> mule-daemon listening on http://{actual_addr}");
    println!("==> WebSocket RPC endpoint: {endpoint}");
    println!("==> Network profile: {network_profile}");
    println!("==> eD2K port: {ed2k_port}, Kad port: {kad_port}, UPnP: {upnp_enabled}");
    println!(
        "==> Session file: {}",
        data_dir.join("session.json").display()
    );

    // If UPnP is enabled and in a preview profile, attempt background port mapping with progressive retries and periodic refresh
    let eff_ed2k_port = state.ed2k_port();
    let eff_kad_port = state.kad_port();
    let eff_udp = state.udp_enabled();
    let eff_upnp = state.upnp.snapshot().await.enabled;
    println!(
        "==> Effective ports: TCP {eff_ed2k_port}, UDP {eff_kad_port} (udp_enabled={eff_udp}, upnp={eff_upnp})"
    );

    if eff_upnp && network_profile.is_preview() {
        let upnp_svc = state.upnp.clone();
        let cancel_token = state.network_policy.cancellation_token();
        let cancel_token_retry = cancel_token.clone();
        tokio::spawn(async move {
            let retry_delays = [
                tokio::time::Duration::from_secs(30),
                tokio::time::Duration::from_secs(120),
                tokio::time::Duration::from_secs(300),
            ];
            let mut result = upnp_svc.map_ports(eff_ed2k_port, eff_kad_port).await;
            if result.is_err() {
                for delay in retry_delays {
                    tokio::select! {
                        _ = cancel_token_retry.cancelled() => break,
                        _ = tokio::time::sleep(delay) => {}
                    }
                    tracing::info!("Retrying UPnP port mapping after initial failure...");
                    result = upnp_svc.map_ports(eff_ed2k_port, eff_kad_port).await;
                    if result.is_ok() {
                        tracing::info!("UPnP port mapping succeeded on background retry!");
                        break;
                    }
                }
            }
        });
        let refresh_handle = state
            .upnp
            .clone()
            .start_periodic_refresh(tokio::time::Duration::from_secs(45 * 60), cancel_token);
        state.register_task(refresh_handle.abort_handle()).await;
    }

    // Auto-initialize Kad routing table and UDP socket if in a preview profile
    if network_profile.allows_udp_kad() && eff_udp {
        let kad_svc = state.kad.clone();
        let kad_cancel = state.network_policy.cancellation_token();
        let kad_bind_ip = if network_profile.is_loopback_only() {
            "127.0.0.1"
        } else {
            "0.0.0.0"
        };
        let kad_bind_addr: std::net::SocketAddr =
            format!("{kad_bind_ip}:{eff_kad_port}").parse().unwrap();
        let nodes_dat_path = state.config.data_dir.join("nodes.dat");
        tokio::spawn(async move {
            kad_svc.init_from_db().await;
            if let Err(e) = kad_svc.udp().start(kad_bind_addr).await {
                tracing::warn!("Failed to auto-start Kad UDP on {kad_bind_addr}: {e}");
            } else {
                tracing::info!("Kad UDP socket auto-started on {kad_bind_addr}");
                // nodes.dat à la racine du dossier data : amorçage
                // optionnel — eMule le lit dans `s_liBootstrapList` à
                // chaque démarrage ; les contacts prouvent leur vivacité
                // en répondant au BOOTSTRAP_REQ du drain.
                kad_svc.load_nodes_dat_candidates(&nodes_dat_path).await;
                kad_svc.bootstrap_active_contacts(15).await;
                kad_svc.start_background_maintenance(kad_cancel);
            }
        });

        // Server UDP socket (OP_GLOBGETSOURCES2 / OP_GLOBFOUNDSOURCES) —
        // separate ephemeral socket like eMule's `UDPSocket`.
        let server_udp_svc = state.server_udp.clone();
        let server_udp_bind_ip: std::net::IpAddr = kad_bind_ip.parse().unwrap();
        tokio::spawn(async move {
            match server_udp_svc.start(server_udp_bind_ip).await {
                Ok(bound) => tracing::info!("Server UDP socket auto-started on {bound}"),
                Err(e) => tracing::warn!("Failed to auto-start server UDP: {e}"),
            }
        });
    }

    // Auto-start Inbound TCP Listener on the eD2K port if in a preview or enabled profile
    if network_profile.is_preview() || network_profile.is_loopback_only() {
        let listener_svc = state.inbound_listener.clone();
        let inbound_bind_ip = if network_profile.is_loopback_only() {
            "127.0.0.1"
        } else {
            "0.0.0.0"
        };
        let inbound_bind_addr: std::net::SocketAddr =
            format!("{inbound_bind_ip}:{eff_ed2k_port}").parse().unwrap();
        tokio::spawn(async move {
            match listener_svc.start(inbound_bind_addr).await {
                Ok(bound) => {
                    println!("==> Inbound eD2K TCP listener auto-started on {bound}");
                }
                Err(e) => {
                    eprintln!("==> Warning: Failed to auto-start Inbound TCP listener on {inbound_bind_addr}: {e}");
                }
            }
        });
    }

    // USS (Upload Speed Sense) — dynamic upload limiter driven by latency
    // probes to the connected server (idles while disabled).
    state.uss.start(
        state.network_policy.rate_limiters(),
        state.network_policy.cancellation_token(),
    );

    // Auto-start autonomous Swarm Coordinator for background source discovery & peer connections
    let coordinator_handle = mule_daemon::services::SwarmCoordinator::start(state.clone());
    state.register_task(coordinator_handle.abort_handle()).await;

    let shutdown_notify = state.shutdown_notify.clone();
    let upnp_cleanup = state.upnp.clone();
    let listener_cleanup = state.inbound_listener.clone();
    let downloads_cleanup = state.downloads.clone();
    let app = create_router(state);

    if let Some(parent_pid) = parent_pid_arg {
        if parent_pid > 0 {
            spawn_parent_watchdog(parent_pid, shutdown_notify.clone());
        }
    }

    // 5. Run with graceful shutdown
    let data_dir_cleanup = data_dir.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    println!("\n==> Shutting down mule-daemon via Ctrl+C...");
                }
                _ = shutdown_notify.notified() => {
                    println!("\n==> Shutting down mule-daemon via shutdown signal/watchdog...");
                }
            }
            DaemonSession::remove(&data_dir_cleanup);
            listener_cleanup.stop().await;
            // eMule saves every .part.met on exit so crash-resume state is
            // never stale (CPartFile::~CPartFile → SavePartFile).
            {
                let dl = downloads_cleanup.lock().await;
                dl.save_active_part_mets();
            }
            let _ = tokio::time::timeout(
                tokio::time::Duration::from_millis(500),
                upnp_cleanup.unmap_ports(),
            )
            .await;
        })
        .await?;

    DaemonSession::remove(&data_dir);
    println!("==> mule-daemon exited cleanly.");
    Ok(())
}

#[cfg(windows)]
fn spawn_parent_watchdog(parent_pid: u32, shutdown_notify: std::sync::Arc<tokio::sync::Notify>) {
    tokio::task::spawn_blocking(move || {
        #[link(name = "kernel32")]
        extern "system" {
            fn OpenProcess(
                dwDesiredAccess: u32,
                bInheritHandle: i32,
                dwProcessId: u32,
            ) -> *mut std::ffi::c_void;
            fn WaitForSingleObject(hHandle: *mut std::ffi::c_void, dwMilliseconds: u32) -> u32;
            fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
        }

        const SYNCHRONIZE: u32 = 0x00100000;
        let handle = unsafe { OpenProcess(SYNCHRONIZE, 0, parent_pid) };
        if handle.is_null() {
            eprintln!(
                "==> Parent process PID {parent_pid} not running or inaccessible; initiating exit."
            );
            shutdown_notify.notify_waiters();
            return;
        }
        println!("==> Watching parent process PID {parent_pid} for auto-teardown...");
        unsafe {
            WaitForSingleObject(handle, 0xFFFFFFFF);
            CloseHandle(handle);
        }
        println!("==> Parent process PID {parent_pid} terminated. Initiating graceful daemon shutdown...");
        shutdown_notify.notify_waiters();
    });
}

#[cfg(not(windows))]
fn spawn_parent_watchdog(parent_pid: u32, shutdown_notify: std::sync::Arc<tokio::sync::Notify>) {
    tokio::spawn(async move {
        println!("==> Watching parent process PID {parent_pid} for auto-teardown...");
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            let exists = std::path::Path::new(&format!("/proc/{parent_pid}")).exists();
            if !exists {
                println!("==> Parent process PID {parent_pid} terminated. Initiating graceful daemon shutdown...");
                shutdown_notify.notify_waiters();
                break;
            }
        }
    });
}
