// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:io';

import 'package:app_shell/app/app_controller.dart';
import 'package:app_shell/app/providers.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';
import 'package:app_shell/ui/features/settings/widgets/migration/emule_migration_dialog.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';

void main() {
  group('EmuleMigrationDialog', () {
    late FakeMuleDaemonClient client;
    late AppController viewModel;
    late Directory configDir;

    setUp(() async {
      client = FakeMuleDaemonClient(enableAutoSimulation: false);
      viewModel = AppController(client: client);
      await viewModel.initialize(url: Uri.parse('ws://fake:4660/rpc'));
      configDir = _fakeEmuleConfig([
        'emfriends.met',
        'cancelled.met',
        'clients.met',
        'cryptkey.dat',
        'preferences.dat',
      ]);
    });

    tearDown(() {
      viewModel.dispose();
      configDir.deleteSync(recursive: true);
    });

    Future<void> pumpDialog(WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 1000);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

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
          child: const MaterialApp(
            locale: Locale('fr'),
            localizationsDelegates:
                AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: Scaffold(body: EmuleMigrationDialog()),
          ),
        ),
      );
      await tester.pumpAndSettle();
    }

    testWidgets('détecte les fichiers eMule du dossier config',
        (tester) async {
      await pumpDialog(tester);

      expect(find.text('Migration depuis eMule'), findsOneWidget);
      expect(find.text('Aucun fichier eMule détecté dans ce dossier'),
          findsNothing);

      // Saisie du dossier → détection automatique.
      await tester.enterText(
          find.byType(TextField).first, configDir.path);
      await tester.pumpAndSettle();

      expect(find.textContaining('emfriends.met'), findsOneWidget);
      expect(find.textContaining('cancelled.met'), findsOneWidget);
      expect(find.textContaining('clients.met'), findsOneWidget);
      expect(find.textContaining('cryptkey.dat'), findsOneWidget);
      // Absents du dossier factice.
      expect(find.textContaining('server.met'), findsNothing);
      expect(find.textContaining('nodes.dat'), findsNothing);
    });

    testWidgets('aucune source dans un dossier vide', (tester) async {
      await pumpDialog(tester);
      final emptyDir = Directory.systemTemp.createTempSync('emule_empty');
      addTearDown(() => emptyDir.deleteSync(recursive: true));

      await tester.enterText(
          find.byType(TextField).first, emptyDir.path);
      await tester.pumpAndSettle();

      expect(find.text('Aucun fichier eMule détecté dans ce dossier'),
          findsOneWidget);
      final runButton = tester.widget<FilledButton>(
          find.widgetWithText(FilledButton, 'Importer la sélection'));
      expect(runButton.onPressed, isNull);
    });

    testWidgets('importe les sources cochées et affiche les rapports',
        (tester) async {
      await pumpDialog(tester);
      await tester.enterText(
          find.byType(TextField).first, configDir.path);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Tout sélectionner'));
      await tester.pumpAndSettle();

      // L'identité est cochée → avertissement + case « remplacer ».
      expect(find.text("Remplacer l'identité actuelle"), findsOneWidget);
      await tester.tap(find.text("Remplacer l'identité actuelle"));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Importer la sélection'));
      await tester.pumpAndSettle();

      // Rapports factices du FakeMuleDaemonClient.
      expect(find.text('1 importés · 1 mis à jour'), findsOneWidget);
      expect(find.text('5 entrées · 12 au total'), findsOneWidget);
      expect(find.text('3 importés · 1 ignorés'), findsOneWidget);
      expect(find.text('clé RSA + hash utilisateur'), findsOneWidget);
    });

    testWidgets("signale l'erreur RPC sur la source concernée",
        (tester) async {
      await pumpDialog(tester);
      await tester.enterText(
          find.byType(TextField).first, configDir.path);
      await tester.pumpAndSettle();

      // Coche uniquement emfriends.met (première source détectée dans
      // l'ordre de l'enum : emfriends, cancelled, clients, identity).
      await tester.tap(find.byType(Checkbox).first);
      await tester.pumpAndSettle();

      client.failNextMigrationImport = true;
      await tester.tap(find.text('Importer la sélection'));
      await tester.pumpAndSettle();

      expect(find.textContaining('fake emfriends.met failure'),
          findsOneWidget);
    });
  });
}

Directory _fakeEmuleConfig(List<String> fileNames) {
  final dir = Directory.systemTemp.createTempSync('emule_cfg');
  for (final name in fileNames) {
    File('${dir.path}${Platform.pathSeparator}$name')
        .writeAsBytesSync([0x0E]);
  }
  return dir;
}
