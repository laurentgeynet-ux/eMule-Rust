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
    let mut bind_addr_arg: Option<std::net::SocketAddr> = None;
    let mut auth_token_arg: Option<String> = None;
    let mut token_file_arg: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--workdir" if i + 1 < args.len() => {
                data_dir_arg = Some(PathBuf::from(&args[i + 1]));
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
            _ => {
                i += 1;
            }
        }
    }

    let data_dir = data_dir_arg.unwrap_or_else(DaemonConfig::default_data_dir);
    std::fs::create_dir_all(&data_dir)?;

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
        let tok = uuid::Uuid::new_v4().to_string();
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

    let config = DaemonConfig::new(data_dir.clone(), auth_token).with_bind_addr(actual_addr);
    let downloads = DownloadService::new(&db_path, temp_dir)?;
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

    let app = create_router(state);

    // 5. Run with graceful shutdown
    let data_dir_cleanup = data_dir.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            println!("\n==> Shutting down mule-daemon...");
            DaemonSession::remove(&data_dir_cleanup);
        })
        .await?;

    DaemonSession::remove(&data_dir);
    println!("==> mule-daemon exited cleanly.");
    Ok(())
}
