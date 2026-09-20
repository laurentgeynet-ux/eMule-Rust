import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/legacy.dart';
import 'package:mule_client/mule_client.dart';

import '../core/daemon_lifecycle.dart';
import 'app_controller.dart';
import 'locale_controller.dart';
import '../ui/features/downloads/view_models/categories_controller.dart';
import '../ui/features/downloads/view_models/network_policy_controller.dart';
import '../ui/features/downloads/view_models/downloads_store_controller.dart';
import 'shell_controller.dart';
import 'stats_controller.dart';
import '../ui/features/downloads/view_models/lab_controller.dart';
import '../ui/features/downloads/view_models/peer_sources_controller.dart';
import '../ui/features/settings/view_models/settings_controller.dart';
import '../ui/features/history/view_models/history_controller.dart';
import '../ui/features/kad/view_models/kad_controller.dart';
import '../ui/features/search/view_models/search_controller.dart';
import '../ui/features/servers/view_models/servers_controller.dart';
import '../ui/features/uploads/view_models/uploads_controller.dart';

/// Daemon client (real `RpcDaemonClient` or `FakeMuleDaemonClient`).
/// Overridden by the `ProviderScope` created in [EmuleSionApp].
final muleDaemonClientProvider = Provider<MuleDaemonClient>(
  (ref) => throw UnimplementedError('muleDaemonClientProvider not overridden'),
);

/// Desktop daemon supervisor (null in fake-daemon or remote-daemon modes).
final daemonSupervisorProvider = Provider<DaemonLifecycle?>(
  (ref) => null,
);

/// Whether dev tools (debug bar, simulation) are enabled
/// (`ENABLE_DEV_TOOLS` or fake daemon).
final devToolsEnabledProvider = Provider<bool>((ref) => false);

/// UI locale preference (persisted via shared_preferences).
final localeControllerProvider = ChangeNotifierProvider<LocaleController>(
  (ref) => LocaleController()..load(),
);

/// Application composition root (daemon session + per-domain controllers).
/// Overridden by the `ProviderScope` created in [EmuleSionApp].
final appControllerProvider = ChangeNotifierProvider<AppController>(
  (ref) => throw UnimplementedError('appControllerProvider not overridden'),
);

/// Latest `DaemonInfo` handshake result (null until connected).
final daemonInfoProvider = Provider<DaemonInfo?>(
  (ref) => ref.watch(appControllerProvider).daemonInfo,
);

/// Daemon WebSocket connection state.
final daemonConnectionStateProvider = Provider<DaemonConnectionState>(
  (ref) => ref.watch(appControllerProvider).connectionState,
);

/// Desktop supervisor state (`idle` when there is no supervisor).
final daemonSupervisorStateProvider = Provider<SupervisorState>(
  (ref) => ref.watch(appControllerProvider).supervisorState,
);

/// History & Statistics sub-domain controller. The provider owns the
/// controller's disposal (it is NOT disposed by the view model).
final historyControllerProvider = ChangeNotifierProvider<HistoryController>(
  (ref) => ref.read(appControllerProvider).history,
);

/// Kad DHT sub-domain controller. Provider-owned disposal.
final kadControllerProvider = ChangeNotifierProvider<KadController>(
  (ref) => ref.read(appControllerProvider).kad,
);

/// Search sub-domain controller. Provider-owned disposal.
final searchControllerProvider = ChangeNotifierProvider<SearchController>(
  (ref) => ref.read(appControllerProvider).search,
);

/// eD2K servers sub-domain controller. Provider-owned disposal.
final serversControllerProvider = ChangeNotifierProvider<ServersController>(
  (ref) => ref.read(appControllerProvider).serversController,
);

/// Peer sources & session sub-domain controller. Provider-owned disposal.
final peerSourcesControllerProvider =
    ChangeNotifierProvider<PeerSourcesController>(
      (ref) => ref.read(appControllerProvider).peerSources,
    );

/// Categories sub-domain controller. Provider-owned disposal.
final categoriesControllerProvider =
    ChangeNotifierProvider<CategoriesController>(
      (ref) => ref.read(appControllerProvider).categoriesCtl,
    );

/// Network policy / IP filter / rate limiter sub-domain controller.
/// Provider-owned disposal.
final networkPolicyControllerProvider =
    ChangeNotifierProvider<NetworkPolicyController>(
      (ref) => ref.read(appControllerProvider).networkPolicyCtl,
    );

/// Daemon settings sub-domain controller. Provider-owned disposal.
final settingsControllerProvider = ChangeNotifierProvider<SettingsController>(
  (ref) => ref.read(appControllerProvider).settingsCtl,
);

/// Loopback lab / dev tools sub-domain controller. Provider-owned disposal.
final labControllerProvider = ChangeNotifierProvider<LabController>(
  (ref) => ref.read(appControllerProvider).labCtl,
);

/// Downloads store sub-domain controller. Provider-owned disposal.
final downloadsStoreControllerProvider =
    ChangeNotifierProvider<DownloadsStoreController>(
      (ref) => ref.read(appControllerProvider).downloadsCtl,
    );

/// Latest aggregate stats tick (`event.statsTick`). Provider-owned disposal.
final statsControllerProvider = ChangeNotifierProvider<StatsController>(
  (ref) => ref.read(appControllerProvider).stats,
);

/// Uploads sub-domain controller. Provider-owned disposal.
final uploadsControllerProvider = ChangeNotifierProvider<UploadsController>(
  (ref) => ref.read(appControllerProvider).uploads,
);

/// App-shell UI state (section, filter, banners). Provider-owned disposal.
final shellControllerProvider = ChangeNotifierProvider<ShellController>(
  (ref) => ref.read(appControllerProvider).shellCtl,
);
