// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/widgets.dart';

import 'generated/app_localizations.dart';

/// Résolution des localisations hors `BuildContext` — messages de
/// contrôleurs (snackbars `onStatus`/`onError`) et autres chaînes émises
/// en dehors de l'arbre de widgets. La locale est branchée sur
/// `LocaleController` au démarrage ; la résolution est faite à la volée,
/// donc un changement de langue s'applique aux messages suivants.
class L10nLookup {
  L10nLookup._();

  static Locale _locale = const Locale('fr');

  /// Appelé par `LocaleController` au chargement et à chaque changement
  /// de langue.
  static void bindLocale(Locale locale) => _locale = locale;

  /// Localisations pour la locale UI courante (repli anglais si la
  /// locale n'est pas encore branchée).
  static AppLocalizations get current => lookupAppLocalizations(_locale);
}
