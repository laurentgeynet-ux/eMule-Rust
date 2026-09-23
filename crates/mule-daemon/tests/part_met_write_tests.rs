// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! Tests for `.part.met` companion file emission (eMule SavePartFile parity).

use md4::{Digest, Md4};
use mule_core::AllocationPolicy;
use mule_daemon::services::downloads::DownloadService;
use mule_daemon::{AppState, DaemonConfig};
use tempfile::tempdir;

#[test]
fn part_met_is_written_on_add_and_updated_on_progress() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let content = vec![0x55u8; 1000];
    let file_hash: [u8; 16] = Md4::digest(&content).into();
    let link = format!("ed2k://|file|sample.bin|1000|{}|/", hex::encode(file_hash));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    let (temp_path, _, _, _) = svc.get_download_file_info(&snap.id).unwrap();
    let mut met_name = temp_path.as_os_str().to_os_string();
    met_name.push(".met");
    let met_path = std::path::PathBuf::from(met_name);

    assert!(met_path.exists(), ".part.met must be created on add");

    let met = mule_format::PartMetFile::open(&met_path).unwrap();
    assert_eq!(met.file_hash.0, file_hash);
    assert_eq!(met.file_name.as_deref(), Some("sample.bin"));
    assert_eq!(met.file_size, Some(1000));
    assert_eq!(met.gaps, vec![(0, 1000)]);
    assert_eq!(
        met.compute_completed_ranges().unwrap(),
        Vec::<(u64, u64)>::new()
    );

    // After injecting data the met reflects covered ranges (no gaps).
    svc.inject_range(&snap.id, 0, &content).unwrap();
    let met = mule_format::PartMetFile::open(&met_path).unwrap();
    assert_eq!(met.transferred, 1000);
    assert!(met.gaps.is_empty());
    assert_eq!(
        met.compute_completed_ranges().unwrap(),
        vec![(0u64, 1000u64)]
    );
}

#[test]
fn part_met_is_deleted_on_remove_with_files() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let link = format!("ed2k://|file|gone.bin|500|{}|/", hex::encode([0x77u8; 16]));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();
    let (temp_path, _, _, _) = svc.get_download_file_info(&snap.id).unwrap();
    let mut met_name = temp_path.as_os_str().to_os_string();
    met_name.push(".met");
    let met_path = std::path::PathBuf::from(met_name);
    assert!(met_path.exists());

    svc.remove_download(&snap.id, true).unwrap();
    assert!(
        !met_path.exists(),
        ".part.met must be removed with delete_files"
    );
    assert!(!temp_path.exists());
}

#[test]
fn completed_file_is_deleted_and_unshared_on_remove_with_files() {
    let tmp = tempdir().unwrap();
    let target_dir = tmp.path().join("incoming");
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    // Le hash du lien doit correspondre au contenu injecté (fichier
    // mono-part : vérifié à la complétion).
    let content = vec![0x42u8; 5000];
    let mut hasher = mule_format::ed2k::Ed2kHasher::new();
    hasher.update(&content);
    let file_hash = hasher.finalize().unwrap().root_hash.to_hex();
    let link = format!("ed2k://|file|done.bin|5000|{file_hash}|/");
    let snap = svc
        .add_ed2k_link(&link, &target_dir, AllocationPolicy::None, None)
        .unwrap();
    svc.inject_range(&snap.id, 0, &content).unwrap();
    svc.finalize_download(&snap.id).unwrap();

    let target_path = target_dir.join("done.bin");
    assert!(target_path.exists(), "finalized file must exist");
    assert!(
        !svc.list_shared_files().unwrap().is_empty(),
        "finalized file must be registered as shared"
    );

    svc.remove_download(&snap.id, true).unwrap();
    assert!(
        !target_path.exists(),
        "completed file must be deleted with delete_files"
    );
    assert!(
        svc.list_shared_files().unwrap().is_empty(),
        "shared entry must be removed with the file"
    );
}

#[test]
fn completed_file_is_kept_on_remove_without_files() {
    let tmp = tempdir().unwrap();
    let target_dir = tmp.path().join("incoming");
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let content = vec![0x24u8; 5000];
    let mut hasher = mule_format::ed2k::Ed2kHasher::new();
    hasher.update(&content);
    let file_hash = hasher.finalize().unwrap().root_hash.to_hex();
    let link = format!("ed2k://|file|kept.bin|5000|{file_hash}|/");
    let snap = svc
        .add_ed2k_link(&link, &target_dir, AllocationPolicy::None, None)
        .unwrap();
    svc.inject_range(&snap.id, 0, &content).unwrap();
    svc.finalize_download(&snap.id).unwrap();

    let target_path = target_dir.join("kept.bin");
    svc.remove_download(&snap.id, false).unwrap();
    assert!(
        target_path.exists(),
        "completed file must be kept without delete_files"
    );
    assert!(
        !svc.list_shared_files().unwrap().is_empty(),
        "kept file stays shared"
    );
}

#[test]
fn session_downloaded_bytes_counts_only_bytes_received_this_session() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let link = format!("ed2k://|file|sess.bin|1000|{}|/", hex::encode([0x11u8; 16]));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    assert_eq!(svc.session_downloaded_bytes(), 0);
    svc.inject_range(&snap.id, 0, &[0xAAu8; 400]).unwrap();
    assert_eq!(svc.session_downloaded_bytes(), 400);
    svc.inject_range(&snap.id, 400, &[0xBBu8; 200]).unwrap();
    assert_eq!(svc.session_downloaded_bytes(), 600);
}

#[test]
fn shared_file_delete_removes_disk_file_and_entry() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    // Fichier partagé « hors downloads » (rescan Incoming / shares.add).
    let dir = tmp.path().join("incoming");
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("loose.bin");
    std::fs::write(&file_path, b"data").unwrap();
    let hash = "aabbccddeeff00112233445566778899";
    svc.add_shared_file(&mule_core::model::SharedFile::new(
        hash,
        "loose.bin",
        4,
        file_path.to_string_lossy().as_ref(),
    ))
    .unwrap();

    assert!(svc.remove_shared_file(hash, true).unwrap());
    assert!(!file_path.exists(), "delete_file must remove the disk file");
    assert!(svc.get_shared_file(hash).unwrap().is_none());
}

#[test]
fn shared_file_delete_keeps_disk_file_without_flag() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let dir = tmp.path().join("incoming");
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("kept_share.bin");
    std::fs::write(&file_path, b"data").unwrap();
    let hash = "11223344556677889900aabbccddeeff";
    svc.add_shared_file(&mule_core::model::SharedFile::new(
        hash,
        "kept_share.bin",
        4,
        file_path.to_string_lossy().as_ref(),
    ))
    .unwrap();

    assert!(svc.remove_shared_file(hash, false).unwrap());
    assert!(file_path.exists(), "unshare must keep the disk file");
    assert!(svc.get_shared_file(hash).unwrap().is_none());
}

#[test]
fn shared_file_delete_rejects_partial_share() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let link = format!("ed2k://|file|partial.bin|500|{}|/", hex::encode([0x77u8; 16]));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();
    let hash = snap.ed2k_hash.clone();

    // Le partage partiel est résolu depuis le download actif.
    let sf = svc.get_shared_file(&hash).unwrap().unwrap();
    assert!(sf.is_partial);

    assert!(svc.remove_shared_file(&hash, true).is_err());
    let (temp_path, _, _, _) = svc.get_download_file_info(&snap.id).unwrap();
    assert!(temp_path.exists(), "partial .part file must be untouched");
    assert!(svc.get_download(&snap.id).is_some());
}

#[test]
fn shared_file_delete_completed_routes_through_remove_download() {
    let tmp = tempdir().unwrap();
    let target_dir = tmp.path().join("incoming");
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let content = vec![0x5Au8; 5000];
    let mut hasher = mule_format::ed2k::Ed2kHasher::new();
    hasher.update(&content);
    let file_hash = hasher.finalize().unwrap().root_hash.to_hex();
    let link = format!("ed2k://|file|shared_done.bin|5000|{file_hash}|/");
    let snap = svc
        .add_ed2k_link(&link, &target_dir, AllocationPolicy::None, None)
        .unwrap();
    svc.inject_range(&snap.id, 0, &content).unwrap();
    svc.finalize_download(&snap.id).unwrap();

    let target_path = target_dir.join("shared_done.bin");
    assert!(target_path.exists());

    // La suppression via le partage retire aussi l'entrée download
    // complétée (sinon elle repointait un fichier inexistant).
    assert!(svc.remove_shared_file(&file_hash, true).unwrap());
    assert!(!target_path.exists());
    assert!(svc.get_shared_file(&file_hash).unwrap().is_none());
    assert!(svc.get_download(&snap.id).is_none());
}

/// `downloads.rename` sur un téléchargement en cours : met à jour le nom
/// affiché et le chemin cible sans toucher le `.part` (nommé par UUID).
#[test]
fn rename_in_progress_updates_name_and_target_not_part_file() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let link = format!("ed2k://|file|old_name.bin|500|{}|/", hex::encode([0x44u8; 16]));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();
    let (temp_path, _, _, _) = svc.get_download_file_info(&snap.id).unwrap();
    let part_path = temp_path.clone();

    let renamed = svc
        .rename_download(&snap.id, "new_name.bin")
        .unwrap();
    assert_eq!(renamed.file_name, "new_name.bin");
    assert_eq!(
        svc.get_download_file_info(&snap.id).unwrap().1,
        tmp.path().join("new_name.bin")
    );
    // Le .part UUID ne bouge pas.
    assert!(part_path.exists());
    assert_eq!(
        svc.get_download(&snap.id).unwrap().file_name,
        "new_name.bin"
    );
}

/// Download terminé : le renommage déplace le fichier disque et met à
/// jour l'entrée `shared_files` (nom + chemin).
#[test]
fn rename_completed_renames_disk_file_and_share_entry() {
    let tmp = tempdir().unwrap();
    let target_dir = tmp.path().join("incoming");
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let content = vec![0x66u8; 500];
    let file_hash: [u8; 16] = Md4::digest(&content).into();
    let hash_hex = hex::encode(file_hash);
    let link = format!("ed2k://|file|before.iso|500|{hash_hex}|/");
    let snap = svc
        .add_ed2k_link(&link, &target_dir, AllocationPolicy::None, None)
        .unwrap();
    svc.inject_range(&snap.id, 0, &content).unwrap();
    svc.finalize_download(&snap.id).unwrap();

    let old_path = target_dir.join("before.iso");
    let new_path = target_dir.join("after.iso");
    assert!(old_path.exists());

    svc.rename_download(&snap.id, "after.iso").unwrap();
    assert!(!old_path.exists());
    assert!(new_path.exists());

    let shared = svc.get_shared_file(&hash_hex).unwrap().unwrap();
    assert_eq!(shared.file_name, "after.iso");
    assert_eq!(shared.file_path, new_path.to_string_lossy());
}

/// `shares.rename` délègue à `rename_download` quand le hash adosse un
/// download complété — les deux vues restent cohérentes.
#[test]
fn rename_shared_file_by_hash_delegates_to_completed_download() {
    let tmp = tempdir().unwrap();
    let target_dir = tmp.path().join("incoming");
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let content = vec![0x77u8; 500];
    let file_hash: [u8; 16] = Md4::digest(&content).into();
    let hash_hex = hex::encode(file_hash);
    let link = format!("ed2k://|file|dl.bin|500|{hash_hex}|/");
    let snap = svc
        .add_ed2k_link(&link, &target_dir, AllocationPolicy::None, None)
        .unwrap();
    svc.inject_range(&snap.id, 0, &content).unwrap();
    svc.finalize_download(&snap.id).unwrap();

    svc.rename_shared_file(&hash_hex, "dl_renamed.bin").unwrap();
    assert!(target_dir.join("dl_renamed.bin").exists());
    assert_eq!(
        svc.get_download(&snap.id).unwrap().file_name,
        "dl_renamed.bin"
    );
    assert_eq!(
        svc.get_shared_file(&hash_hex).unwrap().unwrap().file_name,
        "dl_renamed.bin"
    );
}

/// Les noms invalides (vides, séparateurs, caractères interdits Windows)
/// sont refusés sans toucher au download.
#[test]
fn rename_rejects_invalid_file_names() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let link = format!("ed2k://|file|keep.bin|500|{}|/", hex::encode([0x55u8; 16]));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    for bad in ["", "   ", "a/b.bin", "a\\b.bin", "x*.bin", "nul?.bin", "..", "name."] {
        assert!(
            svc.rename_download(&snap.id, bad).is_err(),
            "name {bad:?} must be rejected"
        );
    }
    assert_eq!(svc.get_download(&snap.id).unwrap().file_name, "keep.bin");
    assert!(svc.rename_shared_file("deadbeef", "a|b.bin").is_err());
}

#[test]
fn part_met_persists_partial_gaps() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let link = format!(
        "ed2k://|file|sparse.bin|300|{}|/",
        hex::encode([0x33u8; 16])
    );
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    // Partial data does not trigger part completion; force a save via pause.
    svc.inject_range(&snap.id, 0, &[0xAAu8; 100]).unwrap();
    svc.pause_download(&snap.id).unwrap();

    let (temp_path, _, _, _) = svc.get_download_file_info(&snap.id).unwrap();
    let mut met_name = temp_path.as_os_str().to_os_string();
    met_name.push(".met");
    let met = mule_format::PartMetFile::open(std::path::PathBuf::from(met_name)).unwrap();
    assert_eq!(met.gaps, vec![(100, 300)]);
    assert_eq!(
        met.compute_completed_ranges().unwrap(),
        vec![(0u64, 100u64)]
    );
    assert_eq!(met.transferred, 100);
}

/// `stats.resetLifetimeTotals` remet à zéro les compteurs cumulés en
/// mémoire ET persistés (settings SQLite) ; les valeurs rechargées au
/// boot repartent bien à zéro.
#[tokio::test]
async fn reset_lifetime_totals_zeroes_memory_and_persisted_settings() {
    use std::sync::atomic::Ordering;

    let tmp = tempdir().unwrap();
    let mut downloads =
        DownloadService::in_memory(tmp.path().join("temp")).unwrap();
    downloads
        .set_setting("lifetime_downloaded_bytes", "12345")
        .unwrap();
    downloads
        .set_setting("lifetime_uploaded_bytes", "6789")
        .unwrap();

    let config = DaemonConfig::new(tmp.path().to_path_buf(), "t");
    let state = AppState::new(config, downloads);

    // Les compteurs sont hydratés depuis les settings persistés.
    assert_eq!(state.lifetime_downloaded_bytes.load(Ordering::SeqCst), 12345);
    assert_eq!(state.lifetime_uploaded_bytes.load(Ordering::SeqCst), 6789);

    mule_daemon::rpc::handlers::settings::handle_stats_reset_lifetime_totals(&state)
        .await
        .unwrap();

    assert_eq!(state.lifetime_downloaded_bytes.load(Ordering::SeqCst), 0);
    assert_eq!(state.lifetime_uploaded_bytes.load(Ordering::SeqCst), 0);

    let dl = state.downloads.lock().await;
    assert_eq!(
        dl.get_setting("lifetime_downloaded_bytes").unwrap().as_deref(),
        Some("0")
    );
    assert_eq!(
        dl.get_setting("lifetime_uploaded_bytes").unwrap().as_deref(),
        Some("0")
    );
}

/// `shares.list` enrichit chaque fichier du débit mesuré agrégé sur ses
/// slots actifs et du total uploadé cumulé (hydraté depuis
/// `share_upload_stats`) — jointure insensible à la casse du hash.
#[tokio::test]
async fn shares_list_enriches_per_file_upload_stats() {
    let tmp = tempdir().unwrap();
    let mut downloads =
        DownloadService::in_memory(tmp.path().join("temp")).unwrap();
    // Total persisté d'un « boot précédent ».
    downloads.add_share_uploaded_bytes("HAA", 7000).unwrap();
    let config = DaemonConfig::new(tmp.path().to_path_buf(), "t");
    let state = AppState::new(config, downloads);

    {
        let mut dl = state.downloads.lock().await;
        dl.add_shared_file(&mule_core::model::SharedFile::new(
            "haa", "f.iso", 100, "f.iso",
        ))
        .unwrap();
        dl.add_shared_file(&mule_core::model::SharedFile::new(
            "hbb", "g.iso", 100, "g.iso",
        ))
        .unwrap();
    }
    let um = state.inbound_listener.upload_manager();
    // Hydratation au boot : le total persisté doit être visible.
    assert_eq!(um.per_file_uploaded_bytes()["haa"], 7000);

    // Deux slots actifs sur le même fichier (casse différente du hash).
    um.request_upload_slot("c1", "c1", "HAA", "f.iso", 1.0, "normal", false)
        .await
        .unwrap();
    um.request_upload_slot("c2", "c2", "haa", "f.iso", 1.0, "normal", false)
        .await
        .unwrap();
    um.update_progress("c1", 3000).await;
    um.update_progress("c2", 500).await;
    // La fenêtre de mesure est ouverte au premier envoi — un second envoi
    // après le premier échantillon (≥ 500 ms) produit un débit mesuré.
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    um.update_progress("c1", 1000).await;

    let out = mule_daemon::rpc::handlers::shares::handle_shares_list(&state)
        .await
        .unwrap();
    let files = out["sharedFiles"].as_array().unwrap();
    let f = files.iter().find(|f| f["fileHash"] == "haa").unwrap();
    assert_eq!(f["uploadedBytes"].as_u64().unwrap(), 7000 + 4500);
    assert!(f["uploadRateBps"].as_u64().unwrap() > 0);
    // Le fichier sans activité : débit 0, total 0.
    let g = files.iter().find(|f| f["fileHash"] == "hbb").unwrap();
    assert_eq!(g["uploadedBytes"].as_u64().unwrap(), 0);
    assert_eq!(g["uploadRateBps"].as_u64().unwrap(), 0);
}
