import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:desktop/data/fake_daemon_client.dart';
import 'package:desktop/main.dart';
import 'package:desktop/ui/features/downloads/view_models/downloads_view_model.dart';
import 'package:desktop/ui/features/downloads/widgets/download_row.dart';
import 'package:desktop/ui/features/downloads/widgets/downloads_sidebar.dart';

void main() {
  group('DownloadsScreen Widget Tests', () {
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

    testWidgets('renders sidebar with branding and download items', (WidgetTester tester) async {
      // Set desktop window size
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Brand and sidebar
      expect(find.text('Emule-Sion'), findsOneWidget);
      expect(find.text('All Downloads'), findsAtLeastNWidgets(1));
      expect(
        find.descendant(
          of: find.byType(DownloadsSidebar),
          matching: find.text('Active'),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: find.byType(DownloadsSidebar),
          matching: find.text('Paused'),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: find.byType(DownloadsSidebar),
          matching: find.text('Completed'),
        ),
        findsOneWidget,
      );

      // Seeded items
      expect(find.text('Debian_12_netinst.iso'), findsOneWidget);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsOneWidget);
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);
    });

    testWidgets('filters list by status category', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Click on 'Paused' filter in sidebar
      await tester.tap(
        find.descendant(
          of: find.byType(DownloadsSidebar),
          matching: find.text('Paused'),
        ),
      );
      await tester.pumpAndSettle();

      // Only blender is paused
      expect(find.text('Blender_Benchmark_Scene.blend'), findsOneWidget);
      expect(find.text('Debian_12_netinst.iso'), findsNothing);
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsNothing);

      // Click on 'Completed' filter in sidebar
      await tester.tap(
        find.descendant(
          of: find.byType(DownloadsSidebar),
          matching: find.text('Completed'),
        ),
      );
      await tester.pumpAndSettle();

      // Only Linux Kernel is completed
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsNothing);

      // Back to All
      await tester.tap(find.text('All Downloads').first);
      await tester.pumpAndSettle();
      expect(find.byType(DownloadRow), findsNWidgets(3));
    });

    testWidgets('search filters list by filename', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Type "debian" in search field
      await tester.enterText(find.byType(TextField).first, 'debian');
      await tester.pumpAndSettle();

      expect(find.text('Debian_12_netinst.iso'), findsOneWidget);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsNothing);
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsNothing);
    });

    testWidgets('selecting item reveals details panel', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Details panel should not be visible initially
      expect(find.textContaining('Details:'), findsNothing);

      // Tap on Debian row
      await tester.tap(find.text('Debian_12_netinst.iso'));
      await tester.pumpAndSettle();

      // Details panel is now shown
      expect(find.text('Details: Debian_12_netinst.iso'), findsOneWidget);
      expect(find.text('Download ID'), findsOneWidget);
    });

    testWidgets('add eD2K link dialog validates and adds download', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Tap "+ Add Link"
      await tester.tap(find.widgetWithText(FilledButton, 'Add Link'));
      await tester.pumpAndSettle();

      expect(find.text('Add eD2K Download'), findsOneWidget);

      // Find the eD2K link text field inside dialog
      final dialogTextField = find.descendant(
        of: find.byType(Dialog),
        matching: find.byType(TextField),
      ).first;

      // Enter invalid link
      await tester.enterText(dialogTextField, 'invalid_link');
      await tester.pumpAndSettle();

      expect(find.text('Must start with "ed2k://|file|"'), findsOneWidget);

      // Enter valid link
      const validLink =
          'ed2k://|file|OpenSourceGame.iso|4294967296|0123456789abcdef0123456789abcdef|/';
      await tester.enterText(dialogTextField, validLink);
      await tester.pumpAndSettle();

      // Preview card shows parsed information
      expect(find.text('OpenSourceGame.iso'), findsOneWidget);
      expect(find.text('4.00 GB'), findsOneWidget);

      // Submit
      final submitButton = find.widgetWithText(FilledButton, 'Add Download');
      await tester.tap(submitButton);
      await tester.pumpAndSettle();

      // Dialog is dismissed and item is in the downloads list!
      expect(find.byType(Dialog), findsNothing);
      expect(find.text('OpenSourceGame.iso'), findsOneWidget);
      expect(find.byType(DownloadRow), findsNWidgets(4));
    });
  });
}
