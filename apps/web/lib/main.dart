import 'package:flutter/material.dart';
import 'package:flutter_web_plugins/url_strategy.dart';

import 'package:app_shell/app/app_controller.dart';
import 'package:app_shell/emule_sion_app.dart';
import 'package:mule_client/mule_client.dart';

/// Emule-Sion web remote controller.
///
/// Connects to an already-running mule-daemon over WebSocket — no local
/// supervisor. The daemon URL is resolved from, in order:
/// 1. `?daemon=ws://host:port/rpc` query parameter
/// 2. `--dart-define=DAEMON_URL=…`
/// 3. `ws://<current-host>:4660/rpc` (same host as the web app)
void main() {
  WidgetsFlutterBinding.ensureInitialized();
  usePathUrlStrategy();

  const defined = String.fromEnvironment('DAEMON_URL');
  final query = Uri.base.queryParameters['daemon'];
  final fallback = 'ws://${Uri.base.host}:4660/rpc';
  final daemonUrl = (query != null && query.isNotEmpty)
      ? query
      : (defined.isNotEmpty ? defined : fallback);

  final client = RpcDaemonClient();
  final viewModel = AppController(client: client);
  viewModel.initialize(url: Uri.parse(daemonUrl));

  runApp(EmuleSionApp(viewModel: viewModel));
}
