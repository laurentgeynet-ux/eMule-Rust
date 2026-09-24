// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use mule_format::KnownMetFile;
use std::path::Path;

pub fn inspect_known_met(file: &Path) {
    println!("== Inspecting known.met file: {} ==", file.display());
    match KnownMetFile::open(file) {
        Ok(met) => {
            println!(
                "Layout      : {}",
                if met.i64_tags {
                    "MET_HEADER_I64TAGS (0x0F)"
                } else {
                    "MET_HEADER (0x0E)"
                }
            );
            println!("Records     : {}", met.records.len());
            if met.dropped_records > 0 {
                println!(
                    "Dropped     : {} (failed eMule hash-set verification)",
                    met.dropped_records
                );
            }
            for (i, rec) in met.records.iter().enumerate() {
                println!("-- Record {i} --");
                println!("  Name        : {}", rec.file_name.as_deref().unwrap_or("<unknown>"));
                println!("  Hash        : {}", rec.file_hash.to_hex());
                println!(
                    "  Size        : {}",
                    rec.file_size
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "<unknown>".into())
                );
                println!("  Part hashes : {}", rec.part_hashes.len());
                println!("  Modified    : {} (utc)", rec.last_modified_utc);
                if let Some(aich) = &rec.aich_root {
                    println!("  AICH root   : {aich}");
                }
                println!("  Transferred : {} bytes", rec.transferred_bytes);
                println!("  Req/Acc     : {}/{}", rec.requested, rec.accepted);
                if let Some(p) = rec.up_priority {
                    println!("  Up priority : {p}");
                }
                if !rec.extra_tags.is_empty() {
                    println!("  Extra tags  : {}", rec.extra_tags.len());
                }
            }
        }
        Err(err) => {
            eprintln!("Error reading known.met file: {err}");
            std::process::exit(1);
        }
    }
}
