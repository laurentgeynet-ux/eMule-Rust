//! Isolated, neutralized ephemeral environment for Tier 3 external reference execution.
//!
//! Enforces:
//! - Ephemeral temporary directory isolation (never touches user AppData or project paths).
//! - Hard neutralization of external networking (Kad disabled, server lists empty & disabled, auto-connect disabled).
//! - Explicit port binding on local loopback.

#![allow(dead_code)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Represents an isolated execution environment for a reference eMule/aMule binary.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Tier3Environment {
    work_dir: TempDir,
    config_dir: PathBuf,
    incoming_dir: PathBuf,
    temp_dir: PathBuf,
    tcp_port: u16,
    udp_port: u16,
}

impl Tier3Environment {
    /// Creates a new isolated environment with neutralized configuration.
    pub fn new(tcp_port: u16, udp_port: u16) -> io::Result<Self> {
        let work_dir = tempfile::tempdir()?;
        let work_path = work_dir.path();

        let config_dir = work_path.join("config");
        let incoming_dir = work_path.join("incoming");
        let temp_dir = work_path.join("temp");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&incoming_dir)?;
        fs::create_dir_all(&temp_dir)?;

        let env = Self {
            work_dir,
            config_dir,
            incoming_dir,
            temp_dir,
            tcp_port,
            udp_port,
        };

        env.write_emule_preferences_ini()?;
        env.write_empty_server_met()?;

        Ok(env)
    }

    /// Path to the isolated root work directory.
    pub fn work_path(&self) -> &Path {
        self.work_dir.path()
    }

    /// Path to the configuration directory.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Path to the incoming downloads directory.
    pub fn incoming_dir(&self) -> &Path {
        &self.incoming_dir
    }

    /// Path to the temporary part files directory.
    pub fn temp_dir(&self) -> &Path {
        &self.temp_dir
    }

    /// The TCP port configured for the reference peer.
    pub fn tcp_port(&self) -> u16 {
        self.tcp_port
    }

    /// The UDP port configured for the reference peer.
    pub fn udp_port(&self) -> u16 {
        self.udp_port
    }

    /// Writes a neutralized eMule `preferences.ini` file.
    fn write_emule_preferences_ini(&self) -> io::Result<()> {
        let ini_path = self.config_dir.join("preferences.ini");
        let ini_content = format!(
            "[eMule]\r\n\
            Nick=RefPeer050a\r\n\
            Port={tcp_port}\r\n\
            UDPPort={udp_port}\r\n\
            EnableKad=0\r\n\
            NetworkED2K=1\r\n\
            NetworkKademlia=0\r\n\
            AutoConnect=0\r\n\
            AutoUpdateServerList=0\r\n\
            ServerUDPPort=0\r\n\
            UpdateNotify=0\r\n\
            CheckDiskspace=0\r\n\
            MinFreeDiskSpace=0\r\n\
            SafeServerConnect=1\r\n\
            AutoConnectStaticOnly=1\r\n\
            Verbose=1\r\n\
            IncomingDir={incoming}\r\n\
            TempDir={temp}\r\n",
            tcp_port = self.tcp_port,
            udp_port = self.udp_port,
            incoming = self.incoming_dir.display(),
            temp = self.temp_dir.display()
        );
        fs::write(ini_path, ini_content)
    }

    /// Writes an empty `server.met` file to prevent server connection attempts.
    fn write_empty_server_met(&self) -> io::Result<()> {
        let server_met_path = self.config_dir.join("server.met");
        // eMule server.met header: version byte 0xE0 + 4 bytes server count (0)
        let empty_met: [u8; 5] = [0xE0, 0x00, 0x00, 0x00, 0x00];
        fs::write(server_met_path, empty_met)
    }
}
