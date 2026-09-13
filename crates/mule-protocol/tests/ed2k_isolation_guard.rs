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
fn test_ed2k_module_has_zero_network_imports_or_sockets() {
    let dir = ed2k_src_dir();
    assert!(dir.exists(), "src/ed2k must exist");

    let mut rs_files = Vec::new();
    collect_rs_files(&dir, &mut rs_files);
    assert!(
        !rs_files.is_empty(),
        "src/ed2k must contain rust source files"
    );

    // Forbidden identifiers and tokens in mule-protocol::ed2k
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
                    "Forbidden network token '{}' found in {}: {}",
                    forbidden,
                    file.display(),
                    line
                );
            }
        }
    }
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

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
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
}
