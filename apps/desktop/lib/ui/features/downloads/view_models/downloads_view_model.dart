import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';

import '../../../../domain/models/category_item.dart';
import '../../../../domain/models/daemon_event.dart';
import '../../../../domain/models/daemon_info.dart';
import '../../../../domain/models/download_item.dart';
import '../../../../domain/models/filter_models.dart';
import '../../../../domain/models/history_entry.dart';
import '../../../../domain/models/history_stats.dart';
import '../../../../domain/models/lab_models.dart';
import '../../../../domain/models/network_policy_models.dart';
import '../../../../domain/models/part_met_models.dart';
import '../../../../domain/models/server_models.dart';
import '../../../../domain/services/mule_daemon_client.dart';

import '../../../../services/daemon_supervisor.dart';
import '../../../../services/work_directory_resolver.dart';

enum AppNavSection { downloads, history, servers, search, kad }

enum DownloadFilter {
  all,
  active,
  paused,
  completed,
  failed;

  String get label {
    switch (this) {
      case DownloadFilter.all:
        return 'All Downloads';
      case DownloadFilter.active:
        return 'Active';
      case DownloadFilter.paused:
        return 'Paused';
      case DownloadFilter.completed:
        return 'Completed';
      case DownloadFilter.failed:
        return 'Failed';
    }
  }
}

class DownloadsViewModel extends ChangeNotifier {
  final MuleDaemonClient client;
  final DaemonSupervisor? supervisor;
  final bool isDevToolsAvailable;

  DaemonConnectionState _connectionState = DaemonConnectionState.disconnected;
  SupervisorState _supervisorState = SupervisorState.idle;
  DaemonInfo? _daemonInfo;
  String? _errorMessage;
  String? _protocolMismatchError;
  bool _isLoading = false;

  AppNavSection _currentSection = AppNavSection.downloads;
  final Map<String, DownloadItem> _downloads = {};
  DownloadFilter _currentFilter = DownloadFilter.all;
  String _searchQuery = '';
  String? _selectedDownloadId;
  bool _isDebugBarVisible = false;
  String? _statusNotification;

  List<CategoryItem> _categories = [];
  String? _selectedCategoryId;

  List<HistoryEntry> _historyEntries = [];
  HistoryStats _historyStats = const HistoryStats();
  bool _isHistoryLoading = false;
  String? _historyEventTypeFilter;
  String? _historyStatusFilter;
  int _historyRetentionDays = 30;

  List<Ed2kServerModel> _servers = [];
  bool _isServersLoading = false;
  ServerSessionModel _serverSession = ServerSessionModel.disconnected;
  bool _isServerConnecting = false;

  final List<SearchResultFileModel> _searchResults = [];
  bool _isSearching = false;
  String? _lastSearchTerm;

  final Map<String, List<PeerSourceModel>> _peerSources = {};
  bool _isSourcesLoading = false;
  PeerSessionModel _peerSession = PeerSessionModel.disconnected;
  bool _isPeerConnecting = false;

  KadStatusModel _kadStatus = KadStatusModel.initial;
  List<KadContactModel> _kadContacts = [];
  bool _isKadLoading = false;
  Timer? _kadStatusTimer;

  StreamSubscription<DaemonConnectionState>? _connSub;
  StreamSubscription<ServerSessionModel>? _serverSessionSub;
  StreamSubscription<DownloadUpdatedEvent>? _eventSub;
  StreamSubscription<SupervisorState>? _supervisorSub;
  StreamSubscription<void>? _instanceSub;
  StreamSubscription<List<SearchResultFileModel>>? _searchResultSub;
  StreamSubscription<Map<String, dynamic>>? _foundSourcesSub;
  StreamSubscription<PeerSessionModel>? _peerSessionSub;

  DownloadsViewModel({
    required this.client,
    this.supervisor,
    this.isDevToolsAvailable = false,
  }) {
    _connSub = client.connectionState.listen((state) {
      _connectionState = state;
      if (state == DaemonConnectionState.connected) {
        _startKadStatusTimer();
      } else {
        _kadStatusTimer?.cancel();
        _kadStatusTimer = null;
      }
      notifyListeners();
    });

    _serverSessionSub = client.serverSessionEvents.listen((session) {
      _serverSession = session;
      if (session.status == ServerConnectionStatus.connected ||
          session.status == ServerConnectionStatus.disconnected) {
        _isServerConnecting = false;
      } else if (session.status == ServerConnectionStatus.connecting) {
        _isServerConnecting = true;
      }
      notifyListeners();
    });

    _eventSub = client.downloadEvents.listen((event) {
      _downloads[event.download.id] = event.download;
      notifyListeners();
    });

    _searchResultSub = client.searchResultEvents.listen((files) {
      final existingHashes = _searchResults.map((f) => f.fileHash).toSet();
      for (final f in files) {
        if (!existingHashes.contains(f.fileHash)) {
          _searchResults.add(f);
          existingHashes.add(f.fileHash);
        }
      }
      _isSearching = false;
      notifyListeners();
    });

    _foundSourcesSub = client.foundSourcesEvents.listen((params) {
      final rawSources = params['sources'] as List<dynamic>? ?? [];
      final hash = params['file_hash'] as String? ?? '';
      _statusNotification =
          '${rawSources.length} source(s) trouvée(s) pour $hash';
      if (hash.isNotEmpty) {
        loadSourcesForHash(hash);
      }
      notifyListeners();
    });

    _peerSessionSub = client.peerSessionEvents.listen((session) {
      _peerSession = session;
      _isPeerConnecting =
          session.status == PeerConnectionStatus.connecting ||
          session.status == PeerConnectionStatus.handshaking;
      notifyListeners();
    });

    if (supervisor != null) {
      _supervisorState = supervisor!.state;
      _daemonInfo = client.daemonInfo;
      _supervisorSub = supervisor!.stateStream.listen((sState) {
        _supervisorState = sState;
        if (sState == SupervisorState.connected) {
          _protocolMismatchError = null;
          _daemonInfo = client.daemonInfo;
          refresh();
        } else if (sState == SupervisorState.protocolMismatch) {
          _protocolMismatchError = supervisor!.lastError;
        }
        notifyListeners();
      });

      _instanceSub = supervisor!.instanceChangedStream.listen((_) {
        // Reset downloads and sequence on daemon restart
        _downloads.clear();
        _daemonInfo = client.daemonInfo;
        refresh();
      });
    }
  }

  // Getters
  DaemonConnectionState get connectionState => _connectionState;
  SupervisorState get supervisorState => _supervisorState;
  String? get protocolMismatchError => _protocolMismatchError;
  DaemonInfo? get daemonInfo => _daemonInfo ?? client.daemonInfo;
  String? get errorMessage => _errorMessage;
  bool get isLoading => _isLoading;
  DownloadFilter get currentFilter => _currentFilter;
  AppNavSection get currentSection => _currentSection;
  String get searchQuery => _searchQuery;
  String? get selectedDownloadId => _selectedDownloadId;
  bool get isDebugBarVisible => isDevToolsAvailable && _isDebugBarVisible;
  String? get statusNotification => _statusNotification;

  void clearStatusNotification() {
    _statusNotification = null;
    notifyListeners();
  }

  List<CategoryItem> get categories => _categories;
  String? get selectedCategoryId => _selectedCategoryId;
  CategoryItem? get selectedCategory => _selectedCategoryId != null
      ? _categories.where((c) => c.id == _selectedCategoryId).firstOrNull
      : null;

  Map<String, int> get categoryCounts {
    final map = <String, int>{};
    for (final cat in _categories) {
      map[cat.id] = _downloads.values
          .where((d) => d.categoryId == cat.id)
          .length;
    }
    return map;
  }

  int get uncategorizedCount =>
      _downloads.values.where((d) => d.categoryId == null).length;

  List<HistoryEntry> get historyEntries => _historyEntries;
  HistoryStats get historyStats => _historyStats;
  bool get isHistoryLoading => _isHistoryLoading;
  String? get historyEventTypeFilter => _historyEventTypeFilter;
  String? get historyStatusFilter => _historyStatusFilter;
  int get historyRetentionDays => _historyRetentionDays;

  /// Lab and test fixture features are strictly dual-gated:
  /// Requires Portable workdir mode AND the daemon having the 'lab-tools' capability.
  bool get isLabModeAvailable {
    if (isDevToolsAvailable) return true;
    final info = daemonInfo;
    if (info == null) return false;
    final isPortable =
        (info.workdirMode?.toLowerCase() == 'portable') ||
        (supervisor?.resolveWorkDir().mode == WorkDirMode.portable);
    return isPortable && info.capabilities.contains('lab-tools');
  }

  Map<String, dynamic> _settings = {};
  Map<String, dynamic> get settings => _settings;

  Directory get defaultDownloadDirectory {
    final customDir = _settings['defaultDownloadDir'] as String?;
    if (customDir != null && customDir.isNotEmpty) {
      return Directory(customDir);
    }
    final daemonDir = daemonInfo?.defaultDownloadDir;
    if (daemonDir != null && daemonDir.isNotEmpty) {
      return Directory(daemonDir);
    }
    if (supervisor != null) {
      return supervisor!.resolveWorkDir().defaultDownloadDirectory;
    }
    return WorkDirectoryResolver.resolve().defaultDownloadDirectory;
  }

  Future<void> openDownloadsFolder() async {
    final dir = defaultDownloadDirectory;
    if (!dir.existsSync()) {
      try {
        dir.createSync(recursive: true);
      } catch (_) {}
    }
    if (Platform.isWindows) {
      await Process.start('explorer.exe', [dir.path]);
    } else if (Platform.isMacOS) {
      await Process.start('open', [dir.path]);
    } else if (Platform.isLinux) {
      await Process.start('xdg-open', [dir.path]);
    }
  }

  DownloadItem? get selectedDownload =>
      _selectedDownloadId != null ? _downloads[_selectedDownloadId] : null;

  int get totalDownloadSpeed => _downloads.values
      .where((d) => d.status.isActive)
      .fold<int>(0, (sum, d) => sum + d.downloadRateBps);

  /// Always 0 B/s in this offline / loopback lab client without public upload/seeding.
  int get totalUploadSpeed => 0;

  int get countAll => _downloads.length;
  int get countActive =>
      _downloads.values.where((d) => d.status.isActive).length;
  int get countPaused =>
      _downloads.values.where((d) => d.status.isPaused).length;
  int get countCompleted =>
      _downloads.values.where((d) => d.status.isCompleted).length;
  int get countFailed =>
      _downloads.values.where((d) => d.status.isFailed).length;

  List<DownloadItem> get filteredDownloads {
    final query = _searchQuery.trim().toLowerCase();
    return _downloads.values.where((item) {
      // Filter tab check
      switch (_currentFilter) {
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
      if (_selectedCategoryId != null) {
        if (_selectedCategoryId == 'uncategorized') {
          if (item.categoryId != null) return false;
        } else {
          if (item.categoryId != _selectedCategoryId) return false;
        }
      }

      return true;
    }).toList()..sort((a, b) => b.createdAt.compareTo(a.createdAt));
  }

  // Actions
  Future<void> initialize({Uri? url, String? token}) async {
    _isLoading = true;
    _errorMessage = null;
    notifyListeners();

    try {
      final targetUrl = url ?? Uri.parse('ws://127.0.0.1:4660/rpc');
      _daemonInfo = await client.connect(url: targetUrl, token: token);
      await refresh();
    } catch (e) {
      _errorMessage = e.toString();
    } finally {
      _isLoading = false;
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
      final list = await client.listDownloads();
      _downloads.clear();
      for (final item in list) {
        _downloads[item.id] = item;
      }
      _errorMessage = null;
    } catch (e) {
      _errorMessage = 'Refresh failed: $e';
    }
    notifyListeners();
  }

  void setSection(AppNavSection section) {
    _currentSection = section;
    if (section == AppNavSection.history) {
      fetchHistory();
    } else if (section == AppNavSection.servers) {
      fetchServers();
    } else if (section == AppNavSection.kad) {
      fetchKadStatus();
      fetchKadContacts();
    }
    notifyListeners();
  }

  void setFilter(DownloadFilter filter) {
    _currentSection = AppNavSection.downloads;
    _currentFilter = filter;
    notifyListeners();
  }

  void setSearchQuery(String query) {
    _searchQuery = query;
    notifyListeners();
  }

  void selectDownload(String? id) {
    _selectedDownloadId = id;
    if (id != null) {
      fetchDownloadDetail(id);
    }
    notifyListeners();
  }

  void toggleDebugBar() {
    if (!isDevToolsAvailable) return;
    _isDebugBarVisible = !_isDebugBarVisible;
    notifyListeners();
  }

  Future<bool> addEd2kLink(
    String link, {
    String? targetDirectory,
    String? allocationPolicy,
    String? categoryId,
  }) async {
    try {
      _isLoading = true;
      notifyListeners();

      final dlId = await client.addEd2kLink(
        link,
        targetDirectory: targetDirectory,
        allocationPolicy: allocationPolicy,
        categoryId: categoryId,
      );

      // Extract human-friendly filename from eD2K link for clean user notification
      String displayName = dlId;
      final tokens = link.split('|');
      if (tokens.length >= 4 && tokens[1].toLowerCase() == 'file') {
        final rawName = tokens[2];
        try {
          displayName = Uri.decodeComponent(rawName);
        } catch (_) {
          displayName = rawName;
        }
      }

      _statusNotification = 'Added "$displayName" to downloads';
      await refresh();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to add link: $e';
      return false;
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<PartMetInspectionResult> inspectPartMet(
    String metPath, {
    String? partPath,
  }) {
    return client.inspectPartMet(metPath, partPath: partPath);
  }

  Future<ImportPartMetResult?> importPartMet(
    String metPath, {
    String? partPath,
    String? targetDirectory,
    String? categoryId,
  }) async {
    try {
      _isLoading = true;
      notifyListeners();

      final res = await client.importPartMet(
        metPath,
        partPath: partPath,
        targetDirectory: targetDirectory,
        categoryId: categoryId,
      );

      _downloads[res.download.id] = res.download;
      _selectedDownloadId = res.download.id;
      _statusNotification =
          'Imported "${res.download.fileName}" (${res.verifiedParts}/${res.totalParts} verified, status: Paused)';
      await refresh();
      return res;
    } catch (e) {
      _errorMessage = 'Failed to import .part.met: $e';
      return null;
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<void> pauseDownload(String id) async {
    try {
      final updated = await client.pauseDownload(id);
      _downloads[updated.id] = updated;
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to pause: $e';
      notifyListeners();
    }
  }

  Future<void> resumeDownload(String id) async {
    try {
      final updated = await client.resumeDownload(id);
      _downloads[updated.id] = updated;
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to resume: $e';
      notifyListeners();
    }
  }

  Future<void> removeDownload(String id, {bool deleteFiles = false}) async {
    try {
      await client.removeDownload(id, deleteFiles: deleteFiles);
      _downloads.remove(id);
      if (_selectedDownloadId == id) {
        _selectedDownloadId = null;
      }
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to remove: $e';
      notifyListeners();
    }
  }

  Future<void> injectSimulationChunk(
    String id,
    int offset,
    String hexData,
  ) async {
    try {
      final updated = await client.injectRange(id, offset, hexData);
      _downloads[updated.id] = updated;
      _statusNotification =
          'Injected ${hexData.length ~/ 2} bytes at offset $offset';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Injection failed: $e';
      notifyListeners();
    }
  }

  Future<LabStatus> getLabStatus() async {
    return await client.getLabStatus();
  }

  Future<LabFixtureInfo> createLabFixture(String fixtureType) async {
    final fixture = await client.createLabFixture(fixtureType);
    _statusNotification = 'Created fixture ${fixture.fileName}';
    notifyListeners();
    return fixture;
  }

  Future<LabPeerInfo> startLabPeer() async {
    final peer = await client.startLabPeer();
    _statusNotification = 'Started loopback peer at ${peer.peerAddr}';
    notifyListeners();
    return peer;
  }

  Future<void> stopLabPeer() async {
    await client.stopLabPeer();
    _statusNotification = 'Stopped loopback test peer';
    notifyListeners();
  }

  Future<LabCleanupReport> cleanupLabTestData() async {
    final report = await client.cleanupLabTestData();
    _statusNotification = report.message;
    await refresh();
    return report;
  }

  Future<void> connectDownloadToSource(
    String downloadId,
    String peerAddr,
  ) async {
    await client.connectDownloadToSource(downloadId, peerAddr);
    _statusNotification = 'Connected download to $peerAddr';
    await refresh();
  }

  Future<void> fetchSettings() async {
    try {
      _settings = await client.getSettings();
      _historyRetentionDays =
          (_settings['historyRetentionDays'] as num?)?.toInt() ?? 30;
      notifyListeners();
    } catch (_) {
      // Non-critical in older or restricted daemons
    }
  }

  void setHistoryEventTypeFilter(String? eventType) {
    _historyEventTypeFilter = eventType;
    fetchHistory();
  }

  void setHistoryStatusFilter(String? status) {
    _historyStatusFilter = status;
    fetchHistory();
  }

  Future<void> fetchHistory() async {
    _isHistoryLoading = true;
    notifyListeners();
    try {
      final entries = await client.listHistory(
        eventType: _historyEventTypeFilter,
        status: _historyStatusFilter,
      );
      final stats = await client.getHistoryStats();
      _historyEntries = entries;
      _historyStats = stats;
    } catch (e) {
      _errorMessage = 'Failed to fetch history: $e';
    } finally {
      _isHistoryLoading = false;
      notifyListeners();
    }
  }

  Future<bool> clearHistory() async {
    try {
      _isHistoryLoading = true;
      notifyListeners();
      final count = await client.clearHistory();
      _historyEntries.clear();
      _historyStats = await client.getHistoryStats();
      _statusNotification = 'Cleared $count history record(s)';
      return true;
    } catch (e) {
      _errorMessage = 'Failed to clear history: $e';
      return false;
    } finally {
      _isHistoryLoading = false;
      notifyListeners();
    }
  }

  Future<String> exportHistory(String format) async {
    try {
      return await client.exportHistory(
        format,
        eventType: _historyEventTypeFilter,
        status: _historyStatusFilter,
      );
    } catch (e) {
      _errorMessage = 'Failed to export history: $e';
      notifyListeners();
      return '';
    }
  }

  Future<bool> setHistoryRetentionDays(int days) async {
    try {
      await client.setHistoryRetentionDays(days);
      _historyRetentionDays = days;
      _statusNotification = days == 0
          ? 'History retention set to unlimited'
          : 'History retention set to $days days';
      await fetchHistory();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to update retention policy: $e';
      notifyListeners();
      return false;
    }
  }

  Future<bool> setDefaultDownloadDirectory(String path) async {
    try {
      await client.setDefaultDownloadDirectory(path);
      await fetchSettings();
      _statusNotification = 'Updated default download directory';
      notifyListeners();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to set default download directory: $e';
      notifyListeners();
      return false;
    }
  }

  Future<bool> resetDefaultDownloadDirectory() async {
    try {
      await client.resetDefaultDownloadDirectory();
      await fetchSettings();
      _statusNotification = 'Reset download directory to portable default';
      notifyListeners();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to reset default download directory: $e';
      notifyListeners();
      return false;
    }
  }

  Future<DownloadItem?> fetchDownloadDetail(String id) async {
    try {
      final detail = await client.getDownload(id);
      if (detail != null) {
        _downloads[detail.id] = detail;
        notifyListeners();
      }
      return detail;
    } catch (e) {
      _errorMessage = 'Failed to fetch details: $e';
      notifyListeners();
      return null;
    }
  }

  Future<bool> changeDownloadDestination(String id, String newDirectory) async {
    try {
      _isLoading = true;
      notifyListeners();
      final updated = await client.setDownloadDestination(id, newDirectory);
      _downloads[updated.id] = updated;
      _statusNotification = 'Updated destination for "${updated.fileName}"';
      return true;
    } catch (e) {
      _errorMessage = 'Failed to set destination: $e';
      return false;
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<bool> retryDownloadConnection(String id) async {
    try {
      final updated = await client.retryDownloadConnection(id);
      _downloads[updated.id] = updated;
      _statusNotification =
          'Retrying loopback connection for "${updated.fileName}"';
      notifyListeners();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to retry connection: $e';
      notifyListeners();
      return false;
    }
  }

  Future<void> openCompletedFile(DownloadItem item) async {
    if (!item.status.isCompleted) return;
    final targetPath = item.targetPath;
    if (targetPath == null || targetPath.isEmpty) return;
    try {
      final file = File(targetPath);
      final dir = file.parent;
      if (Platform.isWindows) {
        if (file.existsSync()) {
          await Process.start('explorer.exe', ['/select,', file.path]);
        } else if (dir.existsSync()) {
          await Process.start('explorer.exe', [dir.path]);
        }
      } else if (Platform.isMacOS) {
        if (file.existsSync()) {
          await Process.start('open', ['-R', file.path]);
        } else if (dir.existsSync()) {
          await Process.start('open', [dir.path]);
        }
      } else if (Platform.isLinux) {
        if (dir.existsSync()) {
          await Process.start('xdg-open', [dir.path]);
        }
      }
    } catch (e) {
      _errorMessage = 'Could not open completed file location: $e';
      notifyListeners();
    }
  }

  String getRedactedDownloadDiagnostic(DownloadItem item) {
    String sanitizedTarget;
    try {
      final sep = Platform.isWindows ? r'\' : '/';
      sanitizedTarget = '[DOWNLOAD_DIR]$sep${item.fileName}';
    } catch (_) {
      sanitizedTarget = '[DOWNLOAD_DIR]/${item.fileName}';
    }

    final sanitizedTemp = 'data/temp/${item.id}.part';
    final totalParts = item.parts.length;
    final verifiedParts = item.parts.where((p) => p.isVerified).length;
    final completedParts = item.parts
        .where((p) => p.state == 'completed')
        .length;
    final downloadingParts = item.parts
        .where((p) => p.state == 'downloading')
        .length;
    final missingParts = item.parts.where((p) => p.state == 'missing').length;

    final buffer = StringBuffer();
    buffer.writeln('=== Emule-Sion Download Diagnostic (Sanitized) ===');
    buffer.writeln('Generated: ${DateTime.now().toUtc().toIso8601String()}');
    buffer.writeln(
      'Confinement: Loopback 127.0.0.1 (Zero WAN traffic, secrets redacted)',
    );
    buffer.writeln('Download ID: ${item.id}');
    buffer.writeln('File Name: ${item.fileName}');
    buffer.writeln('Status: ${item.status.label}');
    buffer.writeln('Size: ${item.totalBytes} bytes');
    buffer.writeln('Received: ${item.receivedBytes} bytes');
    buffer.writeln('Verified: ${item.verifiedBytes} bytes');
    buffer.writeln('Progress: ${(item.progress * 100).toStringAsFixed(1)}%');
    buffer.writeln('Current Speed: ${item.downloadRateBps} B/s');
    buffer.writeln('Average Speed: ${item.averageRateBps} B/s');
    if (item.etaSeconds != null) {
      buffer.writeln('ETA: ${item.etaSeconds}s');
    }
    buffer.writeln('Connection Attempts: ${item.connectionAttempts}');
    if (item.lastError != null) {
      buffer.writeln('Last Error: ${item.lastError}');
    }
    if (item.ed2kHash != null) {
      buffer.writeln('eD2K Hash: ${item.ed2kHash}');
    }
    if (item.ed2kLink != null) {
      buffer.writeln('Canonical Link: ${item.ed2kLink}');
    }
    buffer.writeln('Temp Path: $sanitizedTemp');
    buffer.writeln('Destination Target: $sanitizedTarget');
    buffer.writeln(
      'Part Map Summary: $totalParts parts total ($verifiedParts verified, $completedParts unverified, $downloadingParts downloading, $missingParts missing)',
    );
    buffer.writeln('Created At: ${item.createdAt.toIso8601String()}');
    buffer.writeln('Updated At: ${item.updatedAt.toIso8601String()}');
    return buffer.toString();
  }

  // --- Category Actions ---

  void selectCategory(String? categoryId) {
    _selectedCategoryId = categoryId;
    notifyListeners();
  }

  Future<void> fetchCategories() async {
    try {
      _categories = await client.listCategories();
      notifyListeners();
    } catch (e) {
      debugPrint('Failed to fetch categories: $e');
    }
  }

  Future<bool> createCategory({
    required String name,
    required String color,
    String? defaultDestination,
    String? defaultPriority,
  }) async {
    try {
      final cat = await client.createCategory(
        name: name,
        color: color,
        defaultDestination: defaultDestination,
        defaultPriority: defaultPriority,
      );
      await fetchCategories();
      _statusNotification = 'Created category "${cat.name}"';
      notifyListeners();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to create category: $e';
      notifyListeners();
      return false;
    }
  }

  Future<bool> updateCategory(
    String id, {
    String? name,
    String? color,
    String? defaultDestination,
    String? defaultPriority,
    int? sortOrder,
  }) async {
    try {
      final cat = await client.updateCategory(
        id,
        name: name,
        color: color,
        defaultDestination: defaultDestination,
        defaultPriority: defaultPriority,
        sortOrder: sortOrder,
      );
      await fetchCategories();
      _statusNotification = 'Updated category "${cat.name}"';
      notifyListeners();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to update category: $e';
      notifyListeners();
      return false;
    }
  }

  Future<bool> deleteCategory(String id) async {
    try {
      final cat = _categories.where((c) => c.id == id).firstOrNull;
      await client.deleteCategory(id);
      if (_selectedCategoryId == id) {
        _selectedCategoryId = null;
      }
      await refresh();
      _statusNotification = 'Deleted category "${cat?.name ?? id}"';
      notifyListeners();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to delete category: $e';
      notifyListeners();
      return false;
    }
  }

  Future<bool> reorderCategories(List<String> categoryIds) async {
    try {
      await client.reorderCategories(categoryIds);
      await fetchCategories();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to reorder categories: $e';
      notifyListeners();
      return false;
    }
  }

  Future<bool> setDownloadCategory(
    String downloadId,
    String? categoryId,
  ) async {
    try {
      final updated = await client.setDownloadCategory(downloadId, categoryId);
      _downloads[downloadId] = updated;
      final catName = categoryId != null
          ? (_categories.where((c) => c.id == categoryId).firstOrNull?.name ??
                'Category')
          : 'None';
      _statusNotification = 'Assigned to category "$catName"';
      notifyListeners();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to set category: $e';
      notifyListeners();
      return false;
    }
  }

  // Network Policy & Kill Switch
  NetworkPolicySnapshot? _networkPolicy;
  List<NetworkAuditEntryItem> _networkAuditLog = [];

  NetworkPolicySnapshot? get networkPolicy => _networkPolicy;
  List<NetworkAuditEntryItem> get networkAuditLog => _networkAuditLog;
  NetworkProfile get currentNetworkProfile =>
      _networkPolicy?.profile ?? NetworkProfile.publicEd2kKadPreview;
  NetworkSubsystemState get currentNetworkState =>
      _networkPolicy?.state ?? NetworkSubsystemState.previewActive;
  bool get isNetworkKillSwitchActive =>
      currentNetworkState == NetworkSubsystemState.disabled;

  Future<void> fetchNetworkPolicy() async {
    try {
      _networkPolicy = await client.getNetworkPolicy();
      notifyListeners();
    } catch (_) {}
  }

  Future<void> setNetworkProfile(NetworkProfile profile) async {
    try {
      _networkPolicy = await client.setNetworkProfile(profile);
      _statusNotification = 'Switched network profile to ${profile.label}';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to switch network profile: $e';
      notifyListeners();
    }
  }

  Future<void> disconnectAllNetwork() async {
    try {
      await client.disconnectAll();
      await fetchNetworkPolicy();
      _serverSession = ServerSessionModel.disconnected;
      _statusNotification =
          'Kill Switch engaged: All network connections disconnected';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Kill switch failed: $e';
      notifyListeners();
    }
  }

  Future<void> enableNetwork() async {
    try {
      await client.enableNetwork();
      await fetchNetworkPolicy();
      _statusNotification = 'Network policy re-enabled';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to re-enable network: $e';
      notifyListeners();
    }
  }

  Future<void> fetchNetworkAuditLog({int limit = 50}) async {
    try {
      _networkAuditLog = await client.getNetworkAuditLog(limit: limit);
      notifyListeners();
    } catch (_) {}
  }

  Future<void> setResourceBudgets(ResourceBudgetsModel budgets) async {
    try {
      _networkPolicy = await client.setResourceBudgets(budgets);
      _statusNotification = 'Resource budgets updated';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to set resource budgets: $e';
      notifyListeners();
    }
  }

  // IP Filter & Rate Limiter
  FilterStatusModel? _filterStatus;
  RateLimitsModel? _rateLimits;

  FilterStatusModel? get filterStatus => _filterStatus;
  RateLimitsModel? get rateLimits => _rateLimits;

  Future<void> fetchFilterStatus() async {
    try {
      _filterStatus = await client.getFilterStatus();
      notifyListeners();
    } catch (_) {}
  }

  Future<bool> importFilterFile(String filePath) async {
    try {
      _filterStatus = await client.importFilterFile(filePath);
      _statusNotification =
          'Imported ${_filterStatus?.rangeCount ?? 0} IP filter ranges';
      notifyListeners();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to import IP filter: $e';
      notifyListeners();
      return false;
    }
  }

  Future<void> setFilterEnabled(bool enabled) async {
    try {
      _filterStatus = await client.setFilterEnabled(enabled);
      _statusNotification = enabled
          ? 'IP Filter enabled'
          : 'IP Filter disabled';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to toggle IP filter: $e';
      notifyListeners();
    }
  }

  Future<void> clearFilter() async {
    try {
      _filterStatus = await client.clearFilter();
      _statusNotification = 'IP Filter cleared';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to clear IP filter: $e';
      notifyListeners();
    }
  }

  Future<void> fetchRateLimits() async {
    try {
      _rateLimits = await client.getRateLimits();
      notifyListeners();
    } catch (_) {}
  }

  Future<void> setRateLimits(RateLimitsModel limits) async {
    try {
      _rateLimits = await client.setRateLimits(limits);
      _statusNotification = 'Rate limits updated';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Failed to set rate limits: $e';
      notifyListeners();
    }
  }

  List<Ed2kServerModel> get servers => _servers;
  bool get isServersLoading => _isServersLoading;
  ServerSessionModel get serverSession => _serverSession;
  bool get isServerConnecting => _isServerConnecting;

  Future<void> fetchServers() async {
    _isServersLoading = true;
    notifyListeners();
    try {
      _servers = await client.listServers();
      _errorMessage = null;
    } catch (e) {
      _errorMessage = 'Failed to list servers: $e';
    } finally {
      _isServersLoading = false;
      notifyListeners();
    }
  }

  Future<ServerInspectReport?> inspectServerMet(String filePath) async {
    try {
      final report = await client.inspectServerMet(filePath);
      return report;
    } catch (e) {
      _errorMessage = 'Failed to inspect server.met: $e';
      notifyListeners();
      return null;
    }
  }

  Future<bool> importServerMet(String filePath) async {
    try {
      final result = await client.importServerMet(filePath);
      _statusNotification =
          'Imported ${result.imported} servers (${result.allowedCount} allowed, ${result.blockedCount} blocked by filter)';
      await fetchServers();
      return true;
    } catch (e) {
      _errorMessage = 'Failed to import server.met: $e';
      notifyListeners();
      return false;
    }
  }

  Future<Map<String, dynamic>?> updateServersFromUrl(String url) async {
    _isServersLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final result = await client.updateServersFromUrl(url);
      final count = result['serversCount'] ?? result['imported'] ?? 0;
      _statusNotification =
          'Mise à jour réussie : $count serveurs traités depuis l\'URL';
      await fetchServers();
      return result;
    } catch (e) {
      _errorMessage = 'Échec de la mise à jour des serveurs depuis l\'URL : $e';
      notifyListeners();
      return null;
    } finally {
      _isServersLoading = false;
      notifyListeners();
    }
  }

  Future<void> setServerEnabled(String address, bool enabled) async {
    try {
      await client.setServerEnabled(address, enabled);
      await fetchServers();
    } catch (e) {
      _errorMessage = 'Failed to toggle server state: $e';
      notifyListeners();
    }
  }

  Future<void> deleteServer(String address) async {
    try {
      await client.deleteServer(address);
      await fetchServers();
    } catch (e) {
      _errorMessage = 'Failed to delete server: $e';
      notifyListeners();
    }
  }

  Future<void> clearServers() async {
    try {
      final count = await client.clearServers();
      _statusNotification = 'Cleared $count servers';
      await fetchServers();
    } catch (e) {
      _errorMessage = 'Failed to clear servers: $e';
      notifyListeners();
    }
  }

  // ── eD2K Server Session ─────────────────────────────────────────────────────

  Future<void> connectServer(String address) async {
    if (_isServerConnecting) return;
    _isServerConnecting = true;
    _errorMessage = null;
    notifyListeners();
    try {
      _serverSession = await client.connectServer(address);
      _statusNotification =
          'Connexion au serveur ${_serverSession.serverName ?? address}...';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Connexion impossible : $e';
      _serverSession = ServerSessionModel.disconnected;
      notifyListeners();
    } finally {
      _isServerConnecting = false;
      notifyListeners();
    }
  }

  Future<void> disconnectServer() async {
    try {
      _serverSession = await client.disconnectServer();
      _statusNotification = 'Déconnecté du serveur eD2K';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Erreur de déconnexion : $e';
      notifyListeners();
    }
  }

  Future<void> refreshServerSession() async {
    try {
      _serverSession = await client.getServerStatus();
      notifyListeners();
    } catch (_) {}
  }

  // ── eD2K Server Search & Sources ──────────────────────────────────────────

  List<SearchResultFileModel> get searchResults => _searchResults;
  bool get isSearching => _isSearching;
  String? get lastSearchTerm => _lastSearchTerm;

  Future<void> searchServer({
    required String term,
    int? minSize,
    int? maxSize,
    String? extension,
  }) async {
    final trimmed = term.trim();
    if (trimmed.isEmpty) return;
    _isSearching = true;
    _searchResults.clear();
    _lastSearchTerm = trimmed;
    _errorMessage = null;
    notifyListeners();

    try {
      await client.searchServer(
        term: trimmed,
        minSize: minSize,
        maxSize: maxSize,
        extension: extension,
      );
      _statusNotification = 'Recherche "$trimmed" envoyée au serveur';
    } catch (e) {
      _errorMessage = 'Échec de la recherche : $e';
      _isSearching = false;
    } finally {
      notifyListeners();
    }
  }

  Future<void> searchKadKeywords({
    required String term,
    int? minSize,
    int? maxSize,
  }) async {
    final trimmed = term.trim();
    if (trimmed.isEmpty) return;
    _isSearching = true;
    _searchResults.clear();
    _lastSearchTerm = trimmed;
    _errorMessage = null;
    notifyListeners();

    try {
      final results = await client.searchKadKeywords(
        term: trimmed,
        minSize: minSize,
        maxSize: maxSize,
      );
      final existingHashes = _searchResults.map((f) => f.fileHash).toSet();
      for (final f in results) {
        if (!existingHashes.contains(f.fileHash)) {
          _searchResults.add(f);
          existingHashes.add(f.fileHash);
        }
      }
      _statusNotification =
          'Recherche Kad "$trimmed" terminée (${_searchResults.length} résultat(s))';
    } catch (e) {
      _errorMessage = 'Échec de la recherche Kad : $e';
    } finally {
      _isSearching = false;
      notifyListeners();
    }
  }

  void clearSearchResults() {
    _searchResults.clear();
    _isSearching = false;
    _lastSearchTerm = null;
    notifyListeners();
  }

  Future<bool> downloadSearchResult(
    SearchResultFileModel file, {
    String? categoryId,
    String? targetDirectory,
  }) async {
    return addEd2kLink(
      file.ed2kLink,
      categoryId: categoryId,
      targetDirectory: targetDirectory,
    );
  }

  Future<void> queryServerSources(String fileHash, {int? fileSize}) async {
    try {
      await client.queryServerSources(fileHash: fileHash, fileSize: fileSize);
      _statusNotification = 'Demande de sources envoyée pour $fileHash';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Échec de la demande de sources : $e';
      notifyListeners();
    }
  }

  // --- Peer Sources & Session ---

  bool get isSourcesLoading => _isSourcesLoading;
  PeerSessionModel get peerSession => _peerSession;
  bool get isPeerConnecting => _isPeerConnecting;

  List<PeerSourceModel> getSourcesForHash(String fileHash) {
    return _peerSources[fileHash] ?? [];
  }

  Future<void> loadSourcesForHash(String fileHash) async {
    _isSourcesLoading = true;
    notifyListeners();
    try {
      final sources = await client.listSources(fileHash);
      _peerSources[fileHash] = sources;
    } catch (e) {
      _errorMessage = 'Échec du chargement des sources : $e';
    } finally {
      _isSourcesLoading = false;
      notifyListeners();
    }
  }

  Future<bool> addManualSource(String fileHash, String address) async {
    try {
      final src = await client.addManualSource(fileHash, address);
      final list = _peerSources.putIfAbsent(fileHash, () => []);
      list.removeWhere((s) => s.address == address);
      list.insert(0, src);
      _statusNotification = 'Source $address ajoutée';
      notifyListeners();
      return true;
    } catch (e) {
      _errorMessage = 'Échec de l\'ajout de la source : $e';
      notifyListeners();
      return false;
    }
  }

  Future<bool> connectPeerSource(
    String fileHash,
    String address, {
    bool force = false,
  }) async {
    _isPeerConnecting = true;
    _errorMessage = null;
    notifyListeners();
    try {
      _peerSession = await client.connectPeerSource(
        fileHash,
        address,
        force: force,
      );
      _statusNotification = 'Connexion à $address initialisée';
      await loadSourcesForHash(fileHash);
      return true;
    } catch (e) {
      _errorMessage = 'Échec de la connexion à la source : $e';
      await loadSourcesForHash(fileHash);
      return false;
    } finally {
      _isPeerConnecting = false;
      notifyListeners();
    }
  }

  Future<void> disconnectPeerSource({String? address}) async {
    try {
      _peerSession = await client.disconnectPeerSource(address: address);
      _statusNotification = address != null
          ? 'Peer $address déconnecté'
          : 'Session(s) peer déconnectée(s)';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Échec de la déconnexion peer : $e';
      notifyListeners();
    }
  }

  Future<void> clearSourcesForHash(String fileHash) async {
    try {
      await client.clearSources(fileHash);
      _peerSources.remove(fileHash);
      _statusNotification = 'Sources effacées';
      notifyListeners();
    } catch (e) {
      _errorMessage = 'Échec de l\'effacement des sources : $e';
      notifyListeners();
    }
  }

  // --- Kad DHT (Phase 12.0a) ---

  KadStatusModel get kadStatus => _kadStatus;
  List<KadContactModel> get kadContacts => _kadContacts;
  bool get isKadLoading => _isKadLoading;

  Future<void> fetchKadStatus() async {
    try {
      _kadStatus = await client.getKadStatus();
      notifyListeners();
    } catch (e) {
      debugPrint('Failed to fetch Kad status: $e');
    }
  }

  Future<void> fetchKadContacts({int limit = 100}) async {
    _isKadLoading = true;
    notifyListeners();
    try {
      _kadContacts = await client.listKadContacts(limit: limit);
      _errorMessage = null;
    } catch (e) {
      _errorMessage = 'Échec de la récupération des contacts Kad : $e';
    } finally {
      _isKadLoading = false;
      notifyListeners();
    }
  }

  Future<NodesDatInspectReportModel?> inspectNodesDat(String filePath) async {
    try {
      return await client.inspectNodesDat(filePath);
    } catch (e) {
      _errorMessage = 'Échec de l\'inspection de nodes.dat : $e';
      notifyListeners();
      return null;
    }
  }

  Future<bool> importNodesDat(String filePath) async {
    _isKadLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final res = await client.importNodesDat(filePath);
      final count = res['importedCount'] ?? 0;
      _statusNotification =
          'Importé $count contacts Kad dans la table K-buckets';
      await fetchKadStatus();
      await fetchKadContacts();
      return true;
    } catch (e) {
      _errorMessage = 'Échec de l\'importation de nodes.dat : $e';
      notifyListeners();
      return false;
    } finally {
      _isKadLoading = false;
      notifyListeners();
    }
  }

  Future<void> clearKadContacts() async {
    try {
      final count = await client.clearKadContacts();
      _statusNotification = 'Effacé $count contacts Kad';
      await fetchKadStatus();
      await fetchKadContacts();
    } catch (e) {
      _errorMessage = 'Échec de l\'effacement des contacts Kad : $e';
      notifyListeners();
    }
  }

  Future<bool> startKadUdp({String? bindAddress}) async {
    _isKadLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      await client.startKadUdp(bindAddress: bindAddress);
      _statusNotification =
          'Socket UDP Kad démarré sur ${bindAddress ?? "127.0.0.1:39737"}';
      await fetchKadStatus();
      return true;
    } catch (e) {
      _errorMessage = 'Échec du démarrage UDP Kad : $e';
      notifyListeners();
      return false;
    } finally {
      _isKadLoading = false;
      notifyListeners();
    }
  }

  Future<bool> stopKadUdp() async {
    _isKadLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      await client.stopKadUdp();
      _statusNotification = 'Socket UDP Kad arrêté';
      await fetchKadStatus();
      return true;
    } catch (e) {
      _errorMessage = 'Échec de l\'arrêt UDP Kad : $e';
      notifyListeners();
      return false;
    } finally {
      _isKadLoading = false;
      notifyListeners();
    }
  }

  Future<int?> pingKadContact({
    required String ip,
    required int port,
    String? kadId,
  }) async {
    try {
      final res = await client.pingKadContact(ip: ip, port: port, kadId: kadId);
      final latency = (res['latencyMs'] as num?)?.toInt() ?? 0;
      _statusNotification = 'Pong reçu de $ip:$port en ${latency}ms';
      await fetchKadStatus();
      await fetchKadContacts();
      return latency;
    } catch (e) {
      _errorMessage = 'Échec du ping Kad ($ip:$port) : $e';
      notifyListeners();
      return null;
    }
  }

  Future<bool> bootstrapKad({required String ip, required int port}) async {
    _isKadLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final res = await client.bootstrapKad(ip: ip, port: port);
      final count = (res['receivedContacts'] as num?)?.toInt() ?? 0;
      _statusNotification = 'Amorçage Kad réussi ($count contacts reçus)';
      await fetchKadStatus();
      await fetchKadContacts();
      return true;
    } catch (e) {
      _errorMessage = 'Échec de l\'amorçage Kad : $e';
      notifyListeners();
      return false;
    } finally {
      _isKadLoading = false;
      notifyListeners();
    }
  }

  Future<bool> bootstrapKadFromUrl(String url) async {
    _isKadLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final res = await client.bootstrapKadFromUrl(url);
      final count = (res['importedNodes'] as num?)?.toInt() ?? 0;
      _statusNotification =
          'Amorçage Kad depuis URL réussi ($count nœuds importés)';
      await fetchKadStatus();
      await fetchKadContacts();
      return true;
    } catch (e) {
      _errorMessage = 'Échec de l\'amorçage Kad depuis URL : $e';
      notifyListeners();
      return false;
    } finally {
      _isKadLoading = false;
      notifyListeners();
    }
  }

  Future<List<KadContactModel>> findKadNode(String targetId) async {
    _isKadLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final contacts = await client.findKadNode(targetId);
      _statusNotification =
          'Recherche Kad : ${contacts.length} nœuds les plus proches trouvés';
      await fetchKadStatus();
      await fetchKadContacts();
      return contacts;
    } catch (e) {
      _errorMessage = 'Échec de la recherche Kad : $e';
      notifyListeners();
      return const [];
    } finally {
      _isKadLoading = false;
      notifyListeners();
    }
  }

  /// Searches Kad DHT for sources of a specific file hash and auto-injects them.
  Future<List<PeerSourceModel>> searchKadSources(
    String fileHash, {
    int? fileSize,
  }) async {
    _isKadLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final sources = await client.searchKadSources(
        fileHash: fileHash,
        fileSize: fileSize,
      );
      _statusNotification =
          'Kad : ${sources.length} sources trouvées pour ${fileHash.substring(0, 8)}…';
      notifyListeners();
      return sources;
    } catch (e) {
      _errorMessage = 'Échec de la recherche de sources Kad : $e';
      notifyListeners();
      return const [];
    } finally {
      _isKadLoading = false;
      notifyListeners();
    }
  }

  /// Publishes this node as a source for a file hash to Kad DHT.
  Future<int> publishKadSource(
    String fileHash,
    int fileSize, {
    int tcpPort = 39735,
  }) async {
    _isKadLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final count = await client.publishKadSource(
        fileHash: fileHash,
        fileSize: fileSize,
        tcpPort: tcpPort,
      );
      _statusNotification =
          'Kad : publié sur $count nœuds pour ${fileHash.substring(0, 8)}…';
      notifyListeners();
      return count;
    } catch (e) {
      _errorMessage = 'Échec de la publication Kad : $e';
      notifyListeners();
      return 0;
    } finally {
      _isKadLoading = false;
      notifyListeners();
    }
  }

  void _startKadStatusTimer() {
    _kadStatusTimer?.cancel();
    _kadStatusTimer = Timer.periodic(const Duration(seconds: 3), (_) async {
      if (_connectionState == DaemonConnectionState.connected) {
        await fetchKadStatus();
        if (_currentSection == AppNavSection.kad && !_isKadLoading) {
          try {
            _kadContacts = await client.listKadContacts(limit: 100);
            notifyListeners();
          } catch (_) {}
        }
      }
    });
  }

  @override
  void dispose() {
    _kadStatusTimer?.cancel();
    _connSub?.cancel();
    _serverSessionSub?.cancel();
    _eventSub?.cancel();
    _supervisorSub?.cancel();
    _instanceSub?.cancel();
    _searchResultSub?.cancel();
    _foundSourcesSub?.cancel();
    _peerSessionSub?.cancel();
    supervisor?.dispose();
    client.dispose();
    super.dispose();
  }
}
