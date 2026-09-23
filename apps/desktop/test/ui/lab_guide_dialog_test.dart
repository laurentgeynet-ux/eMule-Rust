// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:app_shell/app/providers.dart';
import 'package:app_shell/ui/features/downloads/widgets/lab_guide_dialog.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';

void main() {
  group('LabGuideDialog Widget Tests', () {
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

    testWidgets('renders all 5 steps of the guided local test flow', (
      WidgetTester tester,
    ) async {
      tester.view.physicalSize = const Size(1280, 900);
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
          home: Scaffold(body: LabGuideDialog()),
        )),
      );
      await tester.pumpAndSettle();

      // Title & subtitle
      expect(find.text('Flux de test loopback local guidé'), findsOneWidget);
      expect(
        find.textContaining('Test eD2K complet de bout en bout sur 127.0.0.1'),
        findsOneWidget,
      );

      // Step cards
      expect(find.text('Générer une fixture de test locale'), findsOneWidget);
      expect(
        find.text('Obtenir le lien eD2K canonique & ajouter aux téléchargements'),
        findsOneWidget,
      );
      expect(find.text('Pair de test loopback (127.0.0.1)'), findsOneWidget);
      expect(find.text('Connecter le téléchargement à la source loopback'), findsOneWidget);
      expect(
        find.text('Nettoyer les données de test (fixtures & parts incomplètes)'),
        findsOneWidget,
      );
    });

    testWidgets('executes complete 5-step local loopback flow interactively', (
      WidgetTester tester,
    ) async {
      tester.view.physicalSize = const Size(1280, 900);
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
          home: Scaffold(body: LabGuideDialog()),
        )),
      );
      await tester.pumpAndSettle();

      // Step 1: Generate Fixture
      expect(find.text('Générer la fixture'), findsOneWidget);
      await tester.tap(find.text('Générer la fixture'));
      await tester.pumpAndSettle();

      expect(
        find.textContaining('lab_sample_1mb.bin (1.0 MB)'),
        findsOneWidget,
      );

      // Step 2: Canonical Link & Add to Downloads
      expect(
        find.textContaining('ed2k://|file|lab_sample_1mb.bin|1048576|'),
        findsOneWidget,
      );
      final addBtn = find.widgetWithText(FilledButton, 'Ajouter aux téléchargements');
      expect(addBtn, findsOneWidget);
      await tester.ensureVisible(addBtn);
      await tester.tap(addBtn);
      await tester.pumpAndSettle();

      expect(find.text('Ajouté aux téléchargements !'), findsOneWidget);
      expect(find.text('Tâche de téléchargement créée dans l\'UI'), findsOneWidget);

      // Step 3: Start Loopback Test Peer
      final startPeerBtn = find.widgetWithText(
        FilledButton,
        'Démarrer le pair loopback',
      );
      expect(startPeerBtn, findsOneWidget);
      await tester.ensureVisible(startPeerBtn);
      await tester.tap(startPeerBtn);
      await tester.pumpAndSettle();

      expect(find.text('Arrêter le pair loopback'), findsOneWidget);
      expect(find.textContaining('Actif sur 127.0.0.1:39735'), findsOneWidget);

      // Step 4: Connect Download to Source
      final connectBtn = find.widgetWithText(FilledButton, 'Connecter à la source');
      expect(connectBtn, findsOneWidget);
      await tester.ensureVisible(connectBtn);
      await tester.tap(connectBtn);
      await tester.pumpAndSettle();

      // Step 5: Cleanup Test Data (opens confirmation dialog)
      final cleanupBtn = find.widgetWithText(
        OutlinedButton,
        'Nettoyer les données de test',
      );
      expect(cleanupBtn, findsOneWidget);
      await tester.ensureVisible(cleanupBtn);
      await tester.tap(cleanupBtn);
      await tester.pumpAndSettle();

      expect(find.text('Confirmer le nettoyage'), findsOneWidget);
      await tester.tap(find.text('Supprimer les données de test'));
      await tester.pumpAndSettle();

      // State is cleaned up
      expect(find.text('Démarrer le pair loopback'), findsOneWidget);
    });
  });
}
