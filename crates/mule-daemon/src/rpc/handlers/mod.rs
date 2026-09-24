// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

pub mod categories;
pub mod downloads;
pub mod friends;
pub mod history;
pub mod kad;
pub mod lab;
pub mod network;
pub mod servers;
pub mod settings;
pub mod shares;
pub mod sources;
pub mod upnp;

/// Sérialise une charge utile de handler en `serde_json::Value`.
/// Les payloads RPC sont `Serialize` par construction ; en cas d'échec
/// (inatteignable en pratique) on renvoie un objet vide plutôt qu'une erreur.
pub(crate) fn json_value<T: serde::Serialize + ?Sized>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or_default()
}
