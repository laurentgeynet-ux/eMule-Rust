import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:desktop/app/app_controller.dart';
import 'package:desktop/l10n/generated/app_localizations.dart';
import 'package:desktop/ui/features/downloads/views/downloads_screen.dart';

void main() {
  group('Network Policy & Kill Switch UI Tests', () {
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

    testWidgets('top bar displays network policy badge and kill switch button',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: DownloadsScreen(viewModel: viewModel),
        ),
      );
      await tester.pumpAndSettle();

      // Verify network badge is visible with initial public-ed2k-kad-preview profile
      expect(find.byKey(const Key('network_policy_badge')), findsOneWidget);
      expect(find.text('public-ed2k-kad-preview'), findsOneWidget);

      // Verify kill switch button is visible
      expect(find.byKey(const Key('network_kill_switch_button')), findsOneWidget);
    });

    testWidgets('kill switch button engages and toggles network state',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: DownloadsScreen(viewModel: viewModel),
        ),
      );
      await tester.pumpAndSettle();

      expect(viewModel.isNetworkKillSwitchActive, isFalse);

      // Tap kill switch button
      await tester.tap(find.byKey(const Key('network_kill_switch_button')));
      await tester.pumpAndSettle();

      // State is now disabled
      expect(viewModel.isNetworkKillSwitchActive, isTrue);
      expect(find.text('Disabled'), findsOneWidget);

      // Tap kill switch button again to re-enable
      await tester.tap(find.byKey(const Key('network_kill_switch_button')));
      await tester.pumpAndSettle();

      // State is restored to preview-active
      expect(viewModel.isNetworkKillSwitchActive, isFalse);
      expect(find.text('public-ed2k-kad-preview'), findsOneWidget);
    });

    testWidgets('network policy dialog allows inspecting budgets and switching profile',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(
        MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: DownloadsScreen(viewModel: viewModel),
        ),
      );
      await tester.pumpAndSettle();

      // Open NetworkPolicyDialog by tapping the badge
      await tester.tap(find.byKey(const Key('network_policy_badge')));
      await tester.pumpAndSettle();

      // Verify dialog title and quotas
      expect(find.text('Network Policy & Defense Guard'), findsOneWidget);
      expect(find.text('Active Connectivity Profile'), findsOneWidget);
      expect(find.text('Active Connections & Limits'), findsOneWidget);
      expect(find.text('Peer Sockets'), findsOneWidget);
      expect(find.text('Server Sockets'), findsOneWidget);
      expect(find.text('Listeners'), findsOneWidget);

      // Initial profile is public-ed2k-kad-preview
      expect(viewModel.currentNetworkProfile, NetworkProfile.publicEd2kKadPreview);

      // Switch profile to public-ed2k-preview
      await tester.tap(find.byKey(const Key('profile_public-ed2k-preview')));
      await tester.pumpAndSettle();

      expect(viewModel.currentNetworkProfile, NetworkProfile.publicEd2kPreview);

      // Switch profile to local-lab
      await tester.tap(find.byKey(const Key('profile_local-lab')));
      await tester.pumpAndSettle();

      expect(viewModel.currentNetworkProfile, NetworkProfile.localLab);

      // Tap Disconnect All inside dialog
      await tester.tap(find.byKey(const Key('kill_switch_dialog_button')));
      await tester.pumpAndSettle();

      expect(viewModel.isNetworkKillSwitchActive, isTrue);
      expect(find.byKey(const Key('reenable_network_button')), findsOneWidget);

      // Re-enable inside dialog
      await tester.tap(find.byKey(const Key('reenable_network_button')));
      await tester.pumpAndSettle();

      expect(viewModel.isNetworkKillSwitchActive, isFalse);
    });
  });
}
