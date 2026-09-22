import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/emule_sion_app.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:app_shell/ui/features/servers/views/servers_screen.dart';

void main() {
  group('ServersView UI Tests', () {
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

    testWidgets('navigates to Serveurs eD2K section, displays safety banner and empty state',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Find Serveurs eD2K in sidebar
      final serversNav = find.text('Serveurs eD2K');
      expect(serversNav, findsOneWidget);

      // Ensure visible and tap to navigate to Serveurs
      await tester.ensureVisible(serversNav);
      await tester.pumpAndSettle();
      await tester.tap(serversNav);
      await tester.pumpAndSettle();

      // Verify ServersView is rendered
      expect(find.byType(ServersView), findsOneWidget);
      expect(find.text('Gestion Locale des Serveurs eD2K'), findsOneWidget);

      // Verify safety banner is present
      expect(
        find.textContaining('filtre IP'),
        findsOneWidget,
      );

      // Verify empty state when no servers loaded
      expect(find.textContaining('Aucun serveur eD2K'), findsOneWidget);
    });

    testWidgets('imports servers and displays server table with filter badges and actions',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Navigate to servers
      final serversNav = find.text('Serveurs eD2K');
      await tester.ensureVisible(serversNav);
      await tester.pumpAndSettle();
      await tester.tap(serversNav);
      await tester.pumpAndSettle();

      // Trigger import via viewModel directly (simulating import completion)
      await viewModel.importServerMet('/mock/path/server.met');
      await tester.pumpAndSettle();

      // Table should now display 2 servers
      expect(find.text('eDonkeyServer No2'), findsOneWidget);
      expect(find.text('176.103.48.36:4184'), findsOneWidget);
      expect(find.textContaining('Autorisé'), findsWidgets);

      expect(find.text('Filtered Rouge Server'), findsOneWidget);
      expect(find.text('10.20.30.40:4661'), findsOneWidget);
      expect(find.textContaining('Bloqué'), findsWidgets);

      // Toggle server enable/disable on the first server
      final pauseBtn = find.byIcon(Icons.pause_rounded).first;
      await tester.ensureVisible(pauseBtn);
      await tester.pumpAndSettle();
      await tester.tap(pauseBtn);
      await tester.pumpAndSettle();

      // Should now be disabled
      expect(find.textContaining('Désactivé'), findsOneWidget);

      // Delete server
      final deleteBtn = find.byIcon(Icons.delete_outline).first;
      await tester.tap(deleteBtn);
      await tester.pumpAndSettle();

      // Should now only have 1 server left
      expect(viewModel.servers.length, 1);
    });

    testWidgets('allows sorting servers by Name, IP:Port, and Ping',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      final serversNav = find.text('Serveurs eD2K');
      await tester.ensureVisible(serversNav);
      await tester.pumpAndSettle();
      await tester.tap(serversNav);
      await tester.pumpAndSettle();

      await viewModel.importServerMet('/mock/path/server.met');
      await tester.pumpAndSettle();

      // Tap 'Nom du Serveur' column header
      final nameCol = find.text('Nom du Serveur');
      await tester.tap(nameCol);
      await tester.pumpAndSettle();

      // Tap 'IP : Port' column header
      final addrCol = find.text('IP : Port');
      await tester.tap(addrCol);
      await tester.pumpAndSettle();

      // Tap 'Ping' column header
      final pingCol = find.text('Ping');
      await tester.tap(pingCol, warnIfMissed: false);
      await tester.pumpAndSettle();

      expect(find.text('eDonkeyServer No2'), findsOneWidget);
    });
  });
}
