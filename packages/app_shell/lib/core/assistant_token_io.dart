// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:io';

/// Longueur du jeton de session daemon : 32 octets en hexadécimal
/// (miroir de `DaemonSession::AUTH_TOKEN_HEX_LEN` côté Rust).
const int authTokenHexLen = 64;

/// Lit le jeton de session du daemon (`<workdir>/auth.token`) pour le
/// handshake `assistant.hello` — même jeton que le RPC, aucun secret
/// parallèle. Renvoie `null` si absent ou mal formé.
Future<String?> readAssistantToken(String? workDirPath) async {
  if (workDirPath == null) return null;
  try {
    final file = File('$workDirPath${Platform.pathSeparator}auth.token');
    final raw = (await file.readAsString()).trim();
    final valid = raw.length == authTokenHexLen &&
        raw.split('').every((c) => '0123456789abcdefABCDEF'.contains(c));
    return valid ? raw : null;
  } catch (_) {
    return null;
  }
}
