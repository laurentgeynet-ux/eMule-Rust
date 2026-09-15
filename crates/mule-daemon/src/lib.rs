#![allow(clippy::too_many_arguments)]

pub mod config;
pub mod error;
#[cfg(feature = "lab-loopback")]
pub mod lab;
#[cfg(feature = "ed2k-loopback")]
pub mod ed2k;
pub mod lock;
pub mod rpc;
pub mod services;
pub mod session;
pub mod state;
pub mod workdir;

pub use config::DaemonConfig;
pub use error::{DaemonError, JsonRpcError};
#[cfg(feature = "lab-loopback")]
pub use lab::*;
#[cfg(feature = "ed2k-loopback")]
pub use ed2k::run_ed2k_download;
pub use lock::DirectoryLock;
pub use rpc::{
    create_router, DownloadSnapshot, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
};
pub use services::{DownloadService, SourceAvailabilitySummary};
pub use session::DaemonSession;
pub use state::AppState;
pub use workdir::{
    ResolvedDirectories, ResolvedWorkDir, WorkDirMode, WorkDirectoryResolver, ENV_DOWNLOADS_DIR,
    ENV_LEGACY_WORKDIR, ENV_WORKDIR, PORTABLE_FLAG_FILENAME,
};
