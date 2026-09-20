import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/legacy.dart';
import 'package:mule_client/mule_client.dart';

import '../services/daemon_supervisor.dart';
import '../ui/features/downloads/view_models/downloads_view_model.dart';
import '../ui/features/history/view_models/history_controller.dart';

/// Daemon client (real `RpcDaemonClient` or `FakeMuleDaemonClient`).
/// Overridden by the `ProviderScope` created in [EmuleSionApp].
final muleDaemonClientProvider = Provider<MuleDaemonClient>(
  (ref) => throw UnimplementedError('muleDaemonClientProvider not overridden'),
);

/// Desktop daemon supervisor (null in fake-daemon or remote-daemon modes).
final daemonSupervisorProvider = Provider<DaemonSupervisor?>(
  (ref) => null,
);

/// Whether dev tools (debug bar, simulation) are enabled
/// (`ENABLE_DEV_TOOLS` or fake daemon).
final devToolsEnabledProvider = Provider<bool>((ref) => false);

/// Root presentation view model. Kept as the source of truth during the
/// incremental Riverpod migration; per-domain controllers are progressively
/// extracted from it and exposed below.
final downloadsViewModelProvider = Provider<DownloadsViewModel>(
  (ref) =>
      throw UnimplementedError('downloadsViewModelProvider not overridden'),
);

/// History & Statistics sub-domain controller. The provider owns the
/// controller's disposal (it is NOT disposed by the view model).
final historyControllerProvider = ChangeNotifierProvider<HistoryController>(
  (ref) => ref.watch(downloadsViewModelProvider).history,
);
