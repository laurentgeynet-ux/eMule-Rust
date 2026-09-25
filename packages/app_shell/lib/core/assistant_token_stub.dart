// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

/// Stub web : le jeton de session n'est pas lisible sans système de
/// fichiers local — le panneau assistant est une fonction desktop.
Future<String?> readAssistantToken(String? workDirPath) async => null;
