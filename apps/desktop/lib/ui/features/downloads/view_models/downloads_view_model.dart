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

enum AppNavSection {
  downloads,
  history,
  servers,
}

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

  StreamSubscription<DaemonConnectionState>? _connSub;
  StreamSubscription<DownloadUpdatedEvent>? _eventSub;
  StreamSubscription<SupervisorState>? _supervisorSub;
  StreamSubscription<void>? _instanceSub;

  DownloadsViewModel({
    required this.client,
    this.supervisor,
    this.isDevToolsAvailable = false,
  }) {
    _connSub = client.connectionState.listen((state) {
      _connectionState = state;
      notifyListeners();
    });

    _eventSub = client.downloadEvents.listen((event) {
      _downloads[event.download.id] = event.download;
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
      map[cat.id] = _downloads.values.where((d) => d.categoryId == cat.id).length;
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
    final isPortable = (info.workdirMode?.toLowerCase() == 'portable') ||
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
  int get countActive => _downloads.values.where((d) => d.status.isActive).length;
  int get countPaused => _downloads.values.where((d) => d.status.isPaused).length;
  int get countCompleted => _downloads.values.where((d) => d.status.isCompleted).length;
  int get countFailed => _downloads.values.where((d) => d.status.isFailed).length;

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
    }).toList()
      ..sort((a, b) => b.createdAt.compareTo(a.createdAt));
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

  Future<PartMetInspectionResult> inspectPartMet(String metPath, {String? partPath}) {
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

  Future<void> injectSimulationChunk(String id, int offset, String hexData) async {
    try {
      final updated = await client.injectRange(id, offset, hexData);
      _downloads[updated.id] = updated;
      _statusNotification = 'Injected ${hexData.length ~/ 2} bytes at offset $offset';
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

  Future<void> connectDownloadToSource(String downloadId, String peerAddr) async {
    await client.connectDownloadToSource(downloadId, peerAddr);
    _statusNotification = 'Connected download to $peerAddr';
    await refresh();
  }

  Future<void> fetchSettings() async {
    try {
      _settings = await client.getSettings();
      _historyRetentionDays = (_settings['historyRetentionDays'] as num?)?.toInt() ?? 30;
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
      _statusNotification = 'Retrying loopback connection for "${updated.fileName}"';
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
    final completedParts = item.parts.where((p) => p.state == 'completed').length;
    final downloadingParts = item.parts.where((p) => p.state == 'downloading').length;
    final missingParts = item.parts.where((p) => p.state == 'missing').length;

    final buffer = StringBuffer();
    buffer.writeln('=== Emule-Sion Download Diagnostic (Sanitized) ===');
    buffer.writeln('Generated: ${DateTime.now().toUtc().toIso8601String()}');
    buffer.writeln('Confinement: Loopback 127.0.0.1 (Zero WAN traffic, secrets redacted)');
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
    buffer.writeln('Part Map Summary: $totalParts parts total ($verifiedParts verified, $completedParts unverified, $downloadingParts downloading, $missingParts missing)');
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

  Future<bool> setDownloadCategory(String downloadId, String? categoryId) async {
    try {
      final updated = await client.setDownloadCategory(downloadId, categoryId);
      _downloads[downloadId] = updated;
      final catName = categoryId != null
          ? (_categories.where((c) => c.id == categoryId).firstOrNull?.name ?? 'Category')
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
      _networkPolicy?.profile ?? NetworkProfile.localLab;
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
      _statusNotification = 'Kill Switch engaged: All network connections disconnected';
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
      _statusNotification = 'Imported ${_filterStatus?.rangeCount ?? 0} IP filter ranges';
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
      _statusNotification = enabled ? 'IP Filter enabled' : 'IP Filter disabled';
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

  @override
  void dispose() {
    _connSub?.cancel();
    _eventSub?.cancel();
    _supervisorSub?.cancel();
    _instanceSub?.cancel();
    supervisor?.dispose();
    client.dispose();
    super.dispose();
  }
}
