import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:desktop/data/fake_daemon_client.dart';
import 'package:desktop/main.dart';
import 'package:desktop/ui/features/downloads/view_models/downloads_view_model.dart';
import 'package:desktop/ui/features/downloads/widgets/downloads_sidebar.dart';
import 'package:desktop/ui/features/downloads/widgets/download_row.dart';

void main() {
  group('Responsive Layout Widget Tests', () {
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

    testWidgets('Large Desktop (1280x800): Full sidebar and full rows rendered', (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Sidebar should be full width (260) and display categories text
      final sidebarFinder = find.byType(DownloadsSidebar);
      expect(sidebarFinder, findsOneWidget);
      expect(tester.getSize(sidebarFinder).width, 260);
      expect(find.text('CATEGORIES'), findsOneWidget);

      // DownloadRow should be in full layout
      expect(find.byType(DownloadRow), findsWidgets);
      expect(tester.takeException(), isNull);
    });

    testWidgets('Medium Screen (900x700): Compact sidebar (width 68) and no overflow', (tester) async {
      tester.view.physicalSize = const Size(900, 700);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Sidebar should be compact width (68)
      final sidebarFinder = find.byType(DownloadsSidebar);
      expect(sidebarFinder, findsOneWidget);
      expect(tester.getSize(sidebarFinder).width, 68);

      // Section titles hidden in compact rail
      expect(find.text('CATEGORIES'), findsNothing);

      // Verify no RenderFlex overflows
      expect(tester.takeException(), isNull);
    });

    testWidgets('Small Screen (600x800): Sidebar in Drawer with hamburger button', (tester) async {
      tester.view.physicalSize = const Size(600, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Sidebar is not directly in the body Row
      expect(find.byType(DownloadsSidebar), findsNothing);

      // Hamburger menu button exists in top bar
      final menuBtn = find.byTooltip('Ouvrir le menu de navigation');
      expect(menuBtn, findsOneWidget);

      // Tap menu button opens the Drawer containing DownloadsSidebar
      await tester.tap(menuBtn);
      await tester.pumpAndSettle();

      expect(find.byType(Drawer), findsOneWidget);
      expect(find.byType(DownloadsSidebar), findsOneWidget);

      // Verify downloads list still renders in compact mode without error
      expect(tester.takeException(), isNull);
    });

    testWidgets('Search and History sections adapt smoothly on small screens', (tester) async {
      tester.view.physicalSize = const Size(640, 750);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Navigate to Search
      viewModel.setSection(AppNavSection.search);
      await tester.pumpAndSettle();
      expect(find.text('Recherche sur le Réseau eD2K'), findsOneWidget);
      expect(tester.takeException(), isNull);

      // Navigate to History
      viewModel.setSection(AppNavSection.history);
      await tester.pumpAndSettle();
      expect(find.text('History & Local Statistics'), findsOneWidget);
      expect(tester.takeException(), isNull);

      // Navigate to Servers
      viewModel.setSection(AppNavSection.servers);
      await tester.pumpAndSettle();
      expect(find.text('Gestion Locale des Serveurs eD2K'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });
}
