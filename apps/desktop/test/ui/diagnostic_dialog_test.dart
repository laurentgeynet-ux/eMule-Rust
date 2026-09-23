// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:app_shell/app/providers.dart';
import 'package:app_shell/ui/features/downloads/widgets/diagnostic_dialog.dart';
import 'package:app_shell/ui/features/downloads/widgets/lab_guide_dialog.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';

void main() {
  group('DiagnosticDialog Widget Tests', () {
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

    testWidgets('renders all diagnostic parameters and loopback confinement notice',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
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
            body: DiagnosticDialog(),
          ),
        )),
      );
      await tester.pumpAndSettle();

      // Dialog title
      expect(find.text('Diagnostics système & lab'), findsOneWidget);

      // Network Profile / Confinement card
      expect(find.text('Profil réseau : eD2K + Kad publics (preview)'), findsOneWidget);
      expect(find.textContaining('Connexions WAN autorisées'), findsOneWidget);

      // Workdir mode & path
      expect(find.text('Portable'), findsOneWidget);
      expect(find.text(r'C:\Fake\Portable\data'), findsOneWidget);

      // Process & RPC endpoint
      expect(find.text('ws://127.0.0.1:4660/rpc'), findsOneWidget);
      expect(find.textContaining('v0.1.0-fake'), findsOneWidget);

      // Actions
      expect(find.text('Copier version partageable (anonymisée)'), findsOneWidget);
      expect(find.text('Copier version complète (dev local)'), findsOneWidget);
      expect(find.text('Ouvrir le guide lab'), findsOneWidget);
      expect(find.text('Fermer'), findsOneWidget);
    });

    testWidgets('tapping Open Lab Guide opens LabGuideDialog', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
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
            body: Builder(
              builder: (context) => ElevatedButton(
                onPressed: () {
                  showDialog(
                    context: context,
                    builder: (ctx) => DiagnosticDialog(),
                  );
                },
                child: const Text('Open Diag'),
              ),
            ),
          ),
        )),
      );
      await tester.pumpAndSettle();

      // Open diagnostic dialog
      await tester.tap(find.text('Open Diag'));
      await tester.pumpAndSettle();

      expect(find.byType(DiagnosticDialog), findsOneWidget);

      // Tap Open Lab Guide
      await tester.tap(find.text('Ouvrir le guide lab'));
      await tester.pumpAndSettle();

      // DiagnosticDialog dismissed and LabGuideDialog opened
      expect(find.byType(DiagnosticDialog), findsNothing);
      expect(find.byType(LabGuideDialog), findsOneWidget);
    });

    testWidgets('tapping Copy Full (Local Dev) copies complete diagnostics without auth token leakage', (WidgetTester tester) async {
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
            body: DiagnosticDialog(),
          ),
        )),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('Copier version complète (dev local)'));
      await tester.pump();

      expect(copiedText, isNotNull);
      expect(copiedText, contains('=== EMule-Rust Diagnostics (Full Local) ==='));
      expect(copiedText, contains('Workdir Mode: Portable'));
      expect(copiedText, contains(r'Workdir Path: C:\Fake\Portable\data'));
      expect(copiedText, contains('RPC Endpoint: ws://127.0.0.1:4660/rpc'));
      expect(copiedText, contains('Security: Auth token strictly redacted (never exported)'));

      // Strict security assertion: ZERO auth tokens, ZERO token secrets
      expect(copiedText!.toLowerCase().contains('auth.token'), isFalse);
      expect(copiedText!.toLowerCase().contains('token:'), isFalse);
      expect(copiedText!.toLowerCase().contains('bearer'), isFalse);
    });

    testWidgets('tapping inline Copy button on Path row copies raw path to clipboard', (WidgetTester tester) async {
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
            body: DiagnosticDialog(),
          ),
        )),
      );
      await tester.pumpAndSettle();

      final copyPathFinder = find.byTooltip('Copier le chemin complet');
      expect(copyPathFinder, findsOneWidget);

      await tester.tap(copyPathFinder);
      await tester.pump();

      expect(copiedText, equals(r'C:\Fake\Portable\data'));
    });
  });
}
