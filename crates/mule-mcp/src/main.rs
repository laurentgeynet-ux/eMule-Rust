use clap::Parser;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use mule_mcp::{
    client::DaemonClient,
    protocol::{
        make_error, make_response, JsonRpcError, JsonRpcMessage, ToolCallResult,
        MCP_PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION,
    },
    tools::{execute_tool, list_tools},
};

#[derive(Parser, Debug, Clone)]
#[command(name = "mule-mcp")]
#[command(about = "Model Context Protocol (MCP) server for EMule-Rust")]
struct Cli {
    /// Explicit WebSocket RPC URL of the EMule-Rust daemon (e.g. ws://127.0.0.1:4660/rpc)
    #[arg(long)]
    url: Option<String>,

    /// Session authentication token for the daemon
    #[arg(long)]
    token: Option<String>,

    /// Path to EMule-Rust workdir where session.json and auth.token reside
    #[arg(long)]
    workdir: Option<PathBuf>,

    /// Path to eMuleRust.exe executable or application bundle root
    #[arg(long)]
    app: Option<PathBuf>,
}

struct AppState {
    cli: Cli,
    client: Option<DaemonClient>,
}

impl AppState {
    async fn get_or_reconnect_client(&mut self) -> Result<DaemonClient, String> {
        if let Some(ref c) = self.client {
            // Test if connection is still alive via a quick status check
            if c.call("system.status", json!({})).await.is_ok() {
                return Ok(c.clone());
            }
            warn!("Previous daemon connection lost. Reconnecting...");
            self.client = None;
        }

        let new_client = DaemonClient::connect(
            self.cli.url.clone(),
            self.cli.token.clone(),
            self.cli.workdir.clone(),
            self.cli.app.clone(),
        )
        .await?;

        self.client = Some(new_client.clone());
        Ok(new_client)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // IMPORTANT: Log exclusively to stderr so stdout remains clean for MCP JSON-RPC protocol
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mule_mcp=info".into()),
        )
        .init();

    let cli = Cli::parse();
    info!("Starting EMule-Rust MCP Server v{SERVER_VERSION}...");

    // Attempt non-fatal initial connection to daemon
    let initial_client = match DaemonClient::connect(
        cli.url.clone(),
        cli.token.clone(),
        cli.workdir.clone(),
        cli.app.clone(),
    )
    .await
    {
        Ok(c) => {
            info!("Daemon connection established on startup.");
            Some(c)
        }
        Err(e) => {
            warn!("EMule-Rust daemon not currently reachable ({e}). MCP server will connect on demand.");
            None
        }
    };

    let state = Arc::new(Mutex::new(AppState {
        cli,
        client: initial_client,
    }));

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    info!("MCP stdio listening loop ready");

    while let Ok(Some(line)) = reader.next_line().await {
        let trimmed = line.trim_start_matches('\u{feff}').trim();
        if trimmed.is_empty() {
            continue;
        }

        let msg: JsonRpcMessage = match serde_json::from_str(trimmed) {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to parse incoming JSON: {e}");
                let err_resp = make_error(Value::Null, JsonRpcError::parse_error());
                send_json(&mut stdout, &err_resp).await?;
                continue;
            }
        };

        // If it's a notification without an id, check if it's notifications/initialized
        let method = match msg.method.as_deref() {
            Some(m) => m,
            None => continue,
        };

        let req_id = match msg.id {
            Some(id) => id,
            None => {
                // Notification (no response needed)
                if method == "notifications/initialized" {
                    info!("MCP client initialized notification received");
                }
                continue;
            }
        };

        match method {
            "initialize" => {
                let res = json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": SERVER_NAME,
                        "version": SERVER_VERSION
                    }
                });
                let resp = make_response(req_id, res);
                send_json(&mut stdout, &resp).await?;
            }

            "ping" => {
                let resp = make_response(req_id, json!({}));
                send_json(&mut stdout, &resp).await?;
            }

            "tools/list" => {
                let tools = list_tools();
                let res = json!({ "tools": tools });
                let resp = make_response(req_id, res);
                send_json(&mut stdout, &resp).await?;
            }

            "tools/call" => {
                let params = msg.params.unwrap_or(json!({}));
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));

                let client_res = {
                    let mut st = state.lock().await;
                    st.get_or_reconnect_client().await
                };

                let tool_result = match client_res {
                    Ok(client) => execute_tool(&client, tool_name, args).await,
                    Err(e) => ToolCallResult::error(format!(
                        "Could not connect to EMule-Rust daemon: {e}. Please ensure EMule-Rust is running."
                    )),
                };

                let resp = make_response(
                    req_id,
                    serde_json::to_value(tool_result).unwrap_or_default(),
                );
                send_json(&mut stdout, &resp).await?;
            }

            unknown => {
                warn!("Received unknown MCP method: {unknown}");
                let resp = make_error(req_id, JsonRpcError::method_not_found(unknown));
                send_json(&mut stdout, &resp).await?;
            }
        }
    }

    info!("MCP server shutting down");
    Ok(())
}

async fn send_json(
    stdout: &mut tokio::io::Stdout,
    val: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut data = serde_json::to_vec(val)?;
    data.push(b'\n');
    stdout.write_all(&data).await?;
    stdout.flush().await?;
    Ok(())
}
