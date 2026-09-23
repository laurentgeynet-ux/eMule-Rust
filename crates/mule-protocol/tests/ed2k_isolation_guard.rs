// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Architectural verification tests enforcing mechanical isolation of eD2K from network,
//! preventing accidental leaking of sockets, web transports, or lab dependencies.

use std::fs;
use std::path::{Path, PathBuf};

fn ed2k_src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("ed2k")
}

fn collect_rs_files(dir: &Path, acc: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, acc);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                acc.push(path);
            }
        }
    }
}

#[test]
fn test_ed2k_module_has_zero_network_imports_or_sockets_in_core_protocol() {
    let dir = ed2k_src_dir();
    assert!(dir.exists(), "src/ed2k must exist");

    let mut rs_files = Vec::new();
    collect_rs_files(&dir, &mut rs_files);
    assert!(
        !rs_files.is_empty(),
        "src/ed2k must contain rust source files"
    );

    // Forbidden network identifiers in all core ed2k protocol and session files
    let forbidden_tokens = [
        "std::net",
        "tokio::net",
        "TcpStream",
        "TcpListener",
        "UdpSocket",
        "SocketAddr",
        "ToSocketAddrs",
        "tungstenite",
        "axum",
        "hyper",
        "reqwest",
        "websocket",
    ];

    for file in &rs_files {
        // loopback.rs and reference.rs are the dedicated, feature-gated loopback drivers audited separately
        if file
            .file_name()
            .is_some_and(|n| n == "loopback.rs" || n == "reference.rs")
        {
            continue;
        }

        let content = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", file.display()));

        for line in content.lines() {
            let trimmed = line.trim();
            // Skip pure documentation comments
            if trimmed.starts_with("//!") || trimmed.starts_with("///") || trimmed.starts_with("//")
            {
                continue;
            }

            for forbidden in forbidden_tokens {
                assert!(
                    !line.contains(forbidden),
                    "Forbidden network token '{}' found in core protocol file {}: {}",
                    forbidden,
                    file.display(),
                    line
                );
            }
        }
    }
}

#[test]
fn test_ed2k_loopback_transport_strictly_confined_and_feature_gated() {
    let loopback_file = ed2k_src_dir().join("loopback.rs");
    assert!(loopback_file.exists(), "src/ed2k/loopback.rs must exist");

    let content = fs::read_to_string(&loopback_file).expect("failed to read loopback.rs");

    // Must strictly bind to loopback 127.0.0.1
    assert!(
        content.contains("\"127.0.0.1:0\""),
        "loopback.rs must only bind to 127.0.0.1:0"
    );
    assert!(
        content.contains(".is_loopback()"),
        "loopback.rs must enforce is_loopback() before connecting"
    );
    // Must NOT bind to any public / all-interfaces wildcard
    assert!(
        !content.contains("\"0.0.0.0"),
        "loopback.rs must never bind to 0.0.0.0"
    );

    // Verify mod.rs gates loopback behind feature ed2k-loopback while keeping session and peer_info pure
    let mod_file = ed2k_src_dir().join("mod.rs");
    let mod_content = fs::read_to_string(&mod_file)
        .expect("failed to read mod.rs")
        .replace("\r\n", "\n");

    assert!(
        mod_content.contains("#[cfg(feature = \"ed2k-loopback\")]\npub mod loopback;"),
        "src/ed2k/mod.rs must gate loopback behind #[cfg(feature = \"ed2k-loopback\")]"
    );
    assert!(
        mod_content.contains("pub mod session;"),
        "src/ed2k/mod.rs must keep pure session state machine accessible unconditionally"
    );
    assert!(
        mod_content.contains("pub mod peer_info;"),
        "src/ed2k/mod.rs must keep pure peer_info metadata accessible unconditionally"
    );
}

#[test]
fn test_ed2k_reference_transport_strictly_confined_and_feature_gated() {
    let ref_file = ed2k_src_dir().join("reference.rs");
    assert!(ref_file.exists(), "src/ed2k/reference.rs must exist");

    let content = fs::read_to_string(&ref_file).expect("failed to read reference.rs");

    assert!(
        content.contains(".is_loopback()"),
        "reference.rs must enforce is_loopback() before connecting"
    );
    assert!(
        !content.contains("\"0.0.0.0"),
        "reference.rs must never bind to 0.0.0.0"
    );

    let mod_file = ed2k_src_dir().join("mod.rs");
    let mod_content = fs::read_to_string(&mod_file)
        .expect("failed to read mod.rs")
        .replace("\r\n", "\n");

    assert!(
        mod_content.contains("#[cfg(feature = \"ed2k-reference-loopback\")]\npub mod reference;"),
        "src/ed2k/mod.rs must gate reference behind #[cfg(feature = \"ed2k-reference-loopback\")]"
    );
}

#[test]
fn test_ed2k_module_has_zero_lab_dependencies() {
    let dir = ed2k_src_dir();
    let mut rs_files = Vec::new();
    collect_rs_files(&dir, &mut rs_files);

    for file in &rs_files {
        let content = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", file.display()));

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            assert!(
                !line.contains("crate::lab") && !line.contains("super::lab"),
                "Forbidden lab dependency found in {}: {}",
                file.display(),
                line
            );
        }
    }
}

#[test]
fn test_ed2k_module_has_zero_unwrap_or_expect() {
    let dir = ed2k_src_dir();
    let mut rs_files = Vec::new();
    collect_rs_files(&dir, &mut rs_files);

    for file in &rs_files {
        let content = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", file.display()));

        let mut in_test_mod = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains("#[cfg(test)]") || trimmed.contains("mod tests {") {
                in_test_mod = true;
            }
            if in_test_mod {
                continue;
            }
            assert!(
                !line.contains(".unwrap()") && !line.contains(".expect("),
                "Forbidden unwrap/expect panic path found in {}: {}",
                file.display(),
                line
            );
        }
    }
}

#[test]
fn test_daemon_rpc_methods_gates_lab_routes_behind_feature() {
    let daemon_methods_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("mule-daemon")
        .join("src")
        .join("rpc")
        .join("methods.rs");

    assert!(
        daemon_methods_path.exists(),
        "mule-daemon rpc/methods.rs must exist"
    );

    let content =
        fs::read_to_string(&daemon_methods_path).expect("failed to read daemon methods.rs");
    assert!(
        content.contains("#[cfg(feature = \"lab-loopback\")]"),
        "mule-daemon rpc/methods.rs must conditionally guard lab routes"
    );
    assert!(
        content.contains("\"lab.addPeerSource\""),
        "mule-daemon rpc/methods.rs must declare lab.addPeerSource"
    );
    assert!(
        content.contains("#[cfg(feature = \"ed2k-loopback\")]"),
        "mule-daemon rpc/methods.rs must conditionally guard ed2k loopback routes"
    );
    assert!(
        content.contains("\"ed2k.probeSource\""),
        "mule-daemon rpc/methods.rs must declare ed2k.probeSource"
    );
}
