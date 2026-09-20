import 'package:mule_client/mule_client.dart';
import 'package:desktop/app/app_controller.dart';
import 'package:desktop/l10n/generated/app_localizations.dart';
import 'package:desktop/ui/features/downloads/views/downloads_screen.dart';
import 'package:desktop/ui/features/downloads/widgets/ip_filter_dialog.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('IP filter button opens dialog and displays tabs', (WidgetTester tester) async {
    final client = FakeMuleDaemonClient(enableAutoSimulation: false);
    final viewModel = AppController(client: client, isDevToolsAvailable: true);

    await tester.binding.setSurfaceSize(const Size(1280, 800));

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: DownloadsScreen(viewModel: viewModel),
        ),
      ),
    );
    await tester.pumpAndSettle();

    // Verify button exists
    final filterBtn = find.byKey(const Key('ip_filter_button'));
    expect(filterBtn, findsOneWidget);

    // Tap button to open dialog
    await tester.tap(filterBtn);
    await tester.pumpAndSettle();

    // Verify dialog content
    expect(find.byType(IpFilterDialog), findsOneWidget);
    expect(find.text('IP Filter & Rate Limiters'), findsOneWidget);
    expect(find.text('IP Filter'), findsOneWidget);
    expect(find.text('Rate Limits'), findsOneWidget);

    // Switch to Rate Limits tab
    await tester.tap(find.text('Rate Limits'));
    await tester.pumpAndSettle();

    expect(find.text('Bandwidth & Token Bucket Quotas'), findsOneWidget);
    expect(find.text('Save Limits'), findsOneWidget);
  });
}
