// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! `mule-assistant` : REPL terminal ou serveur WebSocket pour piloter
//! eMuleRust via un LLM local (Ollama loopback par défaut).

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tokio::sync::mpsc;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use mule_assistant::config::{AssistantConfig, DEFAULT_ENDPOINT, DEFAULT_MODEL, DEFAULT_WS_PORT};
use mule_assistant::llm::ollama::OllamaBackend;
use mule_assistant::repl::run_repl;
use mule_assistant::session::Session;
use mule_assistant::ws::serve_ws;
use mule_daemon::{DaemonSession, WorkDirectoryResolver};
use mule_mcp::tools::ToolProfile;

#[derive(Parser, Debug)]
#[command(name = "mule-assistant")]
#[command(about = "Assistant IA local pour eMuleRust (tool-calling LLM → daemon RPC)")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Explicit WebSocket RPC URL of the eMuleRust daemon (e.g. ws://127.0.0.1:4660/rpc)
    #[arg(long)]
    url: Option<String>,

    /// Session authentication token for the daemon
    #[arg(long)]
    token: Option<String>,

    /// Path to eMuleRust workdir where session.json and auth.token reside
    #[arg(long)]
    workdir: Option<PathBuf>,

    /// Path to eMuleRust.exe executable or application bundle root
    #[arg(long)]
    app: Option<PathBuf>,

    /// LLM backend base URL — loopback IP literal required unless
    /// --allow-remote-endpoint (default: http://127.0.0.1:11434)
    #[arg(long, default_value = DEFAULT_ENDPOINT)]
    endpoint: String,

    /// Model name on the backend (must support Ollama tool calling)
    #[arg(long, default_value = DEFAULT_MODEL)]
    model: String,

    /// Explicitly allow a non-loopback LLM endpoint
    #[arg(long)]
    allow_remote_endpoint: bool,

    /// Tool risk ceiling presented to the model: read, safe, full
    #[arg(long, value_enum, default_value_t = ToolProfile::Full)]
    tool_profile: ToolProfile,
}

#[derive(Subcommand, Debug, Clone)]
enum Command {
    /// Interactive terminal session (default)
    Chat,
    /// One-shot prompt then exit
    Prompt {
        /// Texte du prompt utilisateur
        text: String,
    },
    /// WebSocket server for the Flutter UI (ws://127.0.0.1:<port>/assistant)
    Serve {
        /// Loopback port to bind
        #[arg(long, default_value_t = DEFAULT_WS_PORT)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Logs humains sur stderr uniquement — stdout reste l'interface REPL.
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mule_assistant=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let config = AssistantConfig {
        endpoint: cli.endpoint.clone(),
        model: cli.model.clone(),
        allow_remote_endpoint: cli.allow_remote_endpoint,
        tool_profile: cli.tool_profile,
        ..Default::default()
    };
    config.validate()?;

    let backend = Box::new(OllamaBackend::new(&config)?);
    let session = Session::connect(
        backend,
        config.clone(),
        cli.url.clone(),
        cli.token.clone(),
        cli.workdir.clone(),
        cli.app.clone(),
    )
    .await?;

    info!(
        "session prête — {} outils visibles (profil {})",
        session.visible_tool_count(),
        cli.tool_profile.as_str()
    );

    match cli.command.clone().unwrap_or(Command::Chat) {
        Command::Chat => {
            let mut session = session;
            run_repl(&mut session).await?;
        }
        Command::Prompt { text } => {
            let mut session = session;
            let (events_tx, mut events_rx) = mpsc::unbounded_channel();
            let (confirm_tx, _confirm_rx) = mpsc::channel(8);
            // Sans répondeur de confirmation : les appels à confirmer sont
            // refusés (défaut sûr en mode non interactif).
            let turn = session.user_turn(&text, &events_tx, &confirm_tx);
            tokio::pin!(turn);
            loop {
                tokio::select! {
                    res = &mut turn => {
                        while let Ok(ev) = events_rx.try_recv() {
                            print_event_oneshot(&ev);
                        }
                        res?;
                        break;
                    }
                    ev = events_rx.recv() => {
                        if let Some(e) = ev { print_event_oneshot(&e); }
                    }
                }
            }
        }
        Command::Serve { port } => {
            let bind = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
            let token = resolve_expected_token(&cli).ok_or(
                "jeton de session daemon introuvable — passer --token explicitement",
            )?;
            let listener = tokio::net::TcpListener::bind(bind).await?;
            serve_ws(listener, session, token).await?;
        }
    }
    Ok(())
}

/// Jeton attendu par `assistant.hello` : explicite `--token` ou jeton de
/// la session daemon du workdir — même source que `DaemonClient`.
fn resolve_expected_token(cli: &Cli) -> Option<String> {
    if let Some(t) = &cli.token {
        return Some(t.clone());
    }
    let workdir = cli.workdir.clone().unwrap_or_else(|| {
        WorkDirectoryResolver::resolve(None, None).workdir
    });
    DaemonSession::read_token(&workdir)
}

/// Affichage compact d'un event en mode one-shot.
fn print_event_oneshot(ev: &mule_assistant::AssistantEvent) {
    use mule_assistant::AssistantEvent as E;
    match ev {
        E::Token { content } => print!("{content}"),
        E::ToolStart { name, args } => println!("\n[outil →] {name} {args}"),
        E::ToolResult {
            name,
            is_error,
            content,
        } => println!("[résultat ←] {name} (err={is_error}): {content}"),
        E::Final { content } => println!("\n{content}"),
        E::Notice { message } | E::Error { message } => println!("[info] {message}"),
    }
    let _ = std::io::stdout().flush();
}
