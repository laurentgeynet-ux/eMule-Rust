// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use mule_format::Ed2kFileLink;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_make_link_nominal_and_spaces() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("my test file.bin");
    {
        let mut f = File::create(&file_path).unwrap();
        f.write_all(b"Hello eD2K World!").unwrap();
    }

    let cli_bin = env!("CARGO_BIN_EXE_mule-cli");
    let output = Command::new(cli_bin)
        .args(["make-link", file_path.to_str().unwrap()])
        .output()
        .expect("Failed to run mule-cli");

    assert!(output.status.success());
    let link_str = String::from_utf8(output.stdout).unwrap();
    let trimmed = link_str.trim();

    assert!(trimmed.starts_with("ed2k://|file|my test file.bin|17|"));
    assert!(trimmed.ends_with("|/"));

    // Verify it parses back cleanly
    let parsed = Ed2kFileLink::parse(trimmed).expect("Should parse back cleanly");
    assert_eq!(parsed.file_name, "my test file.bin");
    assert_eq!(parsed.size, 17);
}

#[test]
fn test_make_link_accents_and_unicode() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("fichier_été_français.dat");
    {
        let mut f = File::create(&file_path).unwrap();
        f.write_all(&[0x42; 100]).unwrap();
    }

    let cli_bin = env!("CARGO_BIN_EXE_mule-cli");
    let output = Command::new(cli_bin)
        .args(["make-link", file_path.to_str().unwrap()])
        .output()
        .expect("Failed to run mule-cli");

    assert!(output.status.success());
    let link_str = String::from_utf8(output.stdout).unwrap();
    let trimmed = link_str.trim();

    let parsed = Ed2kFileLink::parse(trimmed).expect("Should parse accented filename");
    assert_eq!(parsed.file_name, "fichier_été_français.dat");
    assert_eq!(parsed.size, 100);
}

#[test]
fn test_make_link_emoji_in_filename() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("archive_🚀_test.bin");
    {
        let mut f = File::create(&file_path).unwrap();
        f.write_all(b"emoji payload").unwrap();
    }

    let cli_bin = env!("CARGO_BIN_EXE_mule-cli");
    let output = Command::new(cli_bin)
        .args(["make-link", file_path.to_str().unwrap()])
        .output()
        .expect("Failed to run mule-cli");

    assert!(output.status.success());
    let link_str = String::from_utf8(output.stdout).unwrap();
    let trimmed = link_str.trim();

    let parsed = Ed2kFileLink::parse(trimmed).expect("Should parse emoji filename");
    assert_eq!(parsed.file_name, "archive_🚀_test.bin");
    assert_eq!(parsed.size, 13);
}

#[test]
fn test_make_link_delimiters_escaped_safely() {
    let temp = tempdir().unwrap();
    // On Windows, '|' is invalid in actual Windows filenames, but we can test
    // Ed2kFileLink::create and sanitize_filename directly, and test valid Windows chars
    let file_path = temp.path().join("file%20name.bin");
    {
        let mut f = File::create(&file_path).unwrap();
        f.write_all(b"percent content").unwrap();
    }

    let cli_bin = env!("CARGO_BIN_EXE_mule-cli");
    let output = Command::new(cli_bin)
        .args(["make-link", file_path.to_str().unwrap()])
        .output()
        .expect("Failed to run mule-cli");

    assert!(output.status.success());
    let link_str = String::from_utf8(output.stdout).unwrap();
    let parsed = Ed2kFileLink::parse(link_str.trim()).unwrap();
    assert_eq!(parsed.size, 15);

    // Direct check of sanitize_filename escaping '|'
    let sanitized = Ed2kFileLink::sanitize_filename("dangerous|pipe|name.bin");
    assert_eq!(sanitized, "dangerous%7Cpipe%7Cname.bin");

    let direct_link =
        Ed2kFileLink::create("dangerous|pipe|name.bin", 50, parsed.ed2k_hash).unwrap();
    let rendered = direct_link.to_link_string();
    assert!(rendered.contains("dangerous%7Cpipe%7Cname.bin"));
    let parsed_direct = Ed2kFileLink::parse(&rendered).unwrap();
    assert_eq!(parsed_direct.file_name, "dangerous|pipe|name.bin");
}

#[test]
fn test_make_link_empty_file_rejected() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("empty_file.bin");
    File::create(&file_path).unwrap(); // 0 bytes

    let cli_bin = env!("CARGO_BIN_EXE_mule-cli");
    let output = Command::new(cli_bin)
        .args(["make-link", file_path.to_str().unwrap()])
        .output()
        .expect("Failed to run mule-cli");

    assert!(!output.status.success(), "0-byte file must be rejected");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("empty") || stderr.contains("0 bytes"));
}

#[test]
fn test_make_link_exact_part_size_boundary() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("exact_part_9728000.bin");
    {
        let mut f = File::create(&file_path).unwrap();
        // 9_728_000 bytes with pattern
        let chunk = vec![0x33u8; 9728];
        for _ in 0..1000 {
            f.write_all(&chunk).unwrap();
        }
    }

    let cli_bin = env!("CARGO_BIN_EXE_mule-cli");
    let output = Command::new(cli_bin)
        .args(["make-link", file_path.to_str().unwrap()])
        .output()
        .expect("Failed to run mule-cli");

    assert!(output.status.success());
    let link_str = String::from_utf8(output.stdout).unwrap();
    let parsed = Ed2kFileLink::parse(link_str.trim()).unwrap();
    assert_eq!(parsed.size, 9_728_000);
}

#[test]
fn test_make_link_fixture_pattern_9728001() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("pattern_9728001.bin");
    {
        let mut f = File::create(&file_path).unwrap();
        // Construct fixture: modulo 256 pattern up to 9_728_001 bytes (matching REPEATING_0_TO_255)
        let mut buf = Vec::with_capacity(65536);
        let mut written: u64 = 0;
        while written < 9_728_001 {
            buf.clear();
            let to_write = std::cmp::min(65536, 9_728_001 - written);
            for i in 0..to_write {
                buf.push(((written + i) % 256) as u8);
            }
            f.write_all(&buf).unwrap();
            written += to_write;
        }
    }

    let cli_bin = env!("CARGO_BIN_EXE_mule-cli");
    let output = Command::new(cli_bin)
        .args(["make-link", file_path.to_str().unwrap(), "--detailed"])
        .output()
        .expect("Failed to run mule-cli");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("File       : pattern_9728001.bin"));
    assert!(stdout.contains("Size       : 9728001 bytes"));
    assert!(stdout.contains("eD2K hash  : 695b81b1727666818988524bd7fc823c"));
    assert!(stdout.contains(
        "eD2K link  : ed2k://|file|pattern_9728001.bin|9728001|695b81b1727666818988524bd7fc823c|"
    ));
}
