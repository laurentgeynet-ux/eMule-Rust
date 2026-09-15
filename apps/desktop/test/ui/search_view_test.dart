import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:desktop/data/fake_daemon_client.dart';
import 'package:desktop/main.dart';
import 'package:desktop/ui/features/downloads/view_models/downloads_view_model.dart';
import 'package:desktop/ui/features/search/views/search_screen.dart';

void main() {
  group('SearchView UI Tests', () {
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

    testWidgets('navigates to Recherche eD2K, shows disconnected warning when not connected',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Find Recherche eD2K in sidebar
      final searchNav = find.text('Recherche eD2K');
      expect(searchNav, findsOneWidget);

      await tester.ensureVisible(searchNav);
      await tester.pumpAndSettle();
      await tester.tap(searchNav);
      await tester.pumpAndSettle();

      // Verify SearchView is rendered
      expect(find.byType(SearchView), findsOneWidget);
      expect(find.text('Recherche sur le Réseau eD2K'), findsOneWidget);

      // Verify disconnected warning banner is present
      expect(
        find.textContaining('pas connecté à un serveur'),
        findsOneWidget,
      );
      expect(find.text('Aller aux Serveurs'), findsOneWidget);
    });

    testWidgets('connects to server, executes search, displays results and downloads file',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Connect to a server first
      await viewModel.connectServer('195.154.52.200:4661');
      await tester.pumpAndSettle();

      // Navigate to search
      final searchNav = find.text('Recherche eD2K');
      await tester.ensureVisible(searchNav);
      await tester.pumpAndSettle();
      await tester.tap(searchNav);
      await tester.pumpAndSettle();

      // Verify connected banner is shown
      expect(find.textContaining('Connecté à'), findsOneWidget);

      // Enter search text
      final searchInput = find.byType(TextField).first;
      await tester.enterText(searchInput, 'debian');
      await tester.pumpAndSettle();

      // Tap Rechercher button
      final searchButton = find.widgetWithText(ElevatedButton, 'Rechercher');
      expect(searchButton, findsOneWidget);
      await tester.tap(searchButton);
      await tester.pumpAndSettle();

      // Verify search results are displayed
      expect(find.text('debian-release.iso'), findsOneWidget);
      expect(find.text('debian-documentation.pdf'), findsOneWidget);

      // Click "Télécharger" on the first result
      final downloadButtons = find.widgetWithText(ElevatedButton, 'Télécharger');
      expect(downloadButtons, findsAtLeastNWidgets(1));
      await tester.ensureVisible(downloadButtons.first);
      await tester.pumpAndSettle();
      await tester.tap(downloadButtons.first);
      await tester.pumpAndSettle();

      // Verify notification / download added
      expect(viewModel.statusNotification, contains('Added'));
    });

    testWidgets('clearing search results resets to empty state',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Connect and search
      await viewModel.connectServer('195.154.52.200:4661');
      await viewModel.searchServer(term: 'debian');
      viewModel.setSection(AppNavSection.search);
      await tester.pumpAndSettle();

      expect(find.text('debian-release.iso'), findsOneWidget);

      // Tap clear results
      final clearButton = find.widgetWithText(OutlinedButton, 'Effacer les résultats');
      expect(clearButton, findsOneWidget);
      await tester.tap(clearButton);
      await tester.pumpAndSettle();

      expect(viewModel.searchResults, isEmpty);
      expect(find.textContaining('Entrez des mots-clés ci-dessus'), findsOneWidget);
    });
  });
}
