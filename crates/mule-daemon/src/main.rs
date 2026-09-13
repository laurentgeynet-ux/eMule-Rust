use mule_daemon::{create_router, AppState, DaemonConfig, DownloadService};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let data_dir = dirs_or_fallback();
    let db_path = data_dir.join("mule.db");
    let temp_dir = data_dir.join("temp");

    let auth_token = std::env::var("MULE_AUTH_TOKEN").unwrap_or_else(|_| {
        let tok = uuid::Uuid::new_v4().to_string();
        println!("==> Generated session auth token: {tok}");
        tok
    });

    let config = DaemonConfig::new(data_dir, auth_token);
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
