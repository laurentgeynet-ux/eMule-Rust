// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Tests d'intégration du `CorruptionBlackBox` (eMule `CCorruptionBlackBox`),
//! de la garde « part déjà complète » de `WriteToBuffer` et du gating ICH —
//! tous hors-ligne.

use md4::{Digest, Md4};
use mule_core::model::{PeerSource, PeerSourceOrigin, PeerSourceStatus};
use mule_core::AllocationPolicy;
use mule_daemon::services::downloads::DownloadService;
use tempfile::tempdir;

fn md4_of(data: &[u8]) -> [u8; 16] {
    Md4::digest(data).into()
}

fn make_source(file_hash: &str, addr: &str) -> PeerSource {
    PeerSource {
        file_hash: file_hash.to_string(),
        address: addr.to_string(),
        ip: 0,
        port: 0,
        origin: PeerSourceOrigin::Server,
        discovered_at_unix_ms: 0,
        last_seen_unix_ms: 0,
        status: PeerSourceStatus::Candidate,
        fail_count: 0,
        last_error: None,
        cooldown_until_unix_ms: None,
        user_hash: None,
        crypt_options: 0,
        buddy_id: None,
        software: None,
        availability: None,
        server_ip: 0,
        server_port: 0,
    }
}

/// eMule `WriteToBuffer` : toute donnée touchant une part déjà complète
/// (« already hashed chunk ») est ignorée — une réinjection dans une part
/// couverte ne réécrit ni n'étend les plages reçues.
#[test]
fn inject_into_completed_part_is_ignored() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    // Deux parts : l'injection de la part 0 ne termine pas le fichier.
    let link = format!(
        "ed2k://|file|two.bin|{}|{}|/",
        9_728_000 + 10,
        hex::encode([0xAAu8; 16])
    );
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    svc.inject_range(&snap.id, 0, &vec![0x11u8; 9_728_000], Some("9.9.9.9:4662"))
        .unwrap();
    assert_eq!(svc.get_covered_len(&snap.id), Some(9_728_000));

    // Réinjection (données différentes) dans la part couverte → ignorée.
    svc.inject_range(&snap.id, 100, &vec![0xEEu8; 500], Some("8.8.8.8:4662"))
        .unwrap();
    assert_eq!(
        svc.get_covered_len(&snap.id),
        Some(9_728_000),
        "write into completed part must be dropped"
    );

    // La part 1 reste injectable normalement.
    svc.inject_range(&snap.id, 9_728_000, &[0x22u8; 10], Some("8.8.8.8:4662"))
        .unwrap();
    assert_eq!(svc.get_covered_len(&snap.id), Some(9_728_000 + 10));
}

/// `CorruptionBlackBox::EvaluateData` : l'expéditeur d'une part entièrement
/// corrompue dépasse `CBB_BANTHRESHOLD` — il est condamné, déconnecté et ses
/// sources sont ignorées à l'ingestion (`IsBannedClient`).
#[test]
fn corrupt_sender_is_banned_and_filtered() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let content = vec![0x42u8; 100];
    let file_hash = md4_of(&content);
    let link = format!("ed2k://|file|small.bin|100|{}|/", hex::encode(file_hash));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    // Hashset délibérément faux → la part échoue la vérif MD4.
    svc.store_part_hashes_by_hash(&hex::encode(file_hash), &[[0xEE; 16]])
        .unwrap();

    svc.inject_range(&snap.id, 0, &content, Some("1.2.3.4:4662"))
        .unwrap();

    // L'expéditeur unique est condamné (>32 % corrompu) et banni de session.
    let bans = svc.drain_corruption_bans(&snap.id);
    assert_eq!(bans, vec!["1.2.3.4:4662".to_string()]);
    assert!(svc.is_source_banned("1.2.3.4:4662"));
    assert!(svc.drain_corruption_bans(&snap.id).is_empty());

    // eMule `AddSources` : les sources d'un client banni sont ignorées.
    let added = svc
        .upsert_peer_sources(
            &hex::encode(file_hash),
            &[
                make_source(&hex::encode(file_hash), "1.2.3.4:4662"),
                make_source(&hex::encode(file_hash), "5.6.7.8:4662"),
            ],
        )
        .unwrap();
    assert_eq!(added, 1, "banned sender must be filtered at ingestion");
}

/// Un pair qui a livré des données vérifiées en plus d'une part corrompue
/// peut rester sous le seuil : il n'est pas condamné.
#[test]
fn verified_history_dilutes_corruption_blame() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    // Fichier de 4 parts : 3 parts correctes puis une part corrompue par le
    // même expéditeur → ratio ~25 % < 32 % → pas de bannissement.
    let part = 9_728_000usize;
    let mut whole = vec![0x42u8; part * 4];
    let p_hashes: Vec<[u8; 16]> = (0..4)
        .map(|i| md4_of(&whole[i * part..(i + 1) * part]))
        .collect();
    // Corrompt la part 3.
    whole[3 * part + 100] ^= 0xFF;
    let file_hash = [0xBBu8; 16];
    let link = format!(
        "ed2k://|file|four.bin|{}|{}|/",
        part * 4,
        hex::encode(file_hash)
    );
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();
    svc.store_part_hashes_by_hash(&hex::encode(file_hash), &p_hashes)
        .unwrap();

    for i in 0..4usize {
        svc.inject_range(
            &snap.id,
            (i * part) as u64,
            &whole[i * part..(i + 1) * part],
            Some("1.2.3.4:4662"),
        )
        .unwrap();
    }

    let bans = svc.drain_corruption_bans(&snap.id);
    assert!(
        bans.is_empty(),
        "sender with 75%% verified history must stay under the 32%% ban threshold, got {bans:?}"
    );
}

/// `ich_enabled = 0` (préférence ICH désactivée) : la part corrompue est
/// évincée en entier au lieu d'attendre une récupération AICH par blocs.
#[test]
fn ich_disabled_evicts_whole_part_without_aich() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();
    svc.db_mut().set_setting("ich_enabled", "0").unwrap();

    // Racine AICH présente dans le lien — sans la préf désactivée la part
    // partirait en file AICH ; ici elle doit être évincée directement.
    let aich_root = [0x77u8; 20];
    let link = format!(
        "ed2k://|file|aich.bin|100|{}|h={}|/",
        hex::encode([0xAAu8; 16]),
        data_encoding::BASE32_NOPAD.encode(&aich_root)
    );
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();
    svc.store_part_hashes_by_hash(&hex::encode([0xAAu8; 16]), &[[0xEE; 16]])
        .unwrap();

    svc.inject_range(&snap.id, 0, &[0x42u8; 100], Some("1.2.3.4:4662"))
        .unwrap();

    // Part évincée en entier → plus aucune couverture, pas de file AICH.
    assert_eq!(svc.get_covered_len(&snap.id), Some(0));
    assert!(svc.next_aich_recovery(&snap.id).is_none());
}
