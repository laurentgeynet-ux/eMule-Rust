// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use mule_daemon::{DaemonSession, PORTABLE_FLAG_FILENAME};
use std::fs::File;
use std::net::TcpListener;
use std::process::Command;
use tempfile::tempdir;

#[tokio::test]
async fn test_dual_portable_bundles_independent_execution() {
    // 1. Ensure default port 4660 is occupied so both daemons must use ephemeral ports.
    // If a live dev daemon is already running on 4660, that also satisfies the requirement.
    let _occupier = TcpListener::bind("127.0.0.1:4660").ok();

    let daemon_bin = env!("CARGO_BIN_EXE_mule-daemon");

    // 2. Setup Bundle A with portable.flag and packaged layout (bin/mule-daemon.exe)
    let temp_a = tempdir().unwrap();
    let bundle_a = temp_a.path().join("BundleA");
    let bin_a = bundle_a.join("bin");
    std::fs::create_dir_all(&bin_a).unwrap();
    File::create(bundle_a.join(PORTABLE_FLAG_FILENAME)).unwrap();
    let daemon_a_exe = bin_a.join(if cfg!(windows) {
        "mule-daemon.exe"
    } else {
        "mule-daemon"
    });
    std::fs::copy(daemon_bin, &daemon_a_exe).unwrap();

    // 3. Setup Bundle B with portable.flag and packaged layout (bin/mule-daemon.exe)
    let temp_b = tempdir().unwrap();
    let bundle_b = temp_b.path().join("BundleB");
    let bin_b = bundle_b.join("bin");
    std::fs::create_dir_all(&bin_b).unwrap();
    File::create(bundle_b.join(PORTABLE_FLAG_FILENAME)).unwrap();
    let daemon_b_exe = bin_b.join(if cfg!(windows) {
        "mule-daemon.exe"
    } else {
        "mule-daemon"
    });
    std::fs::copy(daemon_bin, &daemon_b_exe).unwrap();

    // 4. Launch Daemon A from inside Bundle A (without --workdir, relying on portable.flag)
    let current_pid = std::process::id().to_string();
    let mut proc_a = Command::new(&daemon_a_exe)
        .current_dir(&bundle_a)
        .args(["--parent-pid", &current_pid])
        .spawn()
        .expect("Failed to spawn daemon A");

    // 5. Launch Daemon B from inside Bundle B
    let mut proc_b = Command::new(&daemon_b_exe)
        .current_dir(&bundle_b)
        .args(["--parent-pid", &current_pid])
        .spawn()
        .expect("Failed to spawn daemon B");

    let data_a = bundle_a.join("data");
    let data_b = bundle_b.join("data");

    // 6. Wait for session.json in both bundles (up to 6s)
    let start = std::time::Instant::now();
    let mut session_a = None;
    let mut session_b = None;

    while start.elapsed() < std::time::Duration::from_secs(6) {
        if session_a.is_none() && data_a.join("session.json").exists() {
            session_a = DaemonSession::read(&data_a).ok();
        }
        if session_b.is_none() && data_b.join("session.json").exists() {
            session_b = DaemonSession::read(&data_b).ok();
        }
        if session_a.is_some() && session_b.is_some() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Teardown processes early if timeout
    let s_a = session_a.expect("Daemon A failed to create session.json in BundleA/data");
    let s_b = session_b.expect("Daemon B failed to create session.json in BundleB/data");

    // 7. Verifications
    // A. Both bundles have their own lock, sqlite db, and tokens
    assert!(
        data_a.join("mule.lock").exists(),
        "Bundle A lock must exist"
    );
    assert!(
        data_b.join("mule.lock").exists(),
        "Bundle B lock must exist"
    );
    assert!(data_a.join("mule.db").exists(), "Bundle A db must exist");
    assert!(data_b.join("mule.db").exists(), "Bundle B db must exist");

    let token_a = DaemonSession::read_token(&data_a).expect("Bundle A auth.token must exist");
    let token_b = DaemonSession::read_token(&data_b).expect("Bundle B auth.token must exist");
    assert_ne!(
        token_a, token_b,
        "Tokens between bundles must be independent"
    );

    // B. Instance IDs must be distinct
    assert_ne!(s_a.instance_id, s_b.instance_id, "Instance IDs must differ");

    // C. Endpoints must differ (distinct ephemeral ports)
    assert_ne!(
        s_a.endpoint, s_b.endpoint,
        "Ephemeral endpoints must differ"
    );
    assert!(
        !s_a.endpoint.contains(":4660/"),
        "Endpoint A must not collide on 4660"
    );
    assert!(
        !s_b.endpoint.contains(":4660/"),
        "Endpoint B must not collide on 4660"
    );

    // 8. Clean up
    let _ = proc_a.kill();
    let _ = proc_b.kill();
    let _ = proc_a.wait();
    let _ = proc_b.wait();
    drop(_occupier);
}
