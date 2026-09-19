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
    pub network_profile: mule_network_policy::NetworkProfile,
    pub upnp_enabled: bool,
    pub ed2k_port: u16,
    pub kad_port: u16,
    pub obfuscation_enabled: bool,
    pub obfuscation_required: bool,
    pub obfuscation_padding_max: u8,
    pub max_global_download_peers: usize,
    pub max_peers_per_download: usize,
    pub min_peers_per_download: usize,
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
            network_profile: mule_network_policy::NetworkProfile::PublicEd2kKadPreview,
            upnp_enabled: true,
            ed2k_port: 39735,
            kad_port: 39737,
            obfuscation_enabled: true,
            obfuscation_required: false,
            obfuscation_padding_max: 16,
            max_global_download_peers: 500,
            max_peers_per_download: 400,
            min_peers_per_download: 5,
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

    pub fn with_network_profile(mut self, profile: mule_network_policy::NetworkProfile) -> Self {
        self.network_profile = profile;
        self
    }

    pub fn with_upnp_enabled(mut self, enabled: bool) -> Self {
        self.upnp_enabled = enabled;
        self
    }

    pub fn with_ports(mut self, ed2k_port: u16, kad_port: u16) -> Self {
        self.ed2k_port = ed2k_port;
        self.kad_port = kad_port;
        self
    }

    pub fn with_obfuscation(mut self, enabled: bool, required: bool) -> Self {
        self.obfuscation_enabled = enabled;
        self.obfuscation_required = required;
        self
    }

    pub fn with_swarm_limits(
        mut self,
        max_global: usize,
        max_per_dl: usize,
        min_per_dl: usize,
    ) -> Self {
        self.max_global_download_peers = max_global;
        self.max_peers_per_download = max_per_dl;
        self.min_peers_per_download = min_per_dl;
        self
    }

    pub fn default_data_dir() -> PathBuf {
        crate::workdir::WorkDirectoryResolver::resolve(None, None).workdir
    }
}
