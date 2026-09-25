// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';

import '../../../../../l10n/generated/app_localizations.dart';

/// Un fichier de configuration eMule importable via un RPC `*.import*`.
///
/// Chaque source sait : son nom de fichier, son icône, son libellé
/// localisé, l'appel client correspondant et le résumé de son rapport.
enum MigrationSource {
  knownMet('known.met', Icons.folder_shared_outlined),
  serverMet('server.met', Icons.dns_outlined),
  nodesDat('nodes.dat', Icons.hub_outlined),
  emfriendsMet('emfriends.met', Icons.people_alt_outlined),
  cancelledMet('cancelled.met', Icons.cancel_outlined),
  clientsMet('clients.met', Icons.handshake_outlined),
  identity('cryptkey.dat', Icons.key_outlined);

  const MigrationSource(this.fileName, this.icon);

  /// Nom du fichier eMule attendu dans le dossier config.
  /// Pour [identity], fichier principal (`cryptkey.dat`) — la source est
  /// aussi détectée via `preferences.dat` seul.
  final String fileName;
  final IconData icon;

  /// Nom de fichier secondaire (identité : `preferences.dat`).
  String? get alternateFileName =>
      this == MigrationSource.identity ? 'preferences.dat' : null;

  /// `true` si le fichier (ou l'alternatif) existe dans `dir`.
  bool detectedIn(String dir) {
    final sep = Platform.pathSeparator;
    if (File('$dir$sep$fileName').existsSync()) return true;
    final alt = alternateFileName;
    return alt != null && File('$dir$sep$alt').existsSync();
  }

  /// Libellé localisé de la source.
  String label(AppLocalizations l10n) => switch (this) {
        MigrationSource.knownMet => l10n.migrationSourceKnownMet,
        MigrationSource.serverMet => l10n.migrationSourceServerMet,
        MigrationSource.nodesDat => l10n.migrationSourceNodesDat,
        MigrationSource.emfriendsMet => l10n.migrationSourceEmfriends,
        MigrationSource.cancelledMet => l10n.migrationSourceCancelled,
        MigrationSource.clientsMet => l10n.migrationSourceClients,
        MigrationSource.identity => l10n.migrationSourceIdentity,
      };

  /// Exécute l'import RPC pour cette source. `incomingDir` n'est utilisé
  /// que par [knownMet] (fichiers à matcher) ; `replaceIdentity` par
  /// [identity].
  Future<Object> run(
    MuleDaemonClient client,
    String dir, {
    String? incomingDir,
    bool replaceIdentity = false,
  }) {
    final sep = Platform.pathSeparator;
    final path = '$dir$sep$fileName';
    switch (this) {
      case MigrationSource.knownMet:
        return client.importKnownMet(
          path,
          directory:
              incomingDir != null && incomingDir.isNotEmpty ? incomingDir : null,
        );
      case MigrationSource.serverMet:
        return client.importServerMet(path);
      case MigrationSource.nodesDat:
        return client.importNodesDat(path);
      case MigrationSource.emfriendsMet:
        return client.importEmfriendsMet(path);
      case MigrationSource.cancelledMet:
        return client.importCancelledMet(path);
      case MigrationSource.clientsMet:
        return client.importClientsMet(path);
      case MigrationSource.identity:
        final alt = '$dir$sep${alternateFileName!}';
        return client.importEmuleIdentity(
          cryptkeyPath: File(path).existsSync() ? path : null,
          preferencesDatPath: File(alt).existsSync() ? alt : null,
          replace: replaceIdentity,
        );
    }
  }

  /// Résumé en une ligne du rapport JSON-RPC renvoyé par [run].
  static String describeResult(
    MigrationSource source,
    Object result,
    AppLocalizations l10n,
  ) {
    if (result is ServerImportResult) {
      return l10n.migrationSummaryImported(result.imported, result.totalCount);
    }
    if (result is Map<String, dynamic>) {
      final parts = <String>[];
      void add(String key, String Function(int) fmt) {
        final v = result[key];
        if (v is num && v > 0) parts.add(fmt(v.toInt()));
      }

      switch (source) {
        case MigrationSource.knownMet:
          add('records', l10n.migrationSummaryRecords);
          add('imported', l10n.migrationSummaryImportedOnly);
          add('enriched', l10n.migrationSummaryEnriched);
          add('unmatched_records', l10n.migrationSummaryUnmatched);
        case MigrationSource.serverMet:
          break; // typé — traité au-dessus
        case MigrationSource.nodesDat:
          add('importedCount', l10n.migrationSummaryImportedOnly);
        case MigrationSource.emfriendsMet:
          add('imported', l10n.migrationSummaryImportedOnly);
          add('updated', l10n.migrationSummaryUpdated);
          add('skipped_no_userhash', l10n.migrationSummarySkipped);
        case MigrationSource.cancelledMet:
          add('records', l10n.migrationSummaryRecords);
          add('total', l10n.migrationSummaryTotal);
        case MigrationSource.clientsMet:
          add('imported', l10n.migrationSummaryImportedOnly);
          add('expired', l10n.migrationSummaryExpired);
          add('skippedNoPubkey', l10n.migrationSummarySkipped);
        case MigrationSource.identity:
          final parts = <String>[
            if (result['keyImported'] == true) l10n.migrationSummaryIdentityKey,
            if (result['userhashImported'] == true)
              l10n.migrationSummaryIdentityHash,
          ];
          return parts.isEmpty
              ? l10n.migrationSummaryEmpty
              : parts.join(' + ');
      }
      if (parts.isEmpty) return l10n.migrationSummaryEmpty;
      return parts.join(' · ');
    }
    return l10n.migrationSummaryEmpty;
  }
}
