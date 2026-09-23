// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:mule_client/mule_client.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:app_shell/app/providers.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';
import 'package:app_shell/ui/features/downloads/views/downloads_screen.dart';
import 'package:app_shell/ui/features/settings/views/settings_view.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('IP filter button opens the Security settings section',
      (WidgetTester tester) async {
    final client = FakeMuleDaemonClient(enableAutoSimulation: false);
    final viewModel = AppController(client: client, isDevToolsAvailable: true);

    await tester.binding.setSurfaceSize(const Size(1280, 800));

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
        home: Scaffold(
          body: DownloadsScreen(),
        ),
      )),
    );
    await tester.pumpAndSettle();

    // Verify button exists
    final filterBtn = find.byKey(const Key('ip_filter_button'));
    expect(filterBtn, findsOneWidget);

    // The top bar is horizontally scrollable — longer French labels can
    // push the button out of the viewport, so scroll it into view first.
    await tester.ensureVisible(filterBtn);
    await tester.pumpAndSettle();

    // Tap button → navigates to the Settings page (standalone mode falls
    // back to the section switch, opening the General section).
    await tester.tap(filterBtn);
    await tester.pumpAndSettle();

    expect(find.byType(SettingsView), findsOneWidget);

    // Select the Security rail section — IP filter + rate limits are
    // stacked in the same scrollable pane (no tabs anymore).
    await tester.tap(find.text('Sécurité'));
    await tester.pumpAndSettle();

    expect(find.text('Importer un ipfilter.dat'), findsOneWidget);
    expect(find.text('Limites de débit (Token Buckets)'), findsOneWidget);
    expect(find.text('Enregistrer les limites'), findsOneWidget);
  });
}
