// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use mule_format::Ed2kFileLink;

pub fn inspect_link(link: &str) {
    println!("== Inspecting eD2K Link ==");
    match Ed2kFileLink::parse(link) {
        Ok(parsed) => {
            println!("File Name   : {}", parsed.file_name);
            println!("File Size   : {} bytes", parsed.size);
            println!("eD2K Hash   : {}", parsed.ed2k_hash);
            if let Some(aich) = parsed.aich_root {
                println!(
                    "AICH Root   : {}",
                    data_encoding::BASE32_NOPAD.encode(&aich)
                );
            }
            if !parsed.part_hashes.is_empty() {
                println!("Part Hashes : {} part(s)", parsed.part_hashes.len());
                for (i, ph) in parsed.part_hashes.iter().enumerate() {
                    println!("  Part #{:<3} : {}", i, hex::encode(ph));
                }
            }
            if !parsed.peer_sources.is_empty() {
                println!("Peer Sources: {} source(s)", parsed.peer_sources.len());
                for src in &parsed.peer_sources {
                    println!("  - {}", src);
                }
            }
            if !parsed.web_sources.is_empty() {
                println!("Web Sources : {} source(s)", parsed.web_sources.len());
                for ws in &parsed.web_sources {
                    println!("  - {}", ws);
                }
            }
            if !parsed.unknown_fields.is_empty() {
                println!("Unknown Tags: {:?}", parsed.unknown_fields);
            }
            println!("Canonical   : {}", parsed.to_link_string());
        }
        Err(err) => {
            eprintln!("Error parsing eD2K link: {err}");
            std::process::exit(1);
        }
    }
}
