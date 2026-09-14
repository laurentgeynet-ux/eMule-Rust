use clap::{Parser, Subcommand};
use mule_format::{calculate_aich_root, hash_reader, Ed2kFileLink, PartMetFile};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

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
        Commands::InspectLink { link } => {
            println!("== Inspecting eD2K Link ==");
            match Ed2kFileLink::parse(&link) {
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
        Commands::Hash { file } => {
            println!("== Computing eD2K & AICH Hashes: {} ==", file.display());
            let mut f = File::open(&file)?;
            let ed2k_res = hash_reader(&mut f)?;

            println!("File Size   : {} bytes", ed2k_res.total_bytes);
            println!("eD2K Hash   : {}", ed2k_res.root_hash);
            println!("Part Count  : {}", ed2k_res.part_hashes.len());
            for (i, ph) in ed2k_res.part_hashes.iter().enumerate() {
                println!("  Part #{:<3} : {}", i, hex::encode(ph));
            }

            // Also compute AICH if feasible in memory (or up to reasonable size)
            if let Ok(metadata) = std::fs::metadata(&file) {
                if metadata.len() <= 100 * 1024 * 1024 {
                    let mut content = Vec::new();
                    let mut f = File::open(&file)?;
                    f.read_to_end(&mut content)?;
                    if let Ok(aich) = calculate_aich_root(&content) {
                        println!("AICH Root   : {}", aich.to_base32());
                    }
                }
            }
        }
        Commands::InspectPartMet { file } => {
            println!("== Inspecting .part.met file: {} ==", file.display());
            match PartMetFile::open(&file) {
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
        Commands::MakeLink { file, detailed } => {
            if !file.exists() {
                eprintln!("Error: File not found: {}", file.display());
                std::process::exit(1);
            }
            if file.is_dir() {
                eprintln!("Error: Path is a directory, not a regular file: {}", file.display());
                std::process::exit(1);
            }

            let file_name = match file.file_name().and_then(|n| n.to_str()) {
                Some(name) if !name.is_empty() => name.to_string(),
                _ => {
                    eprintln!("Error: Invalid or empty file name");
                    std::process::exit(1);
                }
            };

            let mut f = File::open(&file)?;
            let ed2k_res = match hash_reader(&mut f) {
                Ok(res) => res,
                Err(err) => {
                    eprintln!("Error hashing file '{}': {err}", file.display());
                    std::process::exit(1);
                }
            };

            let mut link = match Ed2kFileLink::create(&file_name, ed2k_res.total_bytes, ed2k_res.root_hash) {
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
            if let Ok(metadata) = std::fs::metadata(&file) {
                if metadata.len() <= 100 * 1024 * 1024 {
                    let mut content = Vec::new();
                    let mut f = File::open(&file)?;
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
        }
    }

    Ok(())
}
