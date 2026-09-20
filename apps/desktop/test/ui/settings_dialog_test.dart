import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:desktop/app/app_controller.dart';
import 'package:desktop/ui/features/downloads/widgets/settings_dialog.dart';
import 'package:desktop/l10n/generated/app_localizations.dart';

void main() {
  group('SettingsDialog Widget Tests', () {
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

    testWidgets('renders global download directory input and confinement notice', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        ProviderScope(
          child: MaterialApp(
            locale: const Locale('fr'),
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: Scaffold(
              body: SettingsDialog(viewModel: viewModel),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Download Settings & Storage'), findsOneWidget);
      expect(find.text('Global Default Download Directory'), findsOneWidget);
      expect(find.text('Reset to Portable Default'), findsOneWidget);
      expect(find.text('Open Folder'), findsOneWidget);
      expect(find.textContaining('Mode Réseau Public Ouvert (WAN & Kad)'), findsOneWidget);
      expect(find.textContaining('Emule-Sion est connecté aux réseaux eD2K et Kademlia mondiaux'), findsOneWidget);
    });

    testWidgets('saving new default directory updates setting and shows success', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        ProviderScope(
          child: MaterialApp(
            locale: const Locale('fr'),
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: Scaffold(
              body: SettingsDialog(viewModel: viewModel),
            ),
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
        ProviderScope(
          child: MaterialApp(
            locale: const Locale('fr'),
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: Scaffold(
              body: SettingsDialog(viewModel: viewModel),
            ),
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
