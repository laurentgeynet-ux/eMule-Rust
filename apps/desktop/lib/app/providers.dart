import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/legacy.dart';
import 'package:mule_client/mule_client.dart';

import '../services/daemon_supervisor.dart';
import '../ui/features/downloads/view_models/downloads_view_model.dart';
import '../ui/features/downloads/view_models/peer_sources_controller.dart';
import '../ui/features/history/view_models/history_controller.dart';
import '../ui/features/kad/view_models/kad_controller.dart';
import '../ui/features/search/view_models/search_controller.dart';
import '../ui/features/servers/view_models/servers_controller.dart';

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

/// Kad DHT sub-domain controller. Provider-owned disposal.
final kadControllerProvider = ChangeNotifierProvider<KadController>(
  (ref) => ref.watch(downloadsViewModelProvider).kad,
);

/// Search sub-domain controller. Provider-owned disposal.
final searchControllerProvider = ChangeNotifierProvider<SearchController>(
  (ref) => ref.watch(downloadsViewModelProvider).search,
);

/// eD2K servers sub-domain controller. Provider-owned disposal.
final serversControllerProvider = ChangeNotifierProvider<ServersController>(
  (ref) => ref.watch(downloadsViewModelProvider).serversController,
);

/// Peer sources & session sub-domain controller. Provider-owned disposal.
final peerSourcesControllerProvider =
    ChangeNotifierProvider<PeerSourcesController>(
      (ref) => ref.watch(downloadsViewModelProvider).peerSources,
    );
