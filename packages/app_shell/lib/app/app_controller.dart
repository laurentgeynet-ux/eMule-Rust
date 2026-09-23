// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'package:mule_client/mule_client.dart';

import 'shell_controller.dart';

export 'shell_controller.dart';
import '../core/daemon_lifecycle.dart';
import '../l10n/l10n_lookup.dart';

import '../ui/features/history/view_models/history_controller.dart';
import '../ui/features/kad/view_models/kad_controller.dart';
import '../ui/features/search/view_models/search_controller.dart';
import '../ui/features/servers/view_models/servers_controller.dart';
import '../ui/features/settings/view_models/settings_controller.dart';
import '../ui/features/downloads/view_models/lab_controller.dart';
import '../ui/features/downloads/view_models/categories_controller.dart';
import '../ui/features/downloads/view_models/downloads_store_controller.dart';
import 'stats_controller.dart';
import 'transfer_rate_tracker.dart';
import '../ui/features/downloads/view_models/network_policy_controller.dart';
import '../ui/features/downloads/view_models/peer_sources_controller.dart';
import '../ui/features/uploads/view_models/uploads_controller.dart';
import '../ui/features/shares/view_models/shares_controller.dart';

// Application composition root and daemon session owner.
///
/// Wires the per-domain controllers (downloads store, categories, network
/// policy, settings, lab, history, servers, peer sources, kad, search, shell)
/// together, owns the daemon connection/supervisor subscriptions, and
/// orchestrates `initialize`/`refresh`. Forwards controller notifications so
/// legacy `ListenableBuilder` consumers keep working; new widgets should
/// watch the per-domain providers in `app/providers.dart` instead.
class AppController extends ChangeNotifier {
  final MuleDaemonClient client;
  final DaemonLifecycle? supervisor;
  final bool isDevToolsAvailable;

  DaemonConnectionState _connectionState = DaemonConnectionState.disconnected;
  SupervisorState _supervisorState = SupervisorState.idle;
  DaemonInfo? _daemonInfo;
  /// Shell UI state (section, filter, banners) — extracted controller.
  late final ShellController shellCtl;
  /// Downloads store sub-domain (extracted controller, see
  /// `downloads_store_controller.dart`).
  late final DownloadsStoreController downloadsCtl;
  /// Categories sub-domain (extracted controller).
  late final CategoriesController categoriesCtl;

  /// Network policy / IP filter / rate limiter sub-domain (extracted
  /// controller).
  late final NetworkPolicyController networkPolicyCtl;

  /// Daemon settings sub-domain (extracted controller).
  late final SettingsController settingsCtl;

  /// Rolling 60 s transfer-rate samples for the top-bar sparkline.
  late final TransferRateTracker speedTracker;

  /// Loopback lab / dev tools sub-domain (extracted controller).
  late final LabController labCtl;

  /// History & Statistics sub-domain (extracted controller, see
  /// `features/history/view_models/history_controller.dart`).
  late final HistoryController history;

  /// eD2K servers sub-domain (extracted controller, see
  /// `features/servers/view_models/servers_controller.dart`).
  late final ServersController serversController;

  /// Peer sources & session sub-domain (extracted controller).
  late final PeerSourcesController peerSources;

  /// Kad DHT sub-domain (extracted controller).
  late final KadController kad;

  /// Search sub-domain (extracted controller, see
  /// `features/search/view_models/search_controller.dart`).
  late final SearchController search;

  /// Uploads sub-domain (extracted controller, see
  /// `features/uploads/view_models/uploads_controller.dart`).
  late final UploadsController uploads;

  /// Shared files sub-domain (extracted controller, see
  /// `features/shares/view_models/shares_controller.dart`).
  late final SharesController shares;

  /// Latest aggregate `event.statsTick` snapshot (app-level).
  late final StatsController stats;

  StreamSubscription<DaemonConnectionState>? _connSub;
  StreamSubscription<SupervisorState>? _supervisorSub;
  StreamSubscription<void>? _instanceSub;
  StreamSubscription<DownloadLifecycleEvent>? _lifecycleSub;


  AppController({
    required this.client,
    this.supervisor,
    this.isDevToolsAvailable = false,
  }) {
    shellCtl = ShellController(
      isDevToolsAvailable: isDevToolsAvailable,
      onSectionSelected: (section) {
        if (section == AppNavSection.history) {
          history.fetch();
        } else if (section == AppNavSection.servers) {
          serversController.fetch();
        } else if (section == AppNavSection.kad) {
          kad.fetchStatus();
          kad.fetchContacts();
        } else if (section == AppNavSection.uploads) {
          uploads.fetch();
        } else if (section == AppNavSection.shares) {
          shares.fetch();
        }
      },
    )..addListener(notifyListeners);
    downloadsCtl = DownloadsStoreController(
      client: client,
      onStatus: shellCtl.setStatus,
      onError: shellCtl.setError,
      onRefresh: refresh,
    )..addListener(notifyListeners);
    uploads = UploadsController(client: client)
      ..attach()
      ..addListener(notifyListeners);
    shares = SharesController(client: client)
      ..attach()
      ..addListener(notifyListeners);
    stats = StatsController(client: client)
      ..attach()
      ..addListener(notifyListeners);
    speedTracker = TransferRateTracker(
      // Prefer the daemon's aggregate rates (statsTick) when available;
      // fall back to client-side sums for daemons without E2 support.
      downloadRate: () =>
          stats.latest?.downloadBps ?? downloadsCtl.totalDownloadSpeed,
      uploadRate: () =>
          stats.latest?.uploadBps ?? uploads.totalUploadRateBps,
    );
    history = HistoryController(
      client: client,
      onStatus: shellCtl.setStatus,
      onError: shellCtl.setError,
    )..addListener(notifyListeners);
    kad = KadController(
      client: client,
      isSectionActive: () => shellCtl.section == AppNavSection.kad,
      onStatus: shellCtl.setStatus,
      onError: shellCtl.setError,
    )..addListener(notifyListeners);
    search = SearchController(
      client: client,
      onStatus: shellCtl.setStatus,
      onError: shellCtl.setError,
    )..addListener(notifyListeners);
    serversController = ServersController(
      client: client,
      onStatus: shellCtl.setStatus,
      onError: shellCtl.setError,
    )..addListener(notifyListeners);
    peerSources = PeerSourcesController(
      client: client,
      onStatus: shellCtl.setStatus,
      onError: shellCtl.setError,
    )..addListener(notifyListeners);
    categoriesCtl = CategoriesController(
      client: client,
      onStatus: shellCtl.setStatus,
      onError: shellCtl.setError,
      onRefreshDownloads: refresh,
      onDownloadUpdated: downloadsCtl.upsert,
    )..addListener(notifyListeners);
    networkPolicyCtl = NetworkPolicyController(
      client: client,
      onStatus: shellCtl.setStatus,
      onError: shellCtl.setError,
      onAllDisconnected: serversController.markDisconnected,
    )..addListener(notifyListeners);
    settingsCtl = SettingsController(
      client: client,
      onStatus: shellCtl.setStatus,
      onError: shellCtl.setError,
      onSettingsLoaded: history.syncRetentionFromSettings,
    )..addListener(notifyListeners);
    labCtl = LabController(
      client: client,
      onStatus: shellCtl.setStatus,
      onError: shellCtl.setError,
      onDownloadUpdated: downloadsCtl.upsert,
      onRefreshDownloads: refresh,
    )..addListener(notifyListeners);

    _lifecycleSub = client.downloadLifecycleEvents.listen((e) {
      switch (e.kind) {
        case DownloadLifecycleKind.added:
          final d = e.download;
          if (d != null) downloadsCtl.upsert(d);
        case DownloadLifecycleKind.removed:
          final id = e.id;
          if (id != null) downloadsCtl.removeLocal(id);
        case DownloadLifecycleKind.completed:
          final d = e.download;
          if (d != null) {
            downloadsCtl.upsert(d);
            shellCtl.setStatus(
              L10nLookup.current.msgDownloadCompleted(d.fileName),
            );
          }
        case DownloadLifecycleKind.failed:
          final d = e.download;
          if (d != null) downloadsCtl.upsert(d);
          shellCtl.setError(
            L10nLookup.current
                .msgDownloadFailed(d?.fileName ?? e.id ?? '?'),
          );
      }
    });

    _connSub = client.connectionState.listen((state) {
      _connectionState = state;
      notifyListeners();
    });

    if (supervisor != null) {
      _supervisorState = supervisor!.state;
      _daemonInfo = client.daemonInfo;
      _supervisorSub = supervisor!.stateStream.listen((sState) {
        _supervisorState = sState;
        if (sState == SupervisorState.connected) {
          shellCtl.setProtocolMismatch(null);
          _daemonInfo = client.daemonInfo;
          refresh();
        } else if (sState == SupervisorState.protocolMismatch) {
          shellCtl.setProtocolMismatch(supervisor!.lastError);
        }
        notifyListeners();
      });

      _instanceSub = supervisor!.instanceChangedStream.listen((_) {
        // Reset downloads and sequence on daemon restart
        downloadsCtl.clearAll();
        _daemonInfo = client.daemonInfo;
        refresh();
      });
    }
  }

  // Getters
  DaemonConnectionState get connectionState => _connectionState;
  SupervisorState get supervisorState => _supervisorState;
  String? get protocolMismatchError => shellCtl.protocolMismatchError;
  DaemonInfo? get daemonInfo => _daemonInfo ?? client.daemonInfo;
  String? get errorMessage => shellCtl.errorMessage;
  bool get isLoading => downloadsCtl.isLoading;
  AppNavSection get currentSection => shellCtl.section;
  String get searchQuery => shellCtl.searchQuery;
  String? get selectedDownloadId => downloadsCtl.selectedId;
  bool get isDebugBarVisible => shellCtl.isDebugBarVisible;
  String? get statusNotification => shellCtl.statusNotification;

  void clearStatusNotification() {
    shellCtl.clearStatusNotification();
    notifyListeners();
  }

  List<CategoryItem> get categories => categoriesCtl.categories;
  String? get selectedCategoryId => categoriesCtl.selectedId;

  Map<String, int> get categoryCounts {
    final map = <String, int>{};
    for (final cat in categories) {
      map[cat.id] =
          allDownloads.where((d) => d.categoryId == cat.id).length;
    }
    return map;
  }

  int get uncategorizedCount =>
      allDownloads.where((d) => d.categoryId == null).length;

  int get historyRetentionDays => history.retentionDays;

  /// Lab and test fixture features are strictly dual-gated:
  /// Requires Portable workdir mode AND the daemon having the 'lab-tools' capability.

  Map<String, dynamic> get settings => settingsCtl.settings;

  /// Default download directory path (custom setting > daemon > supervisor).


  DownloadItem? get selectedDownload => selectedDownloadId != null
      ? downloadsCtl.downloadById(selectedDownloadId!)
      : null;

  int get totalDownloadSpeed => downloadsCtl.totalDownloadSpeed;

  /// Always 0 B/s in this offline / loopback lab client without public upload/seeding.

  int get countAll => downloadsCtl.countAll;
  int get countActive => downloadsCtl.countActive;
  int get countPaused => downloadsCtl.countPaused;
  int get countCompleted => downloadsCtl.countCompleted;
  int get countFailed => downloadsCtl.countFailed;

  List<DownloadItem> get allDownloads => downloadsCtl.allDownloads;

  DownloadItem? getDownloadByHash(String? hash) =>
      downloadsCtl.getDownloadByHash(hash);

  List<DownloadItem> get filteredDownloads {
    final query = shellCtl.searchQuery.trim().toLowerCase();
    return allDownloads.where((item) {
      // Filter tab check
      switch (shellCtl.filter) {
        case DownloadFilter.all:
          break;
        case DownloadFilter.active:
          if (!item.status.isActive) return false;
          break;
        case DownloadFilter.paused:
          if (!item.status.isPaused) return false;
          break;
        case DownloadFilter.completed:
          if (!item.status.isCompleted) return false;
          break;
        case DownloadFilter.failed:
          if (!item.status.isFailed) return false;
          break;
      }

      // Search query check
      if (query.isNotEmpty && !item.fileName.toLowerCase().contains(query)) {
        return false;
      }

      // Category filter check
      if (selectedCategoryId != null) {
        if (selectedCategoryId == 'uncategorized') {
          if (item.categoryId != null) return false;
        } else {
          if (item.categoryId != selectedCategoryId) return false;
        }
      }

      return true;
    }).toList()..sort(compareDownloadsForList);
  }

  // Actions
  Future<void> initialize({Uri? url, String? token}) async {
    downloadsCtl.setLoading(true);
    shellCtl.setError(null);
    notifyListeners();

    try {
      final targetUrl = url ?? Uri.parse('ws://127.0.0.1:4660/rpc');
      _daemonInfo = await client.connect(url: targetUrl, token: token);
      await refresh();
    } catch (e) {
      shellCtl.setError(e.toString());
    } finally {
      downloadsCtl.setLoading(false);
      notifyListeners();
    }
  }

  Future<void> refresh() async {
    try {
      if (client.daemonInfo != null) {
        _daemonInfo = client.daemonInfo;
      }
      await fetchSettings();
      await fetchHistory();
      await fetchCategories();
      await fetchNetworkPolicy();
      await fetchKadStatus();
      await downloadsCtl.reload();
      await uploads.fetch();
      await shares.fetch();
      shellCtl.setError(null);
    } catch (e) {
      shellCtl.setError("Refresh failed: $e");
    }
    notifyListeners();
  }

  void setSection(AppNavSection section) => shellCtl.setSection(section);

  void setFilter(DownloadFilter filter) => shellCtl.setFilter(filter);

  void setSearchQuery(String query) => shellCtl.setSearchQuery(query);

  void selectDownload(String? id) {
    downloadsCtl.select(id);
  }

  void toggleDebugBar() => shellCtl.toggleDebugBar();

  // ── Download actions (delegated to DownloadsStoreController) ──────────────

  Future<bool> addEd2kLink(
    String link, {
    String? targetDirectory,
    String? allocationPolicy,
    String? categoryId,
  }) =>
      downloadsCtl.addEd2kLink(
        link,
        targetDirectory: targetDirectory,
        allocationPolicy: allocationPolicy,
        categoryId: categoryId,
      );

  Future<PartMetInspectionResult> inspectPartMet(
    String metPath, {
    String? partPath,
  }) =>
      downloadsCtl.inspectPartMet(metPath, partPath: partPath);

  Future<ImportPartMetResult?> importPartMet(
    String metPath, {
    String? partPath,
    String? targetDirectory,
    String? categoryId,
  }) =>
      downloadsCtl.importPartMet(
        metPath,
        partPath: partPath,
        targetDirectory: targetDirectory,
        categoryId: categoryId,
      );

  Future<void> pauseDownload(String id) => downloadsCtl.pause(id);

  Future<void> resumeDownload(String id) => downloadsCtl.resume(id);

  Future<void> removeDownload(String id, {bool deleteFiles = false}) =>
      downloadsCtl.remove(id, deleteFiles: deleteFiles);



  Future<bool> retryDownloadConnection(String id) =>
      downloadsCtl.retryConnection(id);

  Future<void> openCompletedFile(DownloadItem item) =>
      downloadsCtl.openCompletedFile(item);


  // ── Settings (delegated to SettingsController) ────────────────────────────

  Future<void> fetchSettings() => settingsCtl.fetch();

  Future<bool> setDefaultDownloadDirectory(String path) =>
      settingsCtl.setDefaultDownloadDirectory(path);

  Future<bool> resetDefaultDownloadDirectory() =>
      settingsCtl.resetDefaultDownloadDirectory();

  // ── History (delegated to HistoryController) ──────────────────────────────



  Future<void> fetchHistory() => history.fetch();

  Future<bool> clearHistory() => history.clear();

  Future<String> exportHistory(String format) => history.export(format);

  Future<bool> setHistoryRetentionDays(int days) =>
      history.setRetentionDays(days);

  // ── Lab / dev tools (delegated to LabController) ──────────────────────────

  Future<void> injectSimulationChunk(
    String id,
    int offset,
    String hexData,
  ) =>
      labCtl.injectSimulationChunk(id, offset, hexData);

  Future<LabStatus> getLabStatus() => labCtl.getLabStatus();

  Future<LabFixtureInfo> createLabFixture(String fixtureType) =>
      labCtl.createLabFixture(fixtureType);

  Future<LabPeerInfo> startLabPeer() => labCtl.startLabPeer();

  Future<void> stopLabPeer() => labCtl.stopLabPeer();

  Future<LabCleanupReport> cleanupLabTestData() => labCtl.cleanupLabTestData();

  Future<void> connectDownloadToSource(String downloadId, String peerAddr) =>
      labCtl.connectDownloadToSource(downloadId, peerAddr);

  // --- Category Actions (delegated to CategoriesController) ---


  Future<void> fetchCategories() => categoriesCtl.fetch();

  Future<bool> createCategory({
    required String name,
    required String color,
    String? defaultDestination,
    String? defaultPriority,
  }) =>
      categoriesCtl.create(
        name: name,
        color: color,
        defaultDestination: defaultDestination,
        defaultPriority: defaultPriority,
      );

  Future<bool> updateCategory(
    String id, {
    String? name,
    String? color,
    String? defaultDestination,
    String? defaultPriority,
    int? sortOrder,
  }) =>
      categoriesCtl.update(
        id,
        name: name,
        color: color,
        defaultDestination: defaultDestination,
        defaultPriority: defaultPriority,
        sortOrder: sortOrder,
      );

  Future<bool> deleteCategory(String id) => categoriesCtl.delete(id);

  Future<bool> reorderCategories(List<String> categoryIds) =>
      categoriesCtl.reorder(categoryIds);

  Future<bool> setDownloadCategory(
    String downloadId,
    String? categoryId,
  ) =>
      categoriesCtl.assignDownload(downloadId, categoryId);

  // ── Network Policy, IP Filter & Rate Limiter ──────────────────────────────
  // (delegated to NetworkPolicyController)

  NetworkProfile get currentNetworkProfile => networkPolicyCtl.currentProfile;
  bool get isNetworkKillSwitchActive => networkPolicyCtl.isKillSwitchActive;
  FilterStatusModel? get filterStatus => networkPolicyCtl.filterStatus;
  RateLimitsModel? get rateLimits => networkPolicyCtl.rateLimits;

  Future<void> fetchNetworkPolicy() => networkPolicyCtl.fetchPolicy();
  Future<void> setNetworkProfile(NetworkProfile profile) =>
      networkPolicyCtl.setProfile(profile);
  Future<void> disconnectAllNetwork() => networkPolicyCtl.disconnectAll();
  Future<void> enableNetwork() => networkPolicyCtl.enableNetwork();
  Future<void> setResourceBudgets(ResourceBudgetsModel budgets) =>
      networkPolicyCtl.setResourceBudgets(budgets);
  Future<void> fetchFilterStatus() => networkPolicyCtl.fetchFilterStatus();
  Future<bool> importFilterFile(String filePath) =>
      networkPolicyCtl.importFilterFile(filePath);
  Future<void> setFilterEnabled(bool enabled) =>
      networkPolicyCtl.setFilterEnabled(enabled);
  Future<void> clearFilter() => networkPolicyCtl.clearFilter();
  Future<void> fetchRateLimits() => networkPolicyCtl.fetchRateLimits();
  Future<void> setRateLimits(RateLimitsModel limits) =>
      networkPolicyCtl.setRateLimits(limits);

  List<Ed2kServerModel> get servers => serversController.servers;
  ServerSessionModel get serverSession => serversController.session;
  bool get isServerConnecting => serversController.isConnecting;


  Future<ServerInspectReport?> inspectServerMet(String filePath) =>
      serversController.inspectServerMet(filePath);

  Future<bool> importServerMet(String filePath) =>
      serversController.importServerMet(filePath);

  Future<Map<String, dynamic>?> updateServersFromUrl(String url) =>
      serversController.updateFromUrl(url);

  Future<void> setServerEnabled(String address, bool enabled) =>
      serversController.setEnabled(address, enabled);

  Future<void> deleteServer(String address) => serversController.delete(address);

  Future<void> clearServers() => serversController.clear();

  // ── eD2K Server Session ─────────────────────────────────────────────────────

  Future<void> connectServer(String address) => serversController.connect(address);

  Future<void> disconnectServer() => serversController.disconnect();


  // ── eD2K Server Search & Sources ──────────────────────────────────────────

  List<SearchResultFileModel> get searchResults => search.results;
  bool get isSearching => search.isSearching;

  Future<void> searchServer({
    required String term,
    int? minSize,
    int? maxSize,
    String? extension,
    bool udpOnly = false,
  }) =>
      search.searchServer(
        term: term,
        minSize: minSize,
        maxSize: maxSize,
        extension: extension,
        udpOnly: udpOnly,
      );

  Future<void> searchKadKeywords({
    required String term,
    int? minSize,
    int? maxSize,
  }) =>
      search.searchKadKeywords(term: term, minSize: minSize, maxSize: maxSize);



  Future<void> queryServerSources(String fileHash, {int? fileSize}) =>
      search.querySources(fileHash, fileSize: fileSize);

  // --- Peer Sources & Session ---

  bool get isPeerConnecting => peerSources.isConnecting;

  List<PeerSourceModel> getSourcesForHash(String fileHash) =>
      peerSources.getSourcesForHash(fileHash);

  Future<void> loadSourcesForHash(String fileHash) =>
      peerSources.loadSourcesForHash(fileHash);

  Future<bool> addManualSource(String fileHash, String address) =>
      peerSources.addManualSource(fileHash, address);

  Future<bool> connectPeerSource(
    String fileHash,
    String address, {
    bool force = false,
  }) =>
      peerSources.connect(fileHash, address, force: force);

  Future<void> disconnectPeerSource({String? address}) =>
      peerSources.disconnect(address: address);

  Future<void> clearSourcesForHash(String fileHash) =>
      peerSources.clearSourcesForHash(fileHash);

  // --- Kad DHT (delegated to KadController) ---

  KadStatusModel get kadStatus => kad.status;
  List<KadContactModel> get kadContacts => kad.contacts;

  Future<void> fetchKadStatus() => kad.fetchStatus();
  Future<void> fetchKadContacts({int limit = 100}) =>
      kad.fetchContacts(limit: limit);
  Future<NodesDatInspectReportModel?> inspectNodesDat(String filePath) =>
      kad.inspectNodesDat(filePath);
  Future<bool> importNodesDat(String filePath) => kad.importNodesDat(filePath);
  Future<void> clearKadContacts() => kad.clearContacts();
  Future<bool> startKadUdp({String? bindAddress}) =>
      kad.startUdp(bindAddress: bindAddress);
  Future<bool> stopKadUdp() => kad.stopUdp();
  Future<int?> pingKadContact({required String ip, required int port, String? kadId}) =>
      kad.pingContact(ip: ip, port: port, kadId: kadId);
  Future<bool> bootstrapKad({required String ip, required int port}) =>
      kad.bootstrap(ip: ip, port: port);
  Future<bool> bootstrapKadFromUrl(String url) => kad.bootstrapFromUrl(url);
  Future<List<KadContactModel>> findKadNode(String targetId) =>
      kad.findNode(targetId);
  Future<List<PeerSourceModel>> searchKadSources(String fileHash, {int? fileSize}) =>
      kad.searchSources(fileHash, fileSize: fileSize);
  Future<int> publishKadSource(String fileHash, int fileSize, {int tcpPort = 39735}) =>
      kad.publishSource(fileHash, fileSize, tcpPort: tcpPort);

  bool _disposed = false;

  @override
  void dispose() {
    // Idempotent: the controller may be disposed both by its owner (tests,
    // entry points) and by the ProviderScope overriding appControllerProvider.
    if (_disposed) return;
    _disposed = true;
    speedTracker.dispose();
    // stats/uploads and the per-domain controllers are disposed by their
    // ChangeNotifierProviders when the ProviderScope unmounts.
    _connSub?.cancel();
    _lifecycleSub?.cancel();
    
    _supervisorSub?.cancel();
    _instanceSub?.cancel();
    
    

    supervisor?.dispose();
    client.dispose();
    super.dispose();
  }
}
