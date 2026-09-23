// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/emule_sion_app.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:app_shell/ui/features/downloads/widgets/downloads_sidebar.dart';
import 'package:app_shell/ui/features/downloads/widgets/download_row.dart';

void main() {
  group('Responsive Layout Widget Tests', () {
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

    testWidgets('Large Desktop (1280x800): Full sidebar and full rows rendered', (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Sidebar should be full width (260) and display categories text
      final sidebarFinder = find.byType(DownloadsSidebar);
      expect(sidebarFinder, findsOneWidget);
      expect(tester.getSize(sidebarFinder).width, 260);
      expect(find.text('CATÉGORIES'), findsOneWidget);

      // DownloadRow should be in full layout
      expect(find.byType(DownloadRow), findsWidgets);
      expect(tester.takeException(), isNull);
    });

    testWidgets('Medium Screen (900x700): Compact sidebar (width 68) and no overflow', (tester) async {
      tester.view.physicalSize = const Size(900, 700);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Sidebar should be compact width (68)
      final sidebarFinder = find.byType(DownloadsSidebar);
      expect(sidebarFinder, findsOneWidget);
      expect(tester.getSize(sidebarFinder).width, 68);

      // Section titles hidden in compact rail
      expect(find.text('CATÉGORIES'), findsNothing);

      // Verify no RenderFlex overflows
      expect(tester.takeException(), isNull);
    });

    testWidgets('Small Screen (600x800): Sidebar in Drawer with hamburger button', (tester) async {
      tester.view.physicalSize = const Size(600, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Sidebar is not directly in the body Row
      expect(find.byType(DownloadsSidebar), findsNothing);

      // Hamburger menu button exists in top bar
      final menuBtn = find.byTooltip('Ouvrir le menu de navigation');
      expect(menuBtn, findsOneWidget);

      // Tap menu button opens the Drawer containing DownloadsSidebar
      await tester.tap(menuBtn);
      await tester.pumpAndSettle();

      expect(find.byType(Drawer), findsOneWidget);
      expect(find.byType(DownloadsSidebar), findsOneWidget);

      // Verify downloads list still renders in compact mode without error
      expect(tester.takeException(), isNull);
    });

    testWidgets('Search and History sections adapt smoothly on small screens', (tester) async {
      tester.view.physicalSize = const Size(640, 750);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Navigate to Search
      viewModel.setSection(AppNavSection.search);
      await tester.pumpAndSettle();
      expect(find.text('Recherche sur le Réseau eD2K'), findsOneWidget);
      expect(tester.takeException(), isNull);

      // Navigate to History
      viewModel.setSection(AppNavSection.history);
      await tester.pumpAndSettle();
      expect(find.text('Historique & statistiques locales'), findsOneWidget);
      expect(tester.takeException(), isNull);

      // Navigate to Servers
      viewModel.setSection(AppNavSection.servers);
      await tester.pumpAndSettle();
      expect(find.text('Gestion Locale des Serveurs eD2K'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('ServersView renders cards on mobile and adaptive table on medium/large', (tester) async {
      await viewModel.importServerMet('/mock/path/server.met');

      // Test Large Screen (1280x800) -> Full table with Description & Version
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      viewModel.setSection(AppNavSection.servers);
      await tester.pumpAndSettle();

      expect(find.text('eDonkeyServer No2'), findsOneWidget);
      expect(find.text('Description'), findsOneWidget);
      expect(find.text('Version'), findsOneWidget);
      expect(tester.takeException(), isNull);

      // Test Medium Screen (850x700) -> Medium table without Description/Version to prevent overflow
      tester.view.physicalSize = const Size(850, 700);
      await tester.pumpAndSettle();

      expect(find.text('eDonkeyServer No2'), findsOneWidget);
      expect(find.text('État / Filtre'), findsOneWidget);
      expect(tester.takeException(), isNull);

      // Test Small Screen (550x800) -> Responsive cards list
      tester.view.physicalSize = const Size(550, 800);
      await tester.pumpAndSettle();

      expect(find.text('eDonkeyServer No2'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });
}
