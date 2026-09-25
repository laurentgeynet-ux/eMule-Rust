// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Données de test eD2K partagées entre les suites d'intégration.
//! Chaque suite n'utilise qu'un sous-ensemble des helpers — dead_code attendu.
#![allow(dead_code)]

#[cfg(feature = "ed2k-loopback")]
use std::net::SocketAddr;
#[cfg(feature = "ed2k-loopback")]
use std::time::Duration;

#[cfg(feature = "ed2k-loopback")]
use mule_protocol::ed2k::Ed2kPeerInfo;
#[cfg(feature = "ed2k-loopback")]
use mule_protocol::ed2k::tags::{
    Ed2kTag, TagName, TagValue, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
};
#[cfg(feature = "ed2k-loopback")]
use mule_protocol::ed2k::Ed2kLoopbackClient;

/// Hash utilisateur déterministe partagé par les handshakes de test.
#[cfg(feature = "ed2k-loopback")]
pub fn sample_user_hash() -> [u8; 16] {
    let mut h = [0x11; 16];
    h[0] = 0x42;
    h
}

/// Tags client nominaux : nom affiché + version eMule 0.60a (`0x3C`).
#[cfg(feature = "ed2k-loopback")]
pub fn sample_client_tags(nick: &str) -> Vec<Ed2kTag> {
    vec![
        Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_NAME),
            value: TagValue::String(nick.to_string()),
        },
        Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_VERSION),
            value: TagValue::UInt32(60),
        },
    ]
}

/// Hash fichier déterministe indexé (octet `i` = `i`).
pub fn sample_file_hash() -> [u8; 16] {
    let mut h = [0u8; 16];
    for (i, b) in h.iter_mut().enumerate() {
        *b = i as u8;
    }
    h
}

/// Hash 16 octets fixe des scénarios de reprise partielle / séquenceur.
pub fn resume_file_hash() -> [u8; 16] {
    [
        0x69, 0x5b, 0x81, 0xb1, 0x72, 0x76, 0x66, 0x81, 0x89, 0x88, 0x52, 0x4b, 0xd7, 0xfc, 0x82,
        0x3c,
    ]
}

/// Hash pseudo-aléatoire déterministe de taille arbitraire (octet `i` = `seed + i`).
pub fn seeded_hash<const N: usize>(seed: u8) -> [u8; N] {
    let mut h = [0u8; N];
    for (i, b) in h.iter_mut().enumerate() {
        *b = seed.wrapping_add(i as u8);
    }
    h
}

/// Handshake nominal sur un pair loopback : user hash + tags standard.
#[cfg(feature = "ed2k-loopback")]
pub async fn connect_nominal_client(
    peer_addr: SocketAddr,
    nick: &str,
) -> (Ed2kLoopbackClient, Ed2kPeerInfo) {
    Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        sample_user_hash(),
        0xC0A80001,
        39735,
        sample_client_tags(nick),
        0x7F000001,
        49661,
        Duration::from_millis(500),
    )
    .await
    .expect("handshake failed")
}

/// Variante ne renvoyant que le client (majorité des tests).
#[cfg(feature = "ed2k-loopback")]
pub async fn nominal_client(peer_addr: SocketAddr, nick: &str) -> Ed2kLoopbackClient {
    connect_nominal_client(peer_addr, nick).await.0
}
