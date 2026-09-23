// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use mule_format::{calculate_aich_root, hash_reader, Ed2kFileLink};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub fn hash_file(file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("== Computing eD2K & AICH Hashes: {} ==", file.display());
    let mut f = File::open(file)?;
    let ed2k_res = hash_reader(&mut f)?;

    println!("File Size   : {} bytes", ed2k_res.total_bytes);
    println!("eD2K Hash   : {}", ed2k_res.root_hash);
    println!("Part Count  : {}", ed2k_res.part_hashes.len());
    for (i, ph) in ed2k_res.part_hashes.iter().enumerate() {
        println!("  Part #{:<3} : {}", i, hex::encode(ph));
    }

    // Also compute AICH if feasible in memory (or up to reasonable size)
    if let Ok(metadata) = std::fs::metadata(file) {
        if metadata.len() <= 100 * 1024 * 1024 {
            let mut content = Vec::new();
            let mut f = File::open(file)?;
            f.read_to_end(&mut content)?;
            if let Ok(aich) = calculate_aich_root(&content) {
                println!("AICH Root   : {}", aich.to_base32());
            }
        }
    }
    Ok(())
}

pub fn make_link(file: &Path, detailed: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !file.exists() {
        eprintln!("Error: File not found: {}", file.display());
        std::process::exit(1);
    }
    if file.is_dir() {
        eprintln!(
            "Error: Path is a directory, not a regular file: {}",
            file.display()
        );
        std::process::exit(1);
    }

    let file_name = match file.file_name().and_then(|n| n.to_str()) {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => {
            eprintln!("Error: Invalid or empty file name");
            std::process::exit(1);
        }
    };

    let mut f = File::open(file)?;
    let ed2k_res = match hash_reader(&mut f) {
        Ok(res) => res,
        Err(err) => {
            eprintln!("Error hashing file '{}': {err}", file.display());
            std::process::exit(1);
        }
    };

    let mut link = match Ed2kFileLink::create(&file_name, ed2k_res.total_bytes, ed2k_res.root_hash)
    {
        Ok(l) => l,
        Err(err) => {
            eprintln!("Error creating eD2K link: {err}");
            std::process::exit(1);
        }
    };

    // If file has multiple parts, include part hashes
    if ed2k_res.part_hashes.len() > 1 {
        link.part_hashes = ed2k_res.part_hashes.clone();
    }

    // Optionally compute AICH root if <= 100 MiB
    if let Ok(metadata) = std::fs::metadata(file) {
        if metadata.len() <= 100 * 1024 * 1024 {
            let mut content = Vec::new();
            let mut f = File::open(file)?;
            f.read_to_end(&mut content)?;
            if let Ok(aich) = calculate_aich_root(&content) {
                link.aich_root = Some(aich.0);
            }
        }
    }

    let link_str = link.to_link_string();

    if detailed {
        println!("File       : {}", file_name);
        println!("Size       : {} bytes", ed2k_res.total_bytes);
        println!("eD2K hash  : {}", ed2k_res.root_hash);
        if let Some(aich) = link.aich_root {
            println!("AICH root  : {}", data_encoding::BASE32_NOPAD.encode(&aich));
        }
        if !link.part_hashes.is_empty() {
            println!("Parts      : {}", link.part_hashes.len());
        }
        println!("eD2K link  : {}", link_str);
    } else {
        println!("{}", link_str);
    }
    Ok(())
}
