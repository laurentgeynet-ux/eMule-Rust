// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/emulerust_app.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:app_shell/ui/features/servers/views/servers_screen.dart';

void main() {
  group('Server Connection Tests', () {
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

    testWidgets('navigates to Serveurs eD2K section and displays empty state',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      final serversNav = find.text('Serveurs eD2K');
      expect(serversNav, findsOneWidget);

      await tester.ensureVisible(serversNav);
      await tester.pumpAndSettle();
      await tester.tap(serversNav);
      await tester.pumpAndSettle();

      expect(find.byType(ServersView), findsOneWidget);
      expect(find.text('Gestion Locale des Serveurs eD2K'), findsOneWidget);
      expect(find.textContaining('Aucun serveur eD2K'), findsOneWidget);
    });

    testWidgets('imports servers and displays server table with filter badges and actions',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(eMuleRustApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      final serversNav = find.text('Serveurs eD2K');
      await tester.ensureVisible(serversNav);
      await tester.pumpAndSettle();
      await tester.tap(serversNav);
      await tester.pumpAndSettle();

      await viewModel.importServerMet('/mock/path/server.met');
      await tester.pumpAndSettle();

      expect(find.text('eDonkeyServer No2'), findsOneWidget);
      expect(find.text('176.103.48.36:4184'), findsOneWidget);
      expect(find.textContaining('Autorisé'), findsWidgets);
      expect(find.text('Filtered Rouge Server'), findsOneWidget);
      expect(find.text('10.20.30.40:4661'), findsOneWidget);
      expect(find.textContaining('Bloqué'), findsOneWidget);

      final pauseBtn = find.byIcon(Icons.pause_rounded).first;
      await tester.ensureVisible(pauseBtn);
      await tester.pumpAndSettle();
      await tester.tap(pauseBtn);
      await tester.pumpAndSettle();
      expect(find.textContaining('Désactivé'), findsOneWidget);

      final deleteBtn = find.byIcon(Icons.delete_outline).first;
      await tester.tap(deleteBtn);
      await tester.pumpAndSettle();
      expect(viewModel.servers.length, 1);
    });

    // ── Phase 10.1: Connection Tests ─────────────────────────────────────────

    test('session bar shows disconnected state initially', () {
      expect(viewModel.serverSession.status,
          ServerConnectionStatus.disconnected);
      expect(viewModel.isServerConnecting, isFalse);
    });

    test('connectServer updates serverSession to connected', () async {
      await viewModel.importServerMet('/mock/path/server.met');
      final allowed = viewModel.servers
          .where((s) => s.filterStatus == ServerFilterStatus.allowed)
          .first;

      await viewModel.connectServer(allowed.address);

      expect(viewModel.serverSession.status, ServerConnectionStatus.connected);
      expect(viewModel.serverSession.serverAddress, allowed.address);
      expect(viewModel.serverSession.isHighId, isTrue);
      expect(viewModel.isServerConnecting, isFalse);
    });

    test('disconnectServer resets serverSession to disconnected', () async {
      await viewModel.importServerMet('/mock/path/server.met');
      final allowed = viewModel.servers
          .where((s) => s.filterStatus == ServerFilterStatus.allowed)
          .first;

      await viewModel.connectServer(allowed.address);
      expect(viewModel.serverSession.status, ServerConnectionStatus.connected);

      await viewModel.disconnectServer();
      expect(
          viewModel.serverSession.status, ServerConnectionStatus.disconnected);
    });

    test('disconnectAllNetwork resets server session (kill switch)', () async {
      await viewModel.importServerMet('/mock/path/server.met');
      final allowed = viewModel.servers
          .where((s) => s.filterStatus == ServerFilterStatus.allowed)
          .first;

      await viewModel.connectServer(allowed.address);
      expect(viewModel.serverSession.status, ServerConnectionStatus.connected);

      await viewModel.disconnectAllNetwork();
      expect(
          viewModel.serverSession.status, ServerConnectionStatus.disconnected);
    });

    test('sequential connect-disconnect-reconnect works correctly', () async {
      await viewModel.importServerMet('/mock/path/server.met');
      final allowed = viewModel.servers
          .where((s) => s.filterStatus == ServerFilterStatus.allowed)
          .first;

      await viewModel.connectServer(allowed.address);
      await viewModel.disconnectServer();
      await viewModel.connectServer(allowed.address);

      expect(viewModel.serverSession.status, ServerConnectionStatus.connected);
      expect(viewModel.isServerConnecting, isFalse);
    });

    test('refresh recovers a session already connected daemon-side', () async {
      // Cas web : le daemon s'est connecté au serveur eD2K avant que l'UI
      // ne souscrive — l'événement sessionUpdated a été perdu. Le client fake
      // est connecté directement pour simuler cet état sans émettre d'event.
      await fakeClient.connectServer('176.123.5.89:4725');
      expect(viewModel.serverSession.status,
          ServerConnectionStatus.disconnected);

      await viewModel.refresh();

      expect(viewModel.serverSession.status, ServerConnectionStatus.connected);
      expect(viewModel.serverSession.serverAddress, '176.123.5.89:4725');
      expect(viewModel.serverSession.isHighId, isTrue);
    });
  });
}
