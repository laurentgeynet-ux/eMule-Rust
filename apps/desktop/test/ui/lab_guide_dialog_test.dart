import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/app/app_controller.dart';
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
        MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(body: LabGuideDialog(viewModel: viewModel)),
        ),
      );
      await tester.pumpAndSettle();

      // Title & subtitle
      expect(find.text('Guided Local Loopback Test Flow'), findsOneWidget);
      expect(
        find.textContaining('Full end-to-end eD2K testing on 127.0.0.1'),
        findsOneWidget,
      );

      // Step cards
      expect(find.text('Generate Local Test Fixture'), findsOneWidget);
      expect(
        find.text('Obtain Canonical eD2K Link & Add to Downloads'),
        findsOneWidget,
      );
      expect(find.text('Loopback Test Peer (127.0.0.1)'), findsOneWidget);
      expect(find.text('Connect Download to Loopback Source'), findsOneWidget);
      expect(
        find.text('Clean Test Data (Fixtures & Incomplete Parts)'),
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
        MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(body: LabGuideDialog(viewModel: viewModel)),
        ),
      );
      await tester.pumpAndSettle();

      // Step 1: Generate Fixture
      expect(find.text('Generate Fixture'), findsOneWidget);
      await tester.tap(find.text('Generate Fixture'));
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
      final addBtn = find.widgetWithText(FilledButton, 'Add to Downloads');
      expect(addBtn, findsOneWidget);
      await tester.ensureVisible(addBtn);
      await tester.tap(addBtn);
      await tester.pumpAndSettle();

      expect(find.text('Added to Downloads!'), findsOneWidget);
      expect(find.text('Download task created in UI'), findsOneWidget);

      // Step 3: Start Loopback Test Peer
      final startPeerBtn = find.widgetWithText(
        FilledButton,
        'Start Loopback Peer',
      );
      expect(startPeerBtn, findsOneWidget);
      await tester.ensureVisible(startPeerBtn);
      await tester.tap(startPeerBtn);
      await tester.pumpAndSettle();

      expect(find.text('Stop Loopback Peer'), findsOneWidget);
      expect(find.textContaining('Active on 127.0.0.1:39735'), findsOneWidget);

      // Step 4: Connect Download to Source
      final connectBtn = find.widgetWithText(FilledButton, 'Connect to Source');
      expect(connectBtn, findsOneWidget);
      await tester.ensureVisible(connectBtn);
      await tester.tap(connectBtn);
      await tester.pumpAndSettle();

      // Step 5: Cleanup Test Data (opens confirmation dialog)
      final cleanupBtn = find.widgetWithText(
        OutlinedButton,
        'Cleanup Test Data',
      );
      expect(cleanupBtn, findsOneWidget);
      await tester.ensureVisible(cleanupBtn);
      await tester.tap(cleanupBtn);
      await tester.pumpAndSettle();

      expect(find.text('Confirm Cleanup'), findsOneWidget);
      await tester.tap(find.text('Delete Test Data'));
      await tester.pumpAndSettle();

      // State is cleaned up
      expect(find.text('Start Loopback Peer'), findsOneWidget);
    });
  });
}
