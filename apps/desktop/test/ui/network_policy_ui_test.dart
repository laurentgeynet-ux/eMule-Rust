import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:app_shell/app/providers.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';
import 'package:app_shell/ui/features/downloads/views/downloads_screen.dart';

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
          home: DownloadsScreen(),
        )),
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
          home: DownloadsScreen(),
        )),
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

    testWidgets('settings Connection section allows inspecting budgets and switching profile',
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
          home: DownloadsScreen(),
        )),
      );
      await tester.pumpAndSettle();

      // The badge navigates to the Settings page — Connection section
      // (standalone mode falls back to the section switch on General).
      await tester.tap(find.byKey(const Key('network_policy_badge')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Connexion'));
      await tester.pumpAndSettle();

      // Verify section header and quotas
      expect(find.text('Profil de connectivité actif'), findsOneWidget);
      expect(find.text('Connexions actives & limites'), findsOneWidget);
      expect(find.text('Sockets pairs'), findsOneWidget);
      expect(find.text('Sockets serveur'), findsOneWidget);
      expect(find.text("Listeners d'écoute"), findsOneWidget);

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

      // Tap Disconnect All inside dialog — a confirmation dialog now
      // intercepts the action (the kill switch cuts every socket).
      await tester.tap(find.byKey(const Key('kill_switch_dialog_button')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Tout déconnecter'));
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
