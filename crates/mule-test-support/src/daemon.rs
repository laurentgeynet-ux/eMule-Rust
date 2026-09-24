// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Daemons de test : instance in-process derrière le routeur axum (mémoire ou
//! persistante avec verrou de répertoire).

use std::net::SocketAddr;
use std::path::PathBuf;

use tempfile::{tempdir, TempDir};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use mule_daemon::{
    create_router, AppState, DaemonConfig, DirectoryLock, DownloadService, WorkDirMode,
};

/// Démarre un daemon de test en mémoire avec le mode workdir par défaut de
/// [`DaemonConfig::new`]. Retourne `(adresse, token, workdir temporaire)`.
pub async fn spawn_test_daemon(token: &str) -> (SocketAddr, String, TempDir) {
    spawn_test_daemon_inner(token, None, None).await
}

/// Variante de [`spawn_test_daemon`] imposant explicitement le mode workdir.
pub async fn spawn_test_daemon_with_mode(
    token: &str,
    mode: WorkDirMode,
) -> (SocketAddr, String, TempDir) {
    spawn_test_daemon_inner(token, Some(mode), None).await
}

/// Variante dont le répertoire temporaire des parts est un `TempDir` distinct
/// (utile pour observer les écritures de fichiers `.part`).
/// Retourne `(adresse, token, workdir, répertoire temporaire)`.
pub async fn spawn_test_daemon_with_temp_dir(
    token: &str,
) -> (SocketAddr, String, TempDir, TempDir) {
    let temp_dir = tempdir().unwrap();
    let (addr, token, dir) = spawn_test_daemon_inner(token, None, Some(temp_dir.path())).await;
    (addr, token, dir, temp_dir)
}

async fn spawn_test_daemon_inner(
    token: &str,
    mode: Option<WorkDirMode>,
    temp_dir: Option<&std::path::Path>,
) -> (SocketAddr, String, TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = temp_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| dir.path().join("temp"));
    let token = token.to_string();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token).with_bind_addr(addr);
    let config = match mode {
        Some(m) => config.with_workdir_mode(m),
        None => config,
    };
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);

    let app = create_router(state);

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (addr, token, dir)
}

/// Daemon de test persistant : base SQLite sur disque + verrou exclusif du
/// répertoire de travail (reproduit le cycle de vie réel du daemon).
pub struct RunningDaemon {
    pub addr: SocketAddr,
    pub state: AppState,
    _token: String,
    _lock: DirectoryLock,
    server_handle: JoinHandle<()>,
}

impl RunningDaemon {
    /// Acquiert le verrou du workdir puis démarre le serveur RPC sur une
    /// adresse loopback éphemère.
    pub async fn start(workdir: PathBuf, token: &str) -> Self {
        let lock = DirectoryLock::acquire(&workdir).expect("acquires exclusive directory lock");
        let db_path = workdir.join("mule.db");
        let temp_dir = workdir.join("part");

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let token = token.to_string();
        let config = DaemonConfig::new(workdir, &token).with_bind_addr(addr);
        let downloads =
            DownloadService::new(&db_path, temp_dir).expect("opens persistent download service");
        let state = AppState::new(config, downloads);
        let app = create_router(state.clone());

        let server_handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        Self {
            addr,
            state,
            _token: token,
            _lock: lock,
            server_handle,
        }
    }

    /// Arrête les tâches du daemon puis la tâche du serveur HTTP.
    pub async fn stop(self) {
        self.state.abort_all_tasks().await;
        self.server_handle.abort();
    }
}
