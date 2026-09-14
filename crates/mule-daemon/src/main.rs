use mule_daemon::{
    create_router, AppState, DaemonConfig, DaemonSession, DirectoryLock, DownloadService,
};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args: Vec<String> = std::env::args().collect();
    let mut data_dir_arg: Option<PathBuf> = None;
    let mut downloads_dir_arg: Option<PathBuf> = None;
    let mut mode_override: Option<mule_daemon::WorkDirMode> = None;
    let mut bind_addr_arg: Option<std::net::SocketAddr> = None;
    let mut auth_token_arg: Option<String> = None;
    let mut token_file_arg: Option<PathBuf> = None;
    let mut parent_pid_arg: Option<u32> = None;

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
                    "installeddefault" | "installed" => Some(mule_daemon::WorkDirMode::InstalledDefault),
                    "environment" | "env" => Some(mule_daemon::WorkDirMode::Environment),
                    "explicitcli" | "cli" => Some(mule_daemon::WorkDirMode::ExplicitCli),
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

    let mut resolved_workdir = mule_daemon::WorkDirectoryResolver::resolve(data_dir_arg, downloads_dir_arg);
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
    println!("==> Workdir mode: {:?} ({})", resolved_workdir.mode, data_dir.display());
    println!("==> Default download dir: {}", default_download_dir.display());
    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(&default_download_dir)?;

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

    let downloads = DownloadService::new(&db_path, temp_dir)?;
    let initial_download_dir = downloads
        .get_setting("default_download_directory")
        .ok()
        .flatten()
        .map(PathBuf::from)
        .unwrap_or(default_download_dir);

    let config = DaemonConfig::new(data_dir.clone(), auth_token)
        .with_bind_addr(actual_addr)
        .with_workdir_mode(resolved_workdir.mode)
        .with_default_download_dir(initial_download_dir);
    let state = AppState::new(config, downloads);

    // 4. Atomically write session file
    let session = DaemonSession::new(1, state.instance_id, &endpoint);
    session.write_atomic(&data_dir)?;

    println!("==> mule-daemon listening on http://{actual_addr}");
    println!("==> WebSocket RPC endpoint: {endpoint}");
    println!(
        "==> Session file: {}",
        data_dir.join("session.json").display()
    );

    let shutdown_notify = state.shutdown_notify.clone();
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
            eprintln!("==> Parent process PID {parent_pid} not running or inaccessible; initiating exit.");
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
