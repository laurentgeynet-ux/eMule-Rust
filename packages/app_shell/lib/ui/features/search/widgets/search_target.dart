// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import '../../../../l10n/generated/app_localizations.dart';

/// Search transport target: connected eD2K server, global UDP broadcast to
/// known servers, or Kad DHT keyword index.
enum SearchTarget {
  server,
  global,
  kad;

  /// Libellé localisé du sélecteur de cible.
  String labelL10n(AppLocalizations l10n) {
    switch (this) {
      case SearchTarget.server:
        return l10n.searchTargetServer;
      case SearchTarget.global:
        return l10n.globalUdp;
      case SearchTarget.kad:
        return l10n.searchTargetKad;
    }
  }

  /// Forme article défini utilisée dans les phrases (« Activez … »).
  String targetLabelL10n(AppLocalizations l10n) {
    switch (this) {
      case SearchTarget.server:
        return l10n.searchTargetEnableServer;
      case SearchTarget.global:
        return l10n.searchTargetEnableGlobal;
      case SearchTarget.kad:
        return l10n.searchTargetLabelKad;
    }
  }
}
