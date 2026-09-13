use mule_daemon::{create_router, AppState, DaemonConfig, DownloadService};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args: Vec<String> = std::env::args().collect();
    let mut data_dir_arg: Option<PathBuf> = None;
    let mut bind_addr_arg: Option<std::net::SocketAddr> = None;
    let mut auth_token_arg: Option<String> = None;

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
            _ => {
                i += 1;
            }
        }
    }

    let data_dir = data_dir_arg.unwrap_or_else(dirs_or_fallback);
    let db_path = data_dir.join("mule.db");
    let temp_dir = data_dir.join("temp");

    let auth_token = auth_token_arg
        .or_else(|| std::env::var("MULE_AUTH_TOKEN").ok())
        .unwrap_or_else(|| {
            let tok = uuid::Uuid::new_v4().to_string();
            println!("==> Generated session auth token: {tok}");
            tok
        });

    let bind_addr = bind_addr_arg
        .or_else(|| std::env::var("MULE_BIND_ADDR").ok().and_then(|s| s.parse().ok()))
        .unwrap_or_else(|| "127.0.0.1:4660".parse().unwrap());

    let config = DaemonConfig::new(data_dir, auth_token).with_bind_addr(bind_addr);
    let listener_addr = config.bind_addr;

    let downloads = DownloadService::new(&db_path, temp_dir)?;
    let state = AppState::new(config, downloads);

    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind(listener_addr).await?;

    println!("==> mule-daemon listening on http://{listener_addr}");
    println!("==> WebSocket RPC endpoint: ws://{listener_addr}/rpc");

    axum::serve(listener, app).await?;
    Ok(())
}

fn dirs_or_fallback() -> PathBuf {
    if let Ok(dir) = std::env::var("MULE_DATA_DIR") {
        PathBuf::from(dir)
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".mule_data")
    }
}
