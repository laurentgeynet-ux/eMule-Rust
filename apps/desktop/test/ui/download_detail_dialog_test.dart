// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:app_shell/app/providers.dart';
import 'package:app_shell/ui/features/downloads/widgets/download_detail_dialog.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';

void main() {
  group('DownloadDetailDialog Widget Tests', () {
    late FakeMuleDaemonClient fakeClient;
    late AppController viewModel;

    setUp(() async {
      fakeClient = FakeMuleDaemonClient(enableAutoSimulation: false);
      viewModel = AppController(client: fakeClient);
      await viewModel.initialize(url: Uri.parse('ws://fake:4660/rpc'));
    });

    tearDown(() {
      viewModel.dispose();
    });

    testWidgets('renders all metadata, part map, paths, and action bar', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 1000);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      const targetId = '00000000-0000-0000-0000-000000000001';

      await tester.pumpWidget(
        ProviderScope(
        overrides: [
          appControllerProvider.overrideWith((ref) => viewModel),
          devToolsEnabledProvider
              .overrideWithValue(viewModel.isDevToolsAvailable),
          daemonSupervisorProvider
              .overrideWithValue(viewModel.supervisor),
          muleDaemonClientProvider.overrideWithValue(viewModel.client),
        ],
        child: MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: DownloadDetailDialog(
              downloadId: targetId,
              ),
          ),
        )),
      );
      await tester.pumpAndSettle();

      // Header
      expect(find.text('Debian_12_netinst.iso'), findsWidgets);
      expect(find.text('Téléchargement'), findsWidgets);

      // Identity & eD2K Link
      expect(find.text('Identité & liens eD2K'), findsOneWidget);
      expect(find.text('UUID du téléchargement'), findsOneWidget);
      expect(find.text(targetId), findsOneWidget);
      expect(find.text('Hash eD2K (MD4)'), findsOneWidget);
      expect(find.text('31d6cfe0d16ae931b73c59d7e0c089c0'), findsOneWidget);
      expect(find.text('Lien eD2K canonique'), findsOneWidget);
      expect(find.textContaining('ed2k://|file|Debian_12_netinst.iso|654311424|31d6cfe0d16ae931b73c59d7e0c089c0|/'), findsOneWidget);

      // Transfer metrics
      expect(find.text('Métriques de transfert & vitesse'), findsOneWidget);
      expect(find.text('Reçu'), findsOneWidget);
      expect(find.text('Vérifié'), findsOneWidget);
      expect(find.text('Débit actuel'), findsOneWidget);
      expect(find.text('Débit moyen'), findsOneWidget);

      // Part Map
      expect(find.textContaining('Carte des parts eD2K'), findsOneWidget);
      expect(find.textContaining('Vérifiées ('), findsOneWidget);
      expect(find.textContaining('Terminées ('), findsOneWidget);
      expect(find.textContaining('En téléchargement ('), findsOneWidget);
      expect(find.textContaining('Manquantes ('), findsOneWidget);

      // Paths
      expect(find.text('Stockage & destination'), findsOneWidget);
      expect(find.text('Chemin .part temporaire (confiné)'), findsOneWidget);
      expect(find.text('data/temp/00000000-0000-0000-0000-000000000001.part'), findsOneWidget);
      expect(find.text('Destination finale'), findsOneWidget);
      expect(find.text('downloads/Debian_12_netinst.iso'), findsOneWidget);
      expect(find.text('Changer la destination'), findsOneWidget);

      // Action Bar
      expect(find.text('Pause'), findsOneWidget);
      expect(find.text('Réessayer la source loopback'), findsOneWidget);
      expect(find.text('Ouvrir le fichier terminé'), findsOneWidget);
      expect(find.text('Supprimer...'), findsOneWidget);
    });

    testWidgets('copy redacted diagnostic generates safe sanitized output without secrets', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 1000);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      String? copiedText;
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (MethodCall methodCall) async {
          if (methodCall.method == 'Clipboard.setData') {
            copiedText = (methodCall.arguments as Map)['text'] as String?;
          }
          return null;
        },
      );

      const targetId = '00000000-0000-0000-0000-000000000001';

      await tester.pumpWidget(
        ProviderScope(
        overrides: [
          appControllerProvider.overrideWith((ref) => viewModel),
          devToolsEnabledProvider
              .overrideWithValue(viewModel.isDevToolsAvailable),
          daemonSupervisorProvider
              .overrideWithValue(viewModel.supervisor),
          muleDaemonClientProvider.overrideWithValue(viewModel.client),
        ],
        child: MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: DownloadDetailDialog(
              downloadId: targetId,
              ),
          ),
        )),
      );
      await tester.pumpAndSettle();

      final copyBtn = find.text('Copier le diagnostic anonymisé');
      await tester.ensureVisible(copyBtn);
      await tester.pumpAndSettle();
      await tester.tap(copyBtn);
      await tester.pump();

      expect(copiedText, isNotNull);
      expect(copiedText, contains('=== EMule-Rust Download Diagnostic (Sanitized) ==='));
      expect(copiedText, contains('Download ID: 00000000-0000-0000-0000-000000000001'));
      expect(copiedText, contains('File Name: Debian_12_netinst.iso'));
      expect(copiedText, contains('eD2K Hash: 31d6cfe0d16ae931b73c59d7e0c089c0'));
      expect(copiedText, contains('Canonical Link: ed2k://|file|Debian_12_netinst.iso|654311424|31d6cfe0d16ae931b73c59d7e0c089c0|/'));
      expect(copiedText, contains('Confinement: Loopback 127.0.0.1'));
      expect(copiedText, contains('[DOWNLOAD_DIR]'));
      expect(copiedText, contains('data/temp/00000000-0000-0000-0000-000000000001.part'));

      // Strict security: NO tokens or secrets
      expect(copiedText!.toLowerCase().contains('auth.token'), isFalse);
      expect(copiedText!.toLowerCase().contains('token:'), isFalse);
      expect(copiedText!.toLowerCase().contains('bearer'), isFalse);
    });

    testWidgets('changing destination prompts dialog and updates destination', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 1000);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      const targetId = '00000000-0000-0000-0000-000000000001';

      await tester.pumpWidget(
        ProviderScope(
        overrides: [
          appControllerProvider.overrideWith((ref) => viewModel),
          devToolsEnabledProvider
              .overrideWithValue(viewModel.isDevToolsAvailable),
          daemonSupervisorProvider
              .overrideWithValue(viewModel.supervisor),
          muleDaemonClientProvider.overrideWithValue(viewModel.client),
        ],
        child: MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: DownloadDetailDialog(
              downloadId: targetId,
              ),
          ),
        )),
      );
      await tester.pumpAndSettle();

      final changeBtn = find.text('Changer la destination');
      await tester.ensureVisible(changeBtn);
      await tester.pumpAndSettle();
      await tester.tap(changeBtn);
      await tester.pumpAndSettle();

      expect(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.text('Changer la destination'),
        ),
        findsOneWidget,
      );
      expect(find.text('Enregistrer la destination'), findsOneWidget);

      await tester.enterText(
        find.widgetWithText(TextField, 'downloads'),
        'custom_downloads',
      );
      await tester.tap(find.text('Enregistrer la destination'));
      await tester.pumpAndSettle();

      expect(find.text('custom_downloads/Debian_12_netinst.iso'), findsOneWidget);
    });

    testWidgets('remove download button opens isolated confirmation dialog', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 1000);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      const targetId = '00000000-0000-0000-0000-000000000001';

      await tester.pumpWidget(
        ProviderScope(
        overrides: [
          appControllerProvider.overrideWith((ref) => viewModel),
          devToolsEnabledProvider
              .overrideWithValue(viewModel.isDevToolsAvailable),
          daemonSupervisorProvider
              .overrideWithValue(viewModel.supervisor),
          muleDaemonClientProvider.overrideWithValue(viewModel.client),
        ],
        child: MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: DownloadDetailDialog(
              downloadId: targetId,
              ),
          ),
        )),
      );
      await tester.pumpAndSettle();

      final removeBtn = find.text('Supprimer...');
      await tester.ensureVisible(removeBtn);
      await tester.pumpAndSettle();
      await tester.tap(removeBtn);
      await tester.pumpAndSettle();

      expect(find.text('Supprimer le téléchargement ?'), findsOneWidget);
      expect(find.textContaining('SQLite sera définitivement supprimée'), findsOneWidget);
      expect(find.text('Confirmer la suppression'), findsOneWidget);
      // En cours : les fichiers temporaires sont supprimés automatiquement —
      // aucune case à cocher ne doit être proposée.
      expect(find.byType(CheckboxListTile), findsNothing);
    });

    testWidgets('completed download removal proposes deleting the shared file', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 1000);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      // Item 003 is seeded as DownloadStatus.completed in the fake client.
      const targetId = '00000000-0000-0000-0000-000000000003';

      await tester.pumpWidget(
        ProviderScope(
        overrides: [
          appControllerProvider.overrideWith((ref) => viewModel),
          devToolsEnabledProvider
              .overrideWithValue(viewModel.isDevToolsAvailable),
          daemonSupervisorProvider
              .overrideWithValue(viewModel.supervisor),
          muleDaemonClientProvider.overrideWithValue(viewModel.client),
        ],
        child: MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: DownloadDetailDialog(
              downloadId: targetId,
              ),
          ),
        )),
      );
      await tester.pumpAndSettle();

      final removeBtn = find.text('Supprimer...');
      await tester.ensureVisible(removeBtn);
      await tester.pumpAndSettle();
      await tester.tap(removeBtn);
      await tester.pumpAndSettle();

      expect(find.text('Supprimer le téléchargement ?'), findsOneWidget);
      expect(find.byType(CheckboxListTile), findsOneWidget);
      expect(
        find.text('Supprimer aussi le fichier partagé dans le dossier de destination'),
        findsOneWidget,
      );
      // Option décochée par défaut : le fichier final n'est supprimé que si
      // l'utilisateur la coche explicitement.
      final tile = tester.widget<CheckboxListTile>(find.byType(CheckboxListTile));
      expect(tile.value, isFalse);
    });
  });
}
