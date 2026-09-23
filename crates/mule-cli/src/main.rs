// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use mule_cli::commands::{hash, link, part_met};

#[derive(Parser)]
#[command(name = "mule-cli")]
#[command(about = "Inspection CLI for eD2K links, hashing, and eMule metadata files", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect and validate an ed2k:// link
    InspectLink {
        /// The ed2k:// URI string
        link: String,
    },
    /// Compute deterministic eD2K and AICH hashes for a local file
    Hash {
        /// Path to the file
        file: PathBuf,
    },
    /// Inspect an eMule .part.met file
    InspectPartMet {
        /// Path to the .part.met file
        file: PathBuf,
    },
    /// Generate an ed2k:// file link from a local file
    MakeLink {
        /// Path to the local file
        file: PathBuf,
        /// Output detailed information alongside the link
        #[arg(long, short)]
        detailed: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::InspectLink { link: link_uri } => {
            link::inspect_link(&link_uri);
        }
        Commands::Hash { file } => {
            hash::hash_file(&file)?;
        }
        Commands::InspectPartMet { file } => {
            part_met::inspect_part_met(&file);
        }
        Commands::MakeLink { file, detailed } => {
            hash::make_link(&file, detailed)?;
        }
    }

    Ok(())
}
