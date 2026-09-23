// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/emule_sion_app.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:app_shell/ui/features/history/views/history_view.dart';
import 'package:app_shell/ui/features/settings/views/settings_view.dart';

void main() {
  group('HistoryView & Statistics Tests', () {
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

    testWidgets('navigates to History & Statistics section and displays dashboard',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Verify sidebar contains History & Statistics button
      final historyNav = find.text('Historique & Statistiques');
      expect(historyNav, findsOneWidget);

      // The nav item may sit below the sidebar fold — scroll it into view.
      await tester.ensureVisible(historyNav);
      await tester.pumpAndSettle();

      // Tap to navigate to History & Statistics
      await tester.tap(historyNav);
      await tester.pumpAndSettle();

      // Verify HistoryView is rendered
      expect(find.byType(HistoryView), findsOneWidget);
      expect(find.text('Historique & statistiques locales'), findsOneWidget);

      // Verify dashboard statistics cards
      expect(find.text('Total terminés'), findsOneWidget);
      expect(find.text("Fenêtres d'activité"), findsOneWidget);
      expect(find.text('Reprises & tentatives'), findsOneWidget);
      expect(find.text('Intégrité'), findsOneWidget);

      // Verify seeded history entries are listed
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);
      expect(find.text('COMPLETED'), findsOneWidget);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsOneWidget);
      expect(find.text('RESUMED'), findsOneWidget);
    });

    testWidgets('filters history entries by event type', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Navigate to History & Statistics (may be below the sidebar fold)
      await tester.ensureVisible(find.text('Historique & Statistiques'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Historique & Statistiques'));
      await tester.pumpAndSettle();

      // Initially both items are visible
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsOneWidget);

      // Select 'Completed' from filter dropdown
      await tester.tap(find.text('Tous les événements'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Terminés').last);
      await tester.pumpAndSettle();

      // Now only completed entry is visible
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsNothing);
    });

    testWidgets('shows sanitized export dialog', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Navigate to History & Statistics (may be below the sidebar fold)
      await tester.ensureVisible(find.text('Historique & Statistiques'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Historique & Statistiques'));
      await tester.pumpAndSettle();

      // Tap Export Report dropdown
      await tester.tap(find.text('Exporter le rapport'));
      await tester.pumpAndSettle();

      // Tap Export as JSON
      await tester.tap(find.text('Exporter en JSON (anonymisé)'));
      await tester.pumpAndSettle();

      // Verify export dialog opens with sanitized notice
      expect(find.text('Exporter l\'historique (JSON)'), findsOneWidget);
      expect(
        find.text('Le rapport est anonymisé : tous les noms d\'utilisateur et chemins locaux sont masqués.'),
        findsOneWidget,
      );
      expect(find.text('Copier dans le presse-papiers'), findsOneWidget);

      // Close dialog
      await tester.tap(find.text('Fermer'));
      await tester.pumpAndSettle();
      expect(find.text('Exporter l\'historique (JSON)'), findsNothing);
    });

    testWidgets('clears history after destructive confirmation', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Navigate to History & Statistics (may be below the sidebar fold)
      await tester.ensureVisible(find.text('Historique & Statistiques'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Historique & Statistiques'));
      await tester.pumpAndSettle();

      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);

      // Tap Clear History
      await tester.tap(find.text('Effacer l\'historique'));
      await tester.pumpAndSettle();

      // Verify confirmation dialog
      expect(find.text('Effacer l\'historique des téléchargements ?'), findsOneWidget);

      // Tap Clear All History confirm button
      await tester.tap(find.text('Effacer tout l\'historique'));
      await tester.pumpAndSettle();

      // Verify empty state is now displayed
      expect(find.text('Aucun historique de téléchargement enregistré'), findsOneWidget);
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsNothing);
    });

    testWidgets('settings page displays history retention policy options',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Navigate to the /settings page
      final settingsFinder = find.text('Paramètres');
      await tester.ensureVisible(settingsFinder);
      await tester.pumpAndSettle();
      await tester.tap(settingsFinder);
      await tester.pumpAndSettle();

      expect(find.byType(SettingsView), findsOneWidget);
      expect(find.text("Rétention de l'historique"), findsOneWidget);
      expect(find.text('30 jours (défaut)'), findsOneWidget);

      // Change retention policy to 7 Days
      await tester.ensureVisible(find.text('30 jours (défaut)'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('30 jours (défaut)'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('7 jours').last);
      await tester.pumpAndSettle();

      expect(viewModel.historyRetentionDays, 7);
    });
  });
}
