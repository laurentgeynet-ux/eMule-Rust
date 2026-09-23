// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! Process management, cryptographic validation, and loopback confinement for Tier 3 reference execution.

#![allow(dead_code)]

use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::environment::Tier3Environment;

/// Error conditions encountered during Tier 3 orchestration.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum Tier3HarnessError {
    #[error("Reference binary not found at path: {0}")]
    BinaryNotFound(PathBuf),

    #[error("Binary SHA-256 mismatch: expected {expected}, computed {actual}")]
    BinaryHashMismatch { expected: String, actual: String },

    #[error("Strict loopback violation: address {0} is not on 127.0.0.1 loopback")]
    NonLoopbackAddress(SocketAddr),

    #[error("Failed to spawn reference process: {0}")]
    ProcessSpawnFailed(String),

    #[error("Watchdog timeout ({0:?}) exceeded waiting for reference peer")]
    Timeout(Duration),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// RAII Guard ensuring immediate kill & reap of the reference child process upon scope exit or panic,
/// as well as restoration of host registry settings on Windows.
#[derive(Debug)]
pub struct Tier3ProcessGuard {
    child: Option<Child>,
    #[cfg(windows)]
    previous_registry_val: Option<String>,
    #[cfg(windows)]
    registry_existed: bool,
}

impl Tier3ProcessGuard {
    #[cfg(windows)]
    pub fn new(
        child: Child,
        registry_existed: bool,
        previous_registry_val: Option<String>,
    ) -> Self {
        Self {
            child: Some(child),
            registry_existed,
            previous_registry_val,
        }
    }

    #[cfg(not(windows))]
    pub fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    /// Terminates the child process explicitly and restores host settings.
    pub fn terminate(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/IM", "emule.exe", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            if self.registry_existed {
                if let Some(ref prev) = self.previous_registry_val {
                    let _ = Command::new("reg")
                        .args([
                            "add",
                            r"HKCU\Software\eMule",
                            "/v",
                            "UsePublicUserDirectories",
                            "/t",
                            "REG_DWORD",
                            "/d",
                            prev,
                            "/f",
                        ])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
            } else {
                let _ = Command::new("reg")
                    .args([
                        "delete",
                        r"HKCU\Software\eMule",
                        "/v",
                        "UsePublicUserDirectories",
                        "/f",
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        }
    }
}

impl Drop for Tier3ProcessGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// Validates external binary existence and SHA-256 cryptographic identity.
pub struct Tier3BinaryValidator;

impl Tier3BinaryValidator {
    /// Computes the SHA-256 of the binary at `path` and compares it to `expected_sha256` if provided.
    pub fn validate(
        path: &Path,
        expected_sha256: Option<&str>,
    ) -> Result<String, Tier3HarnessError> {
        if !path.exists() || !path.is_file() {
            return Err(Tier3HarnessError::BinaryNotFound(path.to_path_buf()));
        }

        let bytes = fs::read(path)?;
        let actual_hash = compute_sha256_hex(&bytes);

        if let Some(expected) = expected_sha256 {
            let expected_trimmed = expected.trim();
            if !expected_trimmed.is_empty() && !actual_hash.eq_ignore_ascii_case(expected_trimmed) {
                return Err(Tier3HarnessError::BinaryHashMismatch {
                    expected: expected_trimmed.to_string(),
                    actual: actual_hash,
                });
            }
        }

        Ok(actual_hash)
    }
}

/// Primary orchestrator for running an external reference binary against EMule-Rust.
#[allow(dead_code)]
pub struct Tier3Harness {
    environment: Tier3Environment,
    binary_path: PathBuf,
    binary_sha256: String,
    loopback_addr: SocketAddr,
    process_guard: Option<Tier3ProcessGuard>,
}

impl Tier3Harness {
    /// Prepares a new Tier 3 harness, verifying the binary and creating the isolated environment.
    ///
    /// # Panics / Errors
    /// Returns `Tier3HarnessError::NonLoopbackAddress` if the target address is not on loopback.
    pub fn new(
        binary_path: PathBuf,
        expected_sha256: Option<&str>,
        tcp_port: u16,
        udp_port: u16,
    ) -> Result<Self, Tier3HarnessError> {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), tcp_port);

        // Mechanical loopback pre-check
        if !addr.ip().is_loopback() {
            return Err(Tier3HarnessError::NonLoopbackAddress(addr));
        }

        let binary_sha256 = Tier3BinaryValidator::validate(&binary_path, expected_sha256)?;
        let environment = Tier3Environment::new(tcp_port, udp_port)?;

        Ok(Self {
            environment,
            binary_path,
            binary_sha256,
            loopback_addr: addr,
            process_guard: None,
        })
    }

    /// Mechanical assertion that the harness target is strictly on local loopback.
    pub fn assert_loopback_confinement(&self) {
        assert!(
            self.loopback_addr.ip().is_loopback(),
            "FATAL: Tier 3 harness target {:?} is not loopback!",
            self.loopback_addr
        );
    }

    /// Spawns the reference binary within the isolated ephemeral environment.
    pub fn spawn_reference_process(
        &mut self,
        extra_args: &[&str],
    ) -> Result<(), Tier3HarnessError> {
        self.assert_loopback_confinement();

        // Stage reference binary into isolated ephemeral environment so Win32 GetAppDir()
        // resolves directly to the sandboxed workdir, enforcing local preferences.ini loading.
        let exe_name = self
            .binary_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("emule.exe"));
        let staged_bin = self.environment.work_path().join(exe_name);
        if !staged_bin.exists() {
            fs::copy(&self.binary_path, &staged_bin)?;
        }

        // Also mirror adjacent lang/ resources if present to avoid localization dialogs
        if let Some(parent) = self.binary_path.parent() {
            let src_lang = parent.join("lang");
            let dst_lang = self.environment.work_path().join("lang");
            if src_lang.is_dir() && !dst_lang.exists() {
                let _ = fs::create_dir_all(&dst_lang);
                if let Ok(entries) = fs::read_dir(&src_lang) {
                    for entry in entries.flatten() {
                        let _ = fs::copy(entry.path(), dst_lang.join(entry.file_name()));
                    }
                }
            }
        }

        #[cfg(windows)]
        let (registry_existed, previous_registry_val) = {
            // Terminate any orphan emule instance so single-instance lock does not exit early
            let _ = Command::new("taskkill")
                .args(["/IM", "emule.exe", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            // Query existing value if any
            let query_out = Command::new("reg")
                .args([
                    "query",
                    r"HKCU\Software\eMule",
                    "/v",
                    "UsePublicUserDirectories",
                ])
                .output();
            let (existed, val) = match query_out {
                Ok(out) if out.status.success() => {
                    let text = String::from_utf8_lossy(&out.stdout);
                    let parsed = text.split_whitespace().last().map(|s| s.to_string());
                    (true, parsed)
                }
                _ => (false, None),
            };

            // Enforce ExecutableDir mode (UsePublicUserDirectories = 2) so eMule strictly
            // operates in the ephemeral sandbox and binds to the test-allocated loopback port
            let _ = Command::new("reg")
                .args([
                    "add",
                    r"HKCU\Software\eMule",
                    "/v",
                    "UsePublicUserDirectories",
                    "/t",
                    "REG_DWORD",
                    "/d",
                    "2",
                    "/f",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            (existed, val)
        };

        let mut cmd = Command::new(&staged_bin);
        cmd.current_dir(self.environment.work_path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(extra_args);

        // Under Windows or Wine, ensure environment points to isolated temp config
        cmd.env("EMULE_CONFIG_DIR", self.environment.config_dir());

        let child = cmd
            .spawn()
            .map_err(|e| Tier3HarnessError::ProcessSpawnFailed(e.to_string()))?;

        #[cfg(windows)]
        {
            self.process_guard = Some(Tier3ProcessGuard::new(
                child,
                registry_existed,
                previous_registry_val,
            ));
        }
        #[cfg(not(windows))]
        {
            self.process_guard = Some(Tier3ProcessGuard::new(child));
        }
        Ok(())
    }

    /// The verified SHA-256 hash of the reference executable.
    pub fn binary_sha256(&self) -> &str {
        &self.binary_sha256
    }

    /// Truncated redacted hash for safe public logs and divergence reports.
    pub fn binary_sha256_redacted(&self) -> String {
        if self.binary_sha256.len() >= 12 {
            format!("{}...", &self.binary_sha256[..12])
        } else {
            self.binary_sha256.clone()
        }
    }

    /// The isolated ephemeral environment.
    pub fn environment(&self) -> &Tier3Environment {
        &self.environment
    }

    /// The target loopback socket address.
    pub fn loopback_addr(&self) -> SocketAddr {
        self.loopback_addr
    }

    /// Checks whether the child process is still running or has exited.
    pub fn is_child_running(&mut self) -> bool {
        if let Some(ref mut guard) = self.process_guard {
            if let Some(ref mut child) = guard.child {
                match child.try_wait() {
                    Ok(None) => return true,
                    Ok(Some(status)) => {
                        eprintln!("Reference process exited early with status: {:?}", status);
                        return false;
                    }
                    Err(e) => {
                        eprintln!("Error checking reference process: {:?}", e);
                        return false;
                    }
                }
            }
        }
        false
    }

    /// Returns the process ID of the child process if running.
    pub fn child_pid(&self) -> Option<u32> {
        self.process_guard
            .as_ref()
            .and_then(|g| g.child.as_ref().map(|c| c.id()))
    }

    /// Explicitly shuts down the reference child process.
    pub fn shutdown(&mut self) {
        if let Some(mut guard) = self.process_guard.take() {
            guard.terminate();
        }
    }
}

/// Helper function to compute SHA-256 hex string using standard Rust types.
fn compute_sha256_hex(data: &[u8]) -> String {
    // Uses standard SHA-256 implementation matching common::sha256 or crate hash
    use std::fmt::Write;
    // We can use a deterministic byte reduction or SHA-256 if available.
    // In mule-protocol tests, we can compute sha256 or delegate to sha2 crate if available.
    // Let's implement SHA-256 or use standard FIPS 180-4 implementation from common.
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in result {
        let _ = write!(hex, "{:02x}", b);
    }
    hex
}

/// Minimal self-contained FIPS 180-4 SHA-256 hasher for zero-dependency test verification.
struct Sha256 {
    state: [u32; 8],
    count: u64,
    buffer: [u8; 64],
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            count: 0,
            buffer: [0u8; 64],
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        while !data.is_empty() {
            let buf_idx = (self.count % 64) as usize;
            let take = (64 - buf_idx).min(data.len());
            self.buffer[buf_idx..buf_idx + take].copy_from_slice(&data[..take]);
            self.count += take as u64;
            data = &data[take..];
            if self.count.is_multiple_of(64) {
                self.process_block();
            }
        }
    }

    fn process_block(&mut self) {
        let mut w = [0u32; 64];
        for (i, item) in w.iter_mut().enumerate().take(16) {
            *item = u32::from_be_bytes([
                self.buffer[i * 4],
                self.buffer[i * 4 + 1],
                self.buffer[i * 4 + 2],
                self.buffer[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    fn finalize(mut self) -> [u8; 32] {
        let total_bits = self.count * 8;
        let buf_idx = (self.count % 64) as usize;
        self.buffer[buf_idx] = 0x80;
        for i in buf_idx + 1..64 {
            self.buffer[i] = 0;
        }
        if buf_idx >= 56 {
            self.process_block();
            self.buffer = [0u8; 64];
        }
        self.buffer[56..64].copy_from_slice(&total_bits.to_be_bytes());
        self.process_block();

        let mut out = [0u8; 32];
        for i in 0..8 {
            out[i * 4..i * 4 + 4].copy_from_slice(&self.state[i].to_be_bytes());
        }
        out
    }
}
