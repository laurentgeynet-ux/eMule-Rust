// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Fetch HTTP(S) borné et validé pour les imports RPC distants
//! (`servers.updateFromUrl`, `kad.bootstrapFromUrl`).
//!
//! Pipeline : [`parse_fetch_url`] → résolution DNS →
//! [`validate_fetch_ips`] → reqwest avec **redirections désactivées** et
//! **IP épinglées** (`resolve_to`, anti DNS-rebind) → corps lu en
//! streaming avec plafond [`MAX_FETCH_BODY_BYTES`].

use std::net::SocketAddr;

use mule_network_policy::{
    parse_fetch_url, pinned_ips, validate_fetch_ips, FetchUrl,
};

use crate::error::JsonRpcError;
use crate::state::AppState;

/// Taille maximale d'un corps de réponse téléchargé (server.met /
/// nodes.dat restent très loin en dessous ; 16 MiB couvre les listes
/// volumineuses tout en bornant l'allocation mémoire).
pub const MAX_FETCH_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Timeout global de la requête de fetch.
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Télécharge `raw_url` après validation anti-SSRF du profil courant et
/// renvoie le corps borné. Aucun octet n'est émis avant validation.
pub async fn fetch_url_bytes(
    state: &AppState,
    raw_url: &str,
) -> Result<(FetchUrl, Vec<u8>), JsonRpcError> {
    let target = parse_fetch_url(raw_url).map_err(|e| JsonRpcError::business(e.to_string()))?;

    // Résolution DNS si l'hôte n'est pas une IP littérale — toutes les
    // adresses retournées doivent passer la politique (une entrée
    // privée dans un round-robin suffit à rejeter).
    let resolved: Vec<std::net::IpAddr> = match target.literal_ip {
        Some(ip) => vec![ip],
        None => tokio::net::lookup_host(format!("{}:{}", target.host, target.port))
            .await
            .map(|addrs| addrs.map(|sa| sa.ip()).collect())
            .unwrap_or_default(),
    };

    validate_fetch_ips(&target, &resolved, state.network_policy.profile())
        .map_err(|e| JsonRpcError::business(e.to_string()))?;

    // Épingle la première IP validée : le client ne re-résout pas, un
    // rebind DNS ultérieur ne peut pas détourner la connexion.
    let pinned: Option<SocketAddr> = pinned_ips(&target, &resolved)
        .into_iter()
        .next()
        .map(|ip| SocketAddr::new(ip, target.port));

    let mut builder = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none());
    if let Some(addr) = pinned {
        builder = builder.resolve(&target.host, addr);
    }
    let client = builder
        .build()
        .map_err(|e| JsonRpcError::business(format!("HTTP client build error: {e}")))?;

    let mut response = client
        .get(target.url.clone())
        .send()
        .await
        .map_err(|e| JsonRpcError::business(format!("Failed to fetch {}: {e}", target.url)))?;

    if !response.status().is_success() {
        return Err(JsonRpcError::business(format!(
            "HTTP request to {} returned status {}",
            target.url,
            response.status()
        )));
    }

    if let Some(len) = response.content_length() {
        if len > MAX_FETCH_BODY_BYTES as u64 {
            return Err(JsonRpcError::business(format!(
                "Response from {} exceeds {MAX_FETCH_BODY_BYTES} bytes",
                target.url
            )));
        }
    }

    // Lecture bornée : content-length peut être absent ou menteur.
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| JsonRpcError::business(format!("Failed to read response body: {e}")))?
    {
        if body.len() + chunk.len() > MAX_FETCH_BODY_BYTES {
            return Err(JsonRpcError::business(format!(
                "Response from {} exceeds {MAX_FETCH_BODY_BYTES} bytes",
                target.url
            )));
        }
        body.extend_from_slice(&chunk);
    }

    Ok((target, body))
}
