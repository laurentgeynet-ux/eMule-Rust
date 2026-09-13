import 'dart:async';
import 'package:flutter/foundation.dart';

import '../../../../domain/models/daemon_event.dart';
import '../../../../domain/models/daemon_info.dart';
import '../../../../domain/models/download_item.dart';
import '../../../../domain/services/mule_daemon_client.dart';

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

  DaemonConnectionState _connectionState = DaemonConnectionState.disconnected;
  DaemonInfo? _daemonInfo;
  String? _errorMessage;
  bool _isLoading = false;

  final Map<String, DownloadItem> _downloads = {};
  DownloadFilter _currentFilter = DownloadFilter.all;
  String _searchQuery = '';
  String? _selectedDownloadId;
  bool _isDebugBarVisible = false;
  String? _statusNotification;

  StreamSubscription<DaemonConnectionState>? _connSub;
  StreamSubscription<DownloadUpdatedEvent>? _eventSub;

  DownloadsViewModel({required this.client}) {
    _connSub = client.connectionState.listen((state) {
      _connectionState = state;
      notifyListeners();
    });

    _eventSub = client.downloadEvents.listen((event) {
      _downloads[event.download.id] = event.download;
      notifyListeners();
    });
  }

  // Getters
  DaemonConnectionState get connectionState => _connectionState;
  DaemonInfo? get daemonInfo => _daemonInfo;
  String? get errorMessage => _errorMessage;
  bool get isLoading => _isLoading;
  DownloadFilter get currentFilter => _currentFilter;
  String get searchQuery => _searchQuery;
  String? get selectedDownloadId => _selectedDownloadId;
  bool get isDebugBarVisible => _isDebugBarVisible;
  String? get statusNotification => _statusNotification;

  DownloadItem? get selectedDownload =>
      _selectedDownloadId != null ? _downloads[_selectedDownloadId] : null;

  int get totalDownloadSpeed => _downloads.values
      .where((d) => d.status.isActive)
      .fold<int>(0, (sum, d) => sum + d.downloadRateBps);

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

  void setFilter(DownloadFilter filter) {
    _currentFilter = filter;
    notifyListeners();
  }

  void setSearchQuery(String query) {
    _searchQuery = query;
    notifyListeners();
  }

  void selectDownload(String? id) {
    _selectedDownloadId = id;
    notifyListeners();
  }

  void toggleDebugBar() {
    _isDebugBarVisible = !_isDebugBarVisible;
    notifyListeners();
  }

  Future<bool> addEd2kLink(
    String link, {
    String? targetDirectory,
    String? allocationPolicy,
  }) async {
    try {
      _isLoading = true;
      notifyListeners();

      final dlId = await client.addEd2kLink(
        link,
        targetDirectory: targetDirectory,
        allocationPolicy: allocationPolicy,
      );

      _statusNotification = 'Added download $dlId';
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

  @override
  void dispose() {
    _connSub?.cancel();
    _eventSub?.cancel();
    client.dispose();
    super.dispose();
  }
}
