import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_web_plugins/url_strategy.dart';

import 'package:app_shell/app/app_controller.dart';
import 'package:app_shell/emule_sion_app.dart';
import 'package:mule_client/mule_client.dart';
import 'services/daemon_supervisor.dart';
import 'core/deep_link_handler.dart';
import 'core/download_notifier.dart';
import 'core/tray_setup.dart';
import 'core/window_setup.dart';

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

  DeepLinkHandler(viewModel).init();

  // Native OS toasts on download completion/failure (desktop only).
  final notifier = DownloadNotifier(client);
  unawaited(notifier.init());

  if (supervisor != null) {
    supervisor.start();
  } else {
    viewModel.initialize(url: Uri.parse('ws://127.0.0.1:4660/rpc'));
  }

  runApp(eMuleRustApp(viewModel: viewModel, supervisor: supervisor));
}
