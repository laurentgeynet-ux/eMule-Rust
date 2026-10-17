// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

#![allow(clippy::too_many_arguments)]

pub mod config;
#[cfg(feature = "ed2k-loopback")]
pub mod ed2k;
pub mod error;
#[cfg(feature = "lab-loopback")]
pub mod lab;
pub mod lock;
pub mod net_fetch;
pub mod rpc;
pub mod services;
pub mod session;
pub mod state;
pub mod workdir;

pub use config::DaemonConfig;
#[cfg(feature = "ed2k-loopback")]
pub use ed2k::run_ed2k_download;
pub use error::{DaemonError, JsonRpcError};
#[cfg(feature = "lab-loopback")]
pub use lab::*;
pub use lock::DirectoryLock;
pub use rpc::{
    create_router, DownloadSnapshot, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
};
pub use services::{
    DaemonEvent, DownloadDetail, DownloadService, ImportPartMetResult, PartMetInspection,
    PartProgressSummary, SourceAvailabilitySummary,
};
pub use session::DaemonSession;
pub use state::{AppState, LogControl};
pub use workdir::{
    ResolvedDirectories, ResolvedWorkDir, WorkDirMode, WorkDirectoryResolver, ENV_DOWNLOADS_DIR,
    ENV_LEGACY_WORKDIR, ENV_SION_DOWNLOADS_DIR, ENV_SION_WORKDIR, ENV_WORKDIR,
    PORTABLE_FLAG_FILENAME,
};
