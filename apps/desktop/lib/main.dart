import 'package:flutter/material.dart';

import 'data/fake_daemon_client.dart';
import 'data/rpc_daemon_client.dart';
import 'domain/services/mule_daemon_client.dart';
import 'services/daemon_supervisor.dart';
import 'ui/core/theme.dart';
import 'ui/features/downloads/view_models/downloads_view_model.dart';
import 'ui/features/downloads/views/downloads_screen.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

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

  final viewModel = DownloadsViewModel(
    client: client,
    supervisor: supervisor,
    isDevToolsAvailable: isDevToolsAvailable,
  );

  if (supervisor != null) {
    supervisor.start();
  } else {
    viewModel.initialize(url: Uri.parse('ws://127.0.0.1:4660/rpc'));
  }

  runApp(EmuleSionApp(viewModel: viewModel));
}

class EmuleSionApp extends StatelessWidget {
  final DownloadsViewModel viewModel;

  const EmuleSionApp({super.key, required this.viewModel});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Emule-Sion',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.dark(),
      home: DownloadsScreen(viewModel: viewModel),
    );
  }
}
