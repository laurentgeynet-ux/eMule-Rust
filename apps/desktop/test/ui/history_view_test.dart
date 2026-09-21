import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/emule_sion_app.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:app_shell/ui/features/downloads/widgets/settings_dialog.dart';
import 'package:app_shell/ui/features/history/views/history_view.dart';

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

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Verify sidebar contains History & Statistics button
      final historyNav = find.text('History & Statistics');
      expect(historyNav, findsOneWidget);

      // The nav item may sit below the sidebar fold — scroll it into view.
      await tester.ensureVisible(historyNav);
      await tester.pumpAndSettle();

      // Tap to navigate to History & Statistics
      await tester.tap(historyNav);
      await tester.pumpAndSettle();

      // Verify HistoryView is rendered
      expect(find.byType(HistoryView), findsOneWidget);
      expect(find.text('History & Local Statistics'), findsOneWidget);

      // Verify dashboard statistics cards
      expect(find.text('Total Completed'), findsOneWidget);
      expect(find.text('Activity Windows'), findsOneWidget);
      expect(find.text('Resumes & Retries'), findsOneWidget);
      expect(find.text('Integrity Health'), findsOneWidget);

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

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Navigate to History & Statistics (may be below the sidebar fold)
      await tester.ensureVisible(find.text('History & Statistics'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('History & Statistics'));
      await tester.pumpAndSettle();

      // Initially both items are visible
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsOneWidget);

      // Select 'Completed' from filter dropdown
      await tester.tap(find.text('All Events'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Completed').last);
      await tester.pumpAndSettle();

      // Now only completed entry is visible
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsNothing);
    });

    testWidgets('shows sanitized export dialog', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Navigate to History & Statistics (may be below the sidebar fold)
      await tester.ensureVisible(find.text('History & Statistics'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('History & Statistics'));
      await tester.pumpAndSettle();

      // Tap Export Report dropdown
      await tester.tap(find.text('Export Report'));
      await tester.pumpAndSettle();

      // Tap Export as JSON
      await tester.tap(find.text('Export as JSON (Sanitized)'));
      await tester.pumpAndSettle();

      // Verify export dialog opens with sanitized notice
      expect(find.text('Export History (JSON)'), findsOneWidget);
      expect(
        find.text('Report is sanitized: all local usernames and filesystem paths are redacted.'),
        findsOneWidget,
      );
      expect(find.text('Copy to Clipboard'), findsOneWidget);

      // Close dialog
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      expect(find.text('Export History (JSON)'), findsNothing);
    });

    testWidgets('clears history after destructive confirmation', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Navigate to History & Statistics (may be below the sidebar fold)
      await tester.ensureVisible(find.text('History & Statistics'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('History & Statistics'));
      await tester.pumpAndSettle();

      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);

      // Tap Clear History
      await tester.tap(find.text('Clear History'));
      await tester.pumpAndSettle();

      // Verify confirmation dialog
      expect(find.text('Clear Download History?'), findsOneWidget);

      // Tap Clear All History confirm button
      await tester.tap(find.text('Clear All History'));
      await tester.pumpAndSettle();

      // Verify empty state is now displayed
      expect(find.text('No download history recorded yet'), findsOneWidget);
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsNothing);
    });

    testWidgets('settings dialog displays history retention policy options',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Open settings dialog
      final settingsFinder = find.text('Settings');
      await tester.ensureVisible(settingsFinder);
      await tester.pumpAndSettle();
      await tester.tap(settingsFinder);
      await tester.pumpAndSettle();

      expect(find.byType(SettingsDialog), findsOneWidget);
      expect(find.text('History Retention Policy'), findsOneWidget);
      expect(find.text('30 Days (Default)'), findsOneWidget);

      // Change retention policy to 7 Days
      await tester.tap(find.text('30 Days (Default)'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('7 Days').last);
      await tester.pumpAndSettle();

      expect(viewModel.historyRetentionDays, 7);

      // Close dialog
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
    });
  });
}
