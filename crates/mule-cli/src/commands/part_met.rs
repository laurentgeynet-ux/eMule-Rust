// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use mule_format::PartMetFile;
use std::path::Path;

pub fn inspect_part_met(file: &Path) {
    println!("== Inspecting .part.met file: {} ==", file.display());
    match PartMetFile::open(file) {
        Ok(met) => {
            println!("Version     : 0x{:02x}", met.version);
            println!("Mod Date    : {}", met.mod_date);
            println!("File Hash   : {}", met.file_hash);
            println!(
                "File Name   : {}",
                met.file_name.as_deref().unwrap_or("<unknown>")
            );
            println!(
                "File Size   : {} bytes",
                met.file_size
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "<unknown>".into())
            );
            println!("Transferred : {} bytes", met.transferred);
            if let Some(aich) = met.aich_hash {
                println!("AICH Hash   : {}", aich.to_base32());
            }
            println!("Parts Count : {}", met.part_hashes.len());
            println!("Gaps Count  : {}", met.gaps.len());
            for (i, (start, end)) in met.gaps.iter().enumerate() {
                println!("  Gap #{:<3}  : {} -> {}", i, start, end);
            }
            println!("Tags Count  : {}", met.tags.len());
            for tag in &met.tags {
                println!("  - {}", tag);
            }
        }
        Err(err) => {
            eprintln!("Error reading part.met file: {err}");
            std::process::exit(1);
        }
    }
}
