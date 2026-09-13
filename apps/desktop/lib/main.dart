import 'package:flutter/material.dart';

import 'data/fake_daemon_client.dart';
import 'data/rpc_daemon_client.dart';
import 'domain/services/mule_daemon_client.dart';
import 'ui/core/theme.dart';
import 'ui/features/downloads/view_models/downloads_view_model.dart';
import 'ui/features/downloads/views/downloads_screen.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // Check if user specified fake daemon via dart-define
  const useFake = bool.fromEnvironment('USE_FAKE_DAEMON', defaultValue: false);

  final MuleDaemonClient client;
  if (useFake) {
    client = FakeMuleDaemonClient(enableAutoSimulation: true);
  } else {
    client = RpcDaemonClient();
  }

  final viewModel = DownloadsViewModel(client: client);

  // Initialize connection in background
  viewModel.initialize(url: Uri.parse('ws://127.0.0.1:4660/rpc'));

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
