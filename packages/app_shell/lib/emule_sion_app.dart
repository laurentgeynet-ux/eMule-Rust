// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

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
class eMuleRustApp extends StatefulWidget {
  final AppController viewModel;
  final DaemonLifecycle? supervisor;

  /// When non-null (remote web client), the sidebar exposes a
  /// "Change daemon" action that invokes this callback.
  final VoidCallback? onChangeDaemon;

  const eMuleRustApp({
    super.key,
    required this.viewModel,
    this.supervisor,
    this.onChangeDaemon,
  });

  @override
  State<eMuleRustApp> createState() => _eMuleRustAppState();
}

class _eMuleRustAppState extends State<eMuleRustApp> {
  AppLifecycleListener? _lifecycleListener;
  late final GoRouter _router = createAppRouter(widget.viewModel);

  /// Installe une seule fois les hooks de capture d'erreurs Flutter vers
  /// le ring buffer [AppLogger] (framework + erreurs async de plateforme).
  static bool _errorHooksInstalled = false;

  @override
  void initState() {
    super.initState();
    _installErrorHooks();
    _lifecycleListener = AppLifecycleListener(
      onExitRequested: _handleExitRequested,
    );
  }

  void _installErrorHooks() {
    if (_errorHooksInstalled) return;
    _errorHooksInstalled = true;

    final prevFramework = FlutterError.onError;
    FlutterError.onError = (details) {
      AppLogger.error(
        'flutter',
        details.exceptionAsString(),
        error: details.exception,
        stackTrace: details.stack,
      );
      prevFramework?.call(details);
    };

    PlatformDispatcher.instance.onError = (error, stack) {
      AppLogger.error(
        'platform',
        'Erreur asynchrone non interceptée',
        error: error,
        stackTrace: stack,
      );
      // true = erreur gérée : l'app survit et la trace reste dans le
      // buffer diagnostics.
      return true;
    };
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
        onChangeDaemonProvider.overrideWithValue(widget.onChangeDaemon),
      ],
      child: Consumer(
        builder: (context, ref, _) => MaterialApp.router(
          title: 'eMule-Rust',
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
