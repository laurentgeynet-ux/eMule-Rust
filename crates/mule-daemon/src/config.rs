use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub bind_addr: SocketAddr,
    pub data_dir: PathBuf,
    pub auth_token: String,
}

impl DaemonConfig {
    pub fn new(data_dir: PathBuf, auth_token: impl Into<String>) -> Self {
        Self {
            bind_addr: "127.0.0.1:4660".parse().unwrap(),
            data_dir,
            auth_token: auth_token.into(),
        }
    }

    pub fn with_bind_addr(mut self, addr: SocketAddr) -> Self {
        self.bind_addr = addr;
        self
    }
}
