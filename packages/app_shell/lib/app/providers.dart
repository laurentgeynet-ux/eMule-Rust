// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/legacy.dart';
import 'package:mule_client/mule_client.dart';

import '../core/daemon_lifecycle.dart';
import '../core/work_dirs.dart';
import 'transfer_rate_tracker.dart';
import 'app_controller.dart';
import 'locale_controller.dart';
import '../ui/features/downloads/view_models/categories_controller.dart';
import '../ui/features/downloads/view_models/network_policy_controller.dart';
import '../ui/features/downloads/view_models/downloads_store_controller.dart';
import 'stats_controller.dart';
import '../ui/features/downloads/view_models/lab_controller.dart';
import '../ui/features/downloads/view_models/peer_sources_controller.dart';
import '../ui/features/settings/view_models/settings_controller.dart';
import '../ui/features/history/view_models/history_controller.dart';
import '../ui/features/kad/view_models/kad_controller.dart';
import '../ui/features/search/view_models/search_controller.dart';
import '../ui/features/servers/view_models/servers_controller.dart';
import '../ui/features/shares/view_models/shares_controller.dart';
import '../ui/features/uploads/view_models/uploads_controller.dart';

/// Daemon client (real `RpcDaemonClient` or `FakeMuleDaemonClient`).
/// Overridden by the `ProviderScope` created in [eMuleRustApp].
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
/// Overridden by the `ProviderScope` created in [eMuleRustApp].
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

/// Shared files sub-domain controller. Provider-owned disposal.
final sharesControllerProvider = ChangeNotifierProvider<SharesController>(
  (ref) => ref.read(appControllerProvider).shares,
);

/// App-shell UI state (section, filter, banners). Provider-owned disposal.
final shellControllerProvider = ChangeNotifierProvider<ShellController>(
  (ref) => ref.read(appControllerProvider).shellCtl,
);

/// Rolling transfer-rate samples for the top-bar sparkline.
final speedTrackerProvider = Provider<TransferRateTracker>(
  (ref) => ref.read(appControllerProvider).speedTracker,
);

/// Downloads after filter tab + search query + category selection.
final filteredDownloadsProvider = Provider<List<DownloadItem>>((ref) {
  final store = ref.watch(downloadsStoreControllerProvider);
  final shell = ref.watch(shellControllerProvider);
  final categories = ref.watch(categoriesControllerProvider);
  final query = shell.searchQuery.trim().toLowerCase();
  return store.allDownloads.where((item) {
    switch (shell.filter) {
      case DownloadFilter.all:
        break;
      case DownloadFilter.active:
        if (!item.status.isActive) return false;
      case DownloadFilter.paused:
        if (!item.status.isPaused) return false;
      case DownloadFilter.completed:
        if (!item.status.isCompleted) return false;
      case DownloadFilter.failed:
        if (!item.status.isFailed) return false;
    }
    if (query.isNotEmpty && !item.fileName.toLowerCase().contains(query)) {
      return false;
    }
    final catId = categories.selectedId;
    if (catId != null) {
      if (catId == 'uncategorized') {
        if (item.categoryId != null) return false;
      } else if (item.categoryId != catId) {
        return false;
      }
    }
    return true;
  }).toList()
    // Terminés en bas ; les autres par avancement décroissant.
    ..sort(compareDownloadsForList);
});

/// Per-category download counts (sidebar badges).
final categoryCountsProvider = Provider<Map<String, int>>((ref) {
  final store = ref.watch(downloadsStoreControllerProvider);
  final categories = ref.watch(categoriesControllerProvider);
  return {
    for (final cat in categories.categories)
      cat.id:
          store.allDownloads.where((d) => d.categoryId == cat.id).length,
  };
});

/// Downloads with no category assigned.
final uncategorizedCountProvider = Provider<int>(
  (ref) => ref
      .watch(downloadsStoreControllerProvider)
      .allDownloads
      .where((d) => d.categoryId == null)
      .length,
);

/// Currently selected download (null when none).
final selectedDownloadProvider = Provider<DownloadItem?>((ref) {
  final store = ref.watch(downloadsStoreControllerProvider);
  final id = store.selectedId;
  return id == null ? null : store.downloadById(id);
});

/// Effective default download directory (custom setting > daemon >
/// supervisor workdir).
final defaultDownloadDirectoryProvider = Provider<String>((ref) {
  final settings = ref.watch(settingsControllerProvider).settings;
  final custom = settings['defaultDownloadDir'] as String?;
  if (custom != null && custom.isNotEmpty) return custom;
  final daemonDir = ref.watch(daemonInfoProvider)?.defaultDownloadDir;
  if (daemonDir != null && daemonDir.isNotEmpty) return daemonDir;
  return ref
          .watch(daemonSupervisorProvider)
          ?.resolveWorkDirInfo()
          .defaultDownloadDirPath ??
      '';
});

/// Optional "change daemon" action — non-null only for remote clients
/// (web) that can disconnect and return to a connect form.
final onChangeDaemonProvider = Provider<void Function()?>((ref) => null);

/// Lab/fixture features gate: dev tools OR portable daemon with
/// `lab-tools` capability.
final isLabModeAvailableProvider = Provider<bool>((ref) {
  if (ref.watch(devToolsEnabledProvider)) return true;
  final info = ref.watch(daemonInfoProvider);
  if (info == null) return false;
  final isPortable =
      (info.workdirMode?.toLowerCase() == 'portable') ||
      ref.watch(daemonSupervisorProvider)?.resolveWorkDirInfo().mode ==
          WorkDirMode.portable;
  return isPortable && info.capabilities.contains('lab-tools');
});
