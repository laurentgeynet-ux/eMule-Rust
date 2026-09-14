use std::net::SocketAddr;
use std::path::PathBuf;

use crate::workdir::WorkDirMode;

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub bind_addr: SocketAddr,
    pub data_dir: PathBuf,
    pub default_download_dir: PathBuf,
    pub auth_token: String,
    pub workdir_mode: WorkDirMode,
}

impl DaemonConfig {
    pub fn new(data_dir: PathBuf, auth_token: impl Into<String>) -> Self {
        let default_download_dir = data_dir.join("downloads");
        Self {
            bind_addr: "127.0.0.1:4660".parse().unwrap(),
            data_dir,
            default_download_dir,
            auth_token: auth_token.into(),
            workdir_mode: WorkDirMode::InstalledDefault,
        }
    }

    pub fn with_bind_addr(mut self, addr: SocketAddr) -> Self {
        self.bind_addr = addr;
        self
    }

    pub fn with_workdir_mode(mut self, mode: WorkDirMode) -> Self {
        self.workdir_mode = mode;
        self
    }

    pub fn with_default_download_dir(mut self, dir: PathBuf) -> Self {
        self.default_download_dir = dir;
        self
    }

    pub fn default_data_dir() -> PathBuf {
        crate::workdir::WorkDirectoryResolver::resolve(None, None).workdir
    }
}
