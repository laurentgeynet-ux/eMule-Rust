import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:ui_kit/ui_kit.dart';

import 'app/app_controller.dart';
import 'app/providers.dart';
import 'app/router.dart';
import 'core/daemon_lifecycle.dart';
import 'l10n/generated/app_localizations.dart';

/// Root application widget shared by desktop, mobile and web entry points.
class EmuleSionApp extends StatefulWidget {
  final AppController viewModel;
  final DaemonLifecycle? supervisor;

  const EmuleSionApp({
    super.key,
    required this.viewModel,
    this.supervisor,
  });

  @override
  State<EmuleSionApp> createState() => _EmuleSionAppState();
}

class _EmuleSionAppState extends State<EmuleSionApp> {
  AppLifecycleListener? _lifecycleListener;
  late final GoRouter _router = createAppRouter(widget.viewModel);

  @override
  void initState() {
    super.initState();
    _lifecycleListener = AppLifecycleListener(
      onExitRequested: _handleExitRequested,
    );
  }

  Future<AppExitResponse> _handleExitRequested() async {
    if (widget.supervisor != null) {
      await widget.supervisor!.stop();
    }
    return AppExitResponse.exit;
  }

  @override
  void dispose() {
    _lifecycleListener?.dispose();
    _router.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ProviderScope(
      overrides: [
        muleDaemonClientProvider.overrideWithValue(widget.viewModel.client),
        daemonSupervisorProvider.overrideWithValue(widget.viewModel.supervisor),
        devToolsEnabledProvider.overrideWithValue(
          widget.viewModel.isDevToolsAvailable,
        ),
        // The app controller outlives the widget tree; disposal stays manual.
        appControllerProvider.overrideWith(
          (ref) => widget.viewModel,
          disposeNotifier: false,
        ),
      ],
      child: Consumer(
        builder: (context, ref, _) => MaterialApp.router(
          title: 'Emule-Sion',
          debugShowCheckedModeBanner: false,
          theme: AppTheme.dark(),
          locale: ref.watch(localeControllerProvider).locale,
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          routerConfig: _router,
        ),
      ),
    );
  }
}
