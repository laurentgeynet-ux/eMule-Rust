// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/app/providers.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:app_shell/ui/features/kad/views/kad_screen.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';

void main() {
  group('KadView UI Tests (Phase 12.0a)', () {
    late FakeMuleDaemonClient client;
    late AppController viewModel;

    setUp(() async {
      client = FakeMuleDaemonClient(enableAutoSimulation: false);
      viewModel = AppController(client: client);
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
        ProviderScope(
          overrides: [
            appControllerProvider.overrideWith((ref) => viewModel, disposeNotifier: false),
          ],
          child: MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
            home: Scaffold(body: const KadView()),
          ),
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
      expect(find.text('ID Kad (XOR 128 bits)'), findsOneWidget);
      expect(find.textContaining('127.0.0.1:39737'), findsOneWidget);
      expect(find.text('v2'), findsOneWidget);
    });

    testWidgets('filter contacts search input works', (tester) async {
      await viewModel.fetchKadStatus();
      await viewModel.fetchKadContacts();

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            appControllerProvider.overrideWith((ref) => viewModel, disposeNotifier: false),
          ],
          child: MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
            home: Scaffold(body: const KadView()),
          ),
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
