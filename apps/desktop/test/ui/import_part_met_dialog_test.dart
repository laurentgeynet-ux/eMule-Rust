// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:app_shell/app/providers.dart';
import 'package:app_shell/ui/features/downloads/widgets/import_part_met_dialog.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';

void main() {
  group('ImportPartMetDialog Widget Tests', () {
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

    testWidgets('renders input fields, security notice, and inspect button',
        (WidgetTester tester) async {
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
        child: MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: ImportPartMetDialog(),
          ),
        )),
      );
      await tester.pumpAndSettle();

      // Title and security banner
      expect(find.text('Importer un .part.met (expérimental)'), findsOneWidget);
      expect(
        find.textContaining('Le téléchargement sera préparé dans data/temp/ et initialisé en pause'),
        findsOneWidget,
      );

      // Input labels
      expect(find.text('Chemin du fichier .part.met'), findsOneWidget);
      expect(find.text('Chemin du fichier .part associé (optionnel, auto-détecté)'), findsOneWidget);
      expect(find.text('Dossier de destination du téléchargement'), findsOneWidget);
      expect(find.text('Catégorie (optionnel)'), findsOneWidget);

      // Action buttons
      expect(find.text('Annuler'), findsOneWidget);
      expect(find.byKey(const Key('inspect_button')), findsOneWidget);
      expect(find.byKey(const Key('import_submit_button')), findsOneWidget);
    });

    testWidgets('inspects .part.met and displays pre-import metadata preview',
        (WidgetTester tester) async {
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
        child: MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: ImportPartMetDialog(),
          ),
        )),
      );
      await tester.pumpAndSettle();

      // Find met path textfield
      final textFields = find.byType(TextField);
      final metField = textFields.at(0);

      // Enter mock .part.met path
      await tester.enterText(metField, 'C:\\eMule\\001.part.met');
      await tester.pumpAndSettle();

      // Inspect button should be enabled
      final inspectButton = find.byKey(const Key('inspect_button'));
      await tester.tap(inspectButton);
      await tester.pumpAndSettle();

      // Inspection card should appear with parsed information
      expect(find.byIcon(Icons.check_circle_outline), findsOneWidget);
      expect(find.text('001.bin'), findsOneWidget); // Parsed file name
      expect(find.text('31d6cfe0d16ae931b73c59d7e0c089c0'), findsOneWidget); // Hash
      expect(find.text('9.3 MB'), findsWidgets); // Size
      expect(find.text('1 partie(s)'), findsOneWidget); // Part count
    });

    testWidgets('executes import and adds download strictly in paused state',
        (WidgetTester tester) async {
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
        child: MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: ImportPartMetDialog(),
          ),
        )),
      );
      await tester.pumpAndSettle();

      // Enter path and inspect
      final textFields = find.byType(TextField);
      await tester.enterText(textFields.at(0), 'C:\\eMule\\archive.part.met');
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('inspect_button')));
      await tester.pumpAndSettle();

      // Submit import
      final importButton = find.byKey(const Key('import_submit_button'));
      expect(tester.widget<ElevatedButton>(importButton).onPressed, isNotNull);

      await tester.tap(importButton);
      await tester.pumpAndSettle();

      // Verify that audit report view is shown after successful import
      expect(find.byKey(const Key('import_audit_report_text')), findsOneWidget);
      expect(find.textContaining('Format Version: 0xE0'), findsOneWidget);
      expect(find.textContaining('STRICTLY PAUSED'), findsOneWidget);
      expect(find.byKey(const Key('copy_report_button')), findsOneWidget);

      // Tap Copy Report button
      await tester.tap(find.byKey(const Key('copy_report_button')));
      await tester.pumpAndSettle();

      // Tap Done button to dismiss dialog
      final doneButton = find.byKey(const Key('finish_import_button'));
      expect(doneButton, findsOneWidget);
      await tester.tap(doneButton);
      await tester.pumpAndSettle();

      // Verify that viewModel received the imported download
      final imported = viewModel.filteredDownloads.firstWhere(
        (d) => d.fileName == 'archive.bin',
      );
      expect(imported, isNotNull);
      expect(imported.status, DownloadStatus.paused);
      expect(imported.sourceCount, 0);
      expect(imported.tempPath, contains('data/temp/'));
      expect(viewModel.statusNotification, contains('"archive.bin" importé'));
    });
  });
}
