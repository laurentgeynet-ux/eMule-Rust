import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:desktop/data/fake_daemon_client.dart';
import 'package:desktop/ui/features/downloads/view_models/downloads_view_model.dart';
import 'package:desktop/ui/features/downloads/widgets/settings_dialog.dart';

void main() {
  group('SettingsDialog Widget Tests', () {
    late FakeMuleDaemonClient fakeClient;
    late DownloadsViewModel viewModel;

    setUp(() async {
      fakeClient = FakeMuleDaemonClient(enableAutoSimulation: false);
      viewModel = DownloadsViewModel(client: fakeClient);
      await viewModel.initialize(url: Uri.parse('ws://fake:4660/rpc'));
    });

    tearDown(() {
      viewModel.dispose();
    });

    testWidgets('renders global download directory input and confinement notice', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SettingsDialog(viewModel: viewModel),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Download Settings & Storage'), findsOneWidget);
      expect(find.text('Global Default Download Directory'), findsOneWidget);
      expect(find.text('Reset to Portable Default'), findsOneWidget);
      expect(find.text('Open Folder'), findsOneWidget);
      expect(find.text('Strict Local Confinement (127.0.0.1)'), findsOneWidget);
      expect(find.textContaining('No incoming or outgoing public internet P2P'), findsOneWidget);
    });

    testWidgets('saving new default directory updates setting and shows success', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SettingsDialog(viewModel: viewModel),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.enterText(find.byType(TextField), 'my_custom_downloads');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      expect(find.text('Default download directory updated successfully.'), findsOneWidget);
      final settings = await fakeClient.getSettings();
      expect(settings['defaultDownloadDir'], 'my_custom_downloads');
    });

    testWidgets('resetting to portable default clears custom directory', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      // Pre-set custom directory
      await fakeClient.setDefaultDownloadDirectory('custom_dir');
      await viewModel.fetchSettings();

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SettingsDialog(viewModel: viewModel),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('Reset to Portable Default'));
      await tester.pumpAndSettle();

      expect(find.text('Reset to portable default directory.'), findsOneWidget);
      final settings = await fakeClient.getSettings();
      expect(settings['isCustom'], false);
    });
  });
}
