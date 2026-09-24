// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Constructeurs d'`AppState` de test : chaque fonction correspond à un
//! scénario récurrent des tests d'intégration (RPC in-memory, Kad persistant,
//! pair avec téléchargement, partage avec fichier réel).

use tempfile::{tempdir, TempDir};

use mule_daemon::rpc::methods::handle_request;
use mule_daemon::{AppState, DaemonConfig, DownloadService, WorkDirMode};
use mule_network_policy::NetworkProfile;
use serde_json::json;

use crate::rpc::make_req;

/// État minimal en mémoire : workdir temporaire, mode [`WorkDirMode::Portable`],
/// stockage des téléchargements en mémoire.
pub fn rpc_state(token: &str) -> (AppState, TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");

    let config = DaemonConfig::new(dir.path().to_path_buf(), token)
        .with_workdir_mode(WorkDirMode::Portable);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);

    (state, dir)
}

/// État en mémoire dont la [`DaemonConfig`] est personnalisée avant
/// construction (profil réseau, upnp, obfuscation…). Le mode
/// [`WorkDirMode::Portable`] reste imposé après `customize`.
pub fn rpc_state_with_config(
    token: &str,
    customize: impl FnOnce(DaemonConfig) -> DaemonConfig,
) -> (AppState, TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");

    let config = customize(DaemonConfig::new(dir.path().to_path_buf(), token))
        .with_workdir_mode(WorkDirMode::Portable);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);

    (state, dir)
}

/// État en mémoire avec profil réseau forcé (tests de politique réseau).
pub fn rpc_state_with_profile(token: &str, profile: NetworkProfile) -> (AppState, TempDir) {
    let (state, dir) = rpc_state(token);
    state.network_policy.set_profile(profile);
    (state, dir)
}

/// État Kad persistant : arborescence `data/` + `downloads/` dans `tmp`, base
/// SQLite réelle, mode [`WorkDirMode::ExplicitCli`], token fixe `test-token`.
pub fn kad_state(tmp: &TempDir) -> AppState {
    let data_dir = tmp.path().join("data");
    let dl_dir = tmp.path().join("downloads");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::create_dir_all(&dl_dir).unwrap();

    let db_path = data_dir.join("mule.db");
    let downloads = DownloadService::new(&db_path, data_dir.join("temp")).unwrap();
    let config = DaemonConfig::new(data_dir, "test-token")
        .with_bind_addr("127.0.0.1:0".parse().unwrap())
        .with_default_download_dir(dl_dir)
        .with_workdir_mode(WorkDirMode::ExplicitCli);

    AppState::new(config, downloads)
}

/// État LocalLab avec un téléchargement enregistré via `downloads.addEd2kLink`.
/// Retourne `(state, workdir, hash_fichier_hex)` — le hash passé en paramètre
/// est renvoyé tel quel pour les assertions.
pub async fn download_state(
    token: &str,
    file_name: &str,
    file_size: u64,
    file_hash_hex: &str,
) -> (AppState, TempDir, String) {
    download_state_inner(token, file_name, file_size, file_hash_hex, None).await
}

/// Variante de [`download_state`] activant l'obfuscation protocolaire
/// (`enabled`, `required`).
pub async fn download_state_with_obfuscation(
    token: &str,
    file_name: &str,
    file_size: u64,
    file_hash_hex: &str,
    obfuscation_enabled: bool,
    obfuscation_required: bool,
) -> (AppState, TempDir, String) {
    download_state_inner(
        token,
        file_name,
        file_size,
        file_hash_hex,
        Some((obfuscation_enabled, obfuscation_required)),
    )
    .await
}

/// État avec un partage `shares.add` adossé à un fichier réel écrit sur disque.
/// Le chemin upload ne sert que des octets lisibles : `content` est le contenu
/// physique du fichier, `declared_size` la taille annoncée au réseau.
/// Retourne `(state, workdir, hash_fichier_hex)`.
pub async fn share_state(
    token: &str,
    file_name: &str,
    declared_size: u64,
    content: &[u8],
    file_hash_hex: &str,
) -> (AppState, TempDir, String) {
    let (state, dir) = rpc_state(token);

    let shared_path = dir.path().join(file_name);
    std::fs::write(&shared_path, content).unwrap();

    let mut authenticated = true;
    let share_req = make_req(
        1,
        "shares.add",
        Some(json!({
            "fileHash": file_hash_hex,
            "fileName": file_name,
            "fileSize": declared_size,
            "filePath": shared_path.to_string_lossy()
        })),
    );
    let resp = handle_request(&state, share_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    (state, dir, file_hash_hex.to_string())
}

async fn download_state_inner(
    token: &str,
    file_name: &str,
    file_size: u64,
    file_hash_hex: &str,
    obfuscation: Option<(bool, bool)>,
) -> (AppState, TempDir, String) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");

    let config = DaemonConfig::new(dir.path().to_path_buf(), token)
        .with_workdir_mode(WorkDirMode::Portable);
    let config = match obfuscation {
        Some((enabled, required)) => config.with_obfuscation(enabled, required),
        None => config,
    };
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);
    state
        .network_policy
        .set_profile(NetworkProfile::LocalLab);

    // Enregistre un téléchargement pour donner une cible aux sessions pair.
    let link = format!("ed2k://|file|{file_name}|{file_size}|{file_hash_hex}|/");
    let req = make_req(1, "downloads.addEd2kLink", Some(json!({ "link": link })));
    let mut authenticated = true;
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    (state, dir, file_hash_hex.to_string())
}
