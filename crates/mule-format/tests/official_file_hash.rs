// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

#[test]
fn test_hash_official_win_magazine() {
    let path = std::path::Path::new(r"D:\eMule0.50a\Incoming\Win Magazine 342 - Ottobre 2026.pdf");
    if !path.exists() {
        println!("Official file not found, skipping");
        return;
    }
    let mut file = std::fs::File::open(path).unwrap();
    let res = mule_format::ed2k::hash_reader(&mut file).unwrap();
    let hex_hash = hex::encode(res.root_hash.0);
    println!("Official file computed eD2K hash: {}", hex_hash);
    assert_eq!(hex_hash, "1b931f100ca678f285fd3f1daa204446");
}
