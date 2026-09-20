import 'dart:ui';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_web_plugins/url_strategy.dart';

import 'package:mule_client/mule_client.dart';
import 'services/daemon_supervisor.dart';
import 'core/tray_setup.dart';
import 'core/window_setup.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'app/providers.dart';
import 'app/router.dart';
import 'app/app_controller.dart';
import 'l10n/generated/app_localizations.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  if (kIsWeb) usePathUrlStrategy();
  await WindowSetup.init();
  await TraySetup.init();

  // Check if user specified fake daemon or dev tools via dart-define
  const useFake = bool.fromEnvironment('USE_FAKE_DAEMON', defaultValue: false);
  const enableDevTools = bool.fromEnvironment('ENABLE_DEV_TOOLS', defaultValue: false);
  final isDevToolsAvailable = enableDevTools || useFake;

  final MuleDaemonClient client;
  DaemonSupervisor? supervisor;

  if (useFake) {
    client = FakeMuleDaemonClient(enableAutoSimulation: true);
  } else {
    final rpcClient = RpcDaemonClient();
    supervisor = DaemonSupervisor(client: rpcClient);
    client = rpcClient;
  }

  final viewModel = AppController(
    client: client,
    supervisor: supervisor,
    isDevToolsAvailable: isDevToolsAvailable,
  );

  if (supervisor != null) {
    supervisor.start();
  } else {
    viewModel.initialize(url: Uri.parse('ws://127.0.0.1:4660/rpc'));
  }

  runApp(EmuleSionApp(viewModel: viewModel, supervisor: supervisor));
}

class EmuleSionApp extends StatefulWidget {
  final AppController viewModel;
  final DaemonSupervisor? supervisor;

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
