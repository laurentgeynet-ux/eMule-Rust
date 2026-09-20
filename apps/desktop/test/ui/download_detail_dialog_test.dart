import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:app_shell/ui/features/downloads/widgets/download_detail_dialog.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';

void main() {
  group('DownloadDetailDialog Widget Tests', () {
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

    testWidgets('renders all metadata, part map, paths, and action bar', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 1000);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      const targetId = '00000000-0000-0000-0000-000000000001';

      await tester.pumpWidget(
        MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: DownloadDetailDialog(
              downloadId: targetId,
              viewModel: viewModel,
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // Header
      expect(find.text('Debian_12_netinst.iso'), findsWidgets);
      expect(find.text('Downloading'), findsWidgets);

      // Identity & eD2K Link
      expect(find.text('Identity & eD2K Links'), findsOneWidget);
      expect(find.text('Download UUID'), findsOneWidget);
      expect(find.text(targetId), findsOneWidget);
      expect(find.text('eD2K Hash (MD4)'), findsOneWidget);
      expect(find.text('31d6cfe0d16ae931b73c59d7e0c089c0'), findsOneWidget);
      expect(find.text('Canonical eD2K Link'), findsOneWidget);
      expect(find.textContaining('ed2k://|file|Debian_12_netinst.iso|654311424|31d6cfe0d16ae931b73c59d7e0c089c0|/'), findsOneWidget);

      // Transfer metrics
      expect(find.text('Transfer Metrics & Speed'), findsOneWidget);
      expect(find.text('Received'), findsOneWidget);
      expect(find.text('Verified'), findsOneWidget);
      expect(find.text('Current Rate'), findsOneWidget);
      expect(find.text('Average Rate'), findsOneWidget);

      // Part Map
      expect(find.textContaining('eD2K Part Map'), findsOneWidget);
      expect(find.textContaining('Verified ('), findsOneWidget);
      expect(find.textContaining('Completed ('), findsOneWidget);
      expect(find.textContaining('Downloading ('), findsOneWidget);
      expect(find.textContaining('Missing ('), findsOneWidget);

      // Paths
      expect(find.text('Storage & Target Destination'), findsOneWidget);
      expect(find.text('Temporary Part Path (Confined)'), findsOneWidget);
      expect(find.text('data/temp/00000000-0000-0000-0000-000000000001.part'), findsOneWidget);
      expect(find.text('Final Target Destination'), findsOneWidget);
      expect(find.text('downloads/Debian_12_netinst.iso'), findsOneWidget);
      expect(find.text('Change Destination'), findsOneWidget);

      // Action Bar
      expect(find.text('Pause'), findsOneWidget);
      expect(find.text('Retry Loopback Source'), findsOneWidget);
      expect(find.text('Open Completed File'), findsOneWidget);
      expect(find.text('Remove...'), findsOneWidget);
    });

    testWidgets('copy redacted diagnostic generates safe sanitized output without secrets', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 1000);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

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

      const targetId = '00000000-0000-0000-0000-000000000001';

      await tester.pumpWidget(
        MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: DownloadDetailDialog(
              downloadId: targetId,
              viewModel: viewModel,
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      final copyBtn = find.text('Copy Redacted Diagnostic');
      await tester.ensureVisible(copyBtn);
      await tester.pumpAndSettle();
      await tester.tap(copyBtn);
      await tester.pump();

      expect(copiedText, isNotNull);
      expect(copiedText, contains('=== Emule-Sion Download Diagnostic (Sanitized) ==='));
      expect(copiedText, contains('Download ID: 00000000-0000-0000-0000-000000000001'));
      expect(copiedText, contains('File Name: Debian_12_netinst.iso'));
      expect(copiedText, contains('eD2K Hash: 31d6cfe0d16ae931b73c59d7e0c089c0'));
      expect(copiedText, contains('Canonical Link: ed2k://|file|Debian_12_netinst.iso|654311424|31d6cfe0d16ae931b73c59d7e0c089c0|/'));
      expect(copiedText, contains('Confinement: Loopback 127.0.0.1'));
      expect(copiedText, contains('[DOWNLOAD_DIR]'));
      expect(copiedText, contains('data/temp/00000000-0000-0000-0000-000000000001.part'));

      // Strict security: NO tokens or secrets
      expect(copiedText!.toLowerCase().contains('auth.token'), isFalse);
      expect(copiedText!.toLowerCase().contains('token:'), isFalse);
      expect(copiedText!.toLowerCase().contains('bearer'), isFalse);
    });

    testWidgets('changing destination prompts dialog and updates destination', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 1000);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      const targetId = '00000000-0000-0000-0000-000000000001';

      await tester.pumpWidget(
        MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: DownloadDetailDialog(
              downloadId: targetId,
              viewModel: viewModel,
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      final changeBtn = find.text('Change Destination');
      await tester.ensureVisible(changeBtn);
      await tester.pumpAndSettle();
      await tester.tap(changeBtn);
      await tester.pumpAndSettle();

      expect(find.text('Change Target Destination'), findsOneWidget);
      expect(find.text('Save Destination'), findsOneWidget);

      await tester.enterText(
        find.widgetWithText(TextField, 'downloads'),
        'custom_downloads',
      );
      await tester.tap(find.text('Save Destination'));
      await tester.pumpAndSettle();

      expect(find.text('custom_downloads/Debian_12_netinst.iso'), findsOneWidget);
    });

    testWidgets('remove download button opens isolated confirmation dialog', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 1000);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      const targetId = '00000000-0000-0000-0000-000000000001';

      await tester.pumpWidget(
        MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: DownloadDetailDialog(
              downloadId: targetId,
              viewModel: viewModel,
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      final removeBtn = find.text('Remove...');
      await tester.ensureVisible(removeBtn);
      await tester.pumpAndSettle();
      await tester.tap(removeBtn);
      await tester.pumpAndSettle();

      expect(find.text('Remove Download?'), findsOneWidget);
      expect(find.textContaining('SQLite will be permanently deleted'), findsOneWidget);
      expect(find.text('Confirm Remove'), findsOneWidget);
    });
  });
}
