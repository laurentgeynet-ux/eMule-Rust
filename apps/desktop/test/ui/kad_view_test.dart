import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:desktop/data/fake_daemon_client.dart';
import 'package:desktop/ui/features/downloads/view_models/downloads_view_model.dart';
import 'package:desktop/ui/features/kad/views/kad_screen.dart';

void main() {
  group('KadView UI Tests (Phase 12.0a)', () {
    late FakeMuleDaemonClient client;
    late DownloadsViewModel viewModel;

    setUp(() async {
      client = FakeMuleDaemonClient(enableAutoSimulation: false);
      viewModel = DownloadsViewModel(client: client);
      await viewModel.initialize();
    });

    tearDown(() {
      viewModel.dispose();
    });

    testWidgets('renders Kad DHT safety banner and metrics cards', (
      tester,
    ) async {
      await viewModel.fetchKadStatus();
      await viewModel.fetchKadContacts();

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: KadView(viewModel: viewModel)),
        ),
      );
      await tester.pumpAndSettle();

      // Check title and safety banner
      expect(find.text('Réseau Kad DHT (Kademlia)'), findsOneWidget);
      expect(find.textContaining('Transport UDP Kad'), findsOneWidget);

      // Check status cards and local ID
      expect(find.text('ID Kad Local :'), findsOneWidget);
      expect(find.text('deadbeefcafe0123456789abcdef0123'), findsOneWidget);
      expect(find.text('Contacts actifs: '), findsOneWidget);
      expect(
        find.widgetWithText(FilledButton, 'Démarrer UDP Kad'),
        findsOneWidget,
      );

      // Check contacts table
      expect(find.text('Kad ID (128-bit XOR)'), findsOneWidget);
      expect(find.textContaining('127.0.0.1:39737'), findsOneWidget);
      expect(find.text('v2'), findsOneWidget);
    });

    testWidgets('filter contacts search input works', (tester) async {
      await viewModel.fetchKadStatus();
      await viewModel.fetchKadContacts();

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: KadView(viewModel: viewModel)),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.textContaining('127.0.0.1:39737'), findsOneWidget);

      // Enter search filter that does not match
      await tester.enterText(find.byType(TextField), 'nonexistent_ip');
      await tester.pumpAndSettle();

      expect(find.textContaining('127.0.0.1:39737'), findsNothing);
      expect(find.text('Aucun contact Kad'), findsOneWidget);
    });
  });
}
