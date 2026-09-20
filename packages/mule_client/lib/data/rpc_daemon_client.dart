import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:web_socket_channel/web_socket_channel.dart';

import '../../domain/models/category_item.dart';
import '../../domain/models/daemon_event.dart';
import '../../domain/models/daemon_info.dart';
import '../../domain/models/download_item.dart';
import '../../domain/models/filter_models.dart';
import '../../domain/models/history_entry.dart';
import '../../domain/models/history_stats.dart';
import '../../domain/models/lab_models.dart';
import '../../domain/models/network_policy_models.dart';
import '../../domain/models/part_met_models.dart';
import '../../domain/models/server_models.dart';
import '../../domain/services/mule_daemon_client.dart';
import 'dto/rpc_dto.dart';

class RpcDaemonClient implements MuleDaemonClient {
  WebSocketChannel? _channel;
  StreamSubscription? _subscription;

  int _nextId = 1;
  int _lastSequence = 0;
  DaemonConnectionState _state = DaemonConnectionState.disconnected;
  DaemonInfo? _daemonInfo;

  Uri? _url;
  String? _token;
  bool _explicitDisconnect = false;
  int _connectGeneration = 0;
  Timer? _reconnectTimer;
  Timer? _keepAliveTimer;
  int _reconnectAttempts = 0;

  final Map<int, Completer<dynamic>> _pendingRequests = {};
  final StreamController<DaemonConnectionState> _connectionController =
      StreamController<DaemonConnectionState>.broadcast();
  final StreamController<DownloadUpdatedEvent> _eventsController =
      StreamController<DownloadUpdatedEvent>.broadcast();
  final StreamController<List<SearchResultFileModel>> _searchResultController =
      StreamController<List<SearchResultFileModel>>.broadcast();
  final StreamController<Map<String, dynamic>> _foundSourcesController =
      StreamController<Map<String, dynamic>>.broadcast();
  final StreamController<PeerSessionModel> _peerSessionController =
      StreamController<PeerSessionModel>.broadcast();
  final StreamController<ServerSessionModel> _serverSessionController =
      StreamController<ServerSessionModel>.broadcast();

  // Factory constructor or provider callback for custom channel (useful for testing)
  final WebSocketChannel Function(Uri uri)? channelFactory;

  /// When true, the client retries the last `connect()` automatically on
  /// connection loss (exponential backoff, capped at 10 s). Desktop leaves
  /// this off because `DaemonSupervisor` already orchestrates reconnects and
  /// daemon relaunch; remote clients (mobile/web) should enable it.
  final bool autoReconnect;

  /// Periodic `system.status` ping that detects dead connections whose TCP
  /// close frame never arrived (NAT timeouts, suspend/resume). Null disables.
  final Duration keepAliveInterval;

  RpcDaemonClient({
    this.channelFactory,
    this.autoReconnect = false,
    this.keepAliveInterval = const Duration(seconds: 20),
  });

  @override
  DaemonConnectionState get currentConnectionState => _state;

  @override
  Stream<DaemonConnectionState> get connectionState =>
      _connectionController.stream;

  @override
  Stream<DownloadUpdatedEvent> get downloadEvents => _eventsController.stream;

  @override
  Stream<List<SearchResultFileModel>> get searchResultEvents =>
      _searchResultController.stream;

  @override
  Stream<Map<String, dynamic>> get foundSourcesEvents =>
      _foundSourcesController.stream;

  @override
  Stream<PeerSessionModel> get peerSessionEvents =>
      _peerSessionController.stream;

  @override
  Stream<ServerSessionModel> get serverSessionEvents =>
      _serverSessionController.stream;

  @override
  DaemonInfo? get daemonInfo => _daemonInfo;
  int get lastSequence => _lastSequence;

  @override
  Future<DaemonInfo> connect({required Uri url, String? token}) async {
    _url = url;
    _token = token;
    _explicitDisconnect = false;
    final generation = ++_connectGeneration;
    _setConnectionState(DaemonConnectionState.connecting);

    try {
      final factory = channelFactory;
      final channel = factory != null
          ? factory(url)
          : WebSocketChannel.connect(url);
      _channel = channel;

      _subscription = channel.stream.listen(
        _handleIncomingMessage,
        onError: (err) {
          _handleConnectionLost(generation, errored: true);
        },
        onDone: () {
          _handleConnectionLost(generation, errored: false);
        },
      );

      // Ensure underlying socket connection succeeds before proceeding
      await channel.ready.timeout(const Duration(seconds: 2));

      // Perform Handshake
      final helloResp = await _call('system.hello', {
        'protocolVersion': 1,
        'token': token ?? '',
      });

      _daemonInfo = RpcDto.daemonInfoFromJson(
        helloResp as Map<String, dynamic>,
      );

      // Subscribe to real-time events
      await _call('events.subscribe', {});

      _setConnectionState(DaemonConnectionState.connected);
      _reconnectAttempts = 0;
      _startKeepAlive(generation);

      // After a reconnect, the daemon may have emitted events we missed:
      // pull a fresh snapshot so the UI catches up.
      if (_lastSequence > 0) {
        unawaited(_resync());
      }
      return _daemonInfo!;
    } catch (e) {
      if (generation != _connectGeneration || _explicitDisconnect) rethrow;
      _setConnectionState(DaemonConnectionState.error);
      await _teardownChannel();
      // Preserve pre-reconnect semantics: a failed connect ends disconnected.
      _setConnectionState(DaemonConnectionState.disconnected);
      rethrow;
    }
  }

  /// Connection dropped or failed. With [autoReconnect], schedules a retry
  /// against the last `connect()` target; otherwise surfaces the state.
  void _handleConnectionLost(int generation, {required bool errored}) {
    if (generation != _connectGeneration) return; // stale socket
    _keepAliveTimer?.cancel();
    for (final c in _pendingRequests.values) {
      if (!c.isCompleted) {
        c.completeError(StateError('Connection lost'));
      }
    }
    _pendingRequests.clear();

    if (_explicitDisconnect) {
      _setConnectionState(DaemonConnectionState.disconnected);
      return;
    }
    _setConnectionState(
      errored ? DaemonConnectionState.error : DaemonConnectionState.disconnected,
    );
    if (autoReconnect) _scheduleReconnect(generation);
  }

  void _scheduleReconnect(int generation) {
    _reconnectTimer?.cancel();
    _reconnectAttempts++;
    final delayMs =
        _reconnectAttempts <= 4 ? 500 * (1 << (_reconnectAttempts - 1)) : 10000;
    _reconnectTimer = Timer(Duration(milliseconds: delayMs), () async {
      if (generation != _connectGeneration || _explicitDisconnect) return;
      final url = _url;
      if (url == null) return;
      try {
        await connect(url: url, token: _token);
      } catch (_) {
        // connect() already surfaced the error state; the next retry is
        // scheduled only on socket loss, so retry here.
        if (autoReconnect && !_explicitDisconnect) {
          _scheduleReconnect(generation);
        }
      }
    });
  }

  void _startKeepAlive(int generation) {
    _keepAliveTimer?.cancel();
    _keepAliveTimer = Timer.periodic(keepAliveInterval, (_) async {
      if (generation != _connectGeneration) return;
      try {
        await _call('system.status');
      } catch (_) {
        // Ping failed: close the socket to trigger the onDone → reconnect path.
        try {
          await _channel?.sink.close();
        } catch (_) {}
      }
    });
  }

  /// Refreshes the download snapshot and replays it as update events so
  /// listeners rebuild their state (used after gaps and reconnects).
  Future<void> _resync() async {
    try {
      final items = await listDownloads();
      for (final item in items) {
        if (!_eventsController.isClosed) {
          _eventsController.add(
            DownloadUpdatedEvent(sequence: _lastSequence, download: item),
          );
        }
      }
    } catch (_) {}
  }

  /// Closes the socket and fails pending requests without marking the
  /// disconnect as explicit (used by keepalive failure and reconnect).
  Future<void> _teardownChannel() async {
    await _subscription?.cancel();
    _subscription = null;
    try {
      await _channel?.sink.close().timeout(
        const Duration(milliseconds: 500),
        onTimeout: () {},
      );
    } catch (_) {}
    _channel = null;
    for (final c in _pendingRequests.values) {
      if (!c.isCompleted) {
        c.completeError(StateError('Client disconnected'));
      }
    }
    _pendingRequests.clear();
  }

  void _setConnectionState(DaemonConnectionState state) {
    if (_state != state) {
      _state = state;
      if (!_connectionController.isClosed) {
        _connectionController.add(_state);
      }
    }
  }

  void _handleIncomingMessage(dynamic rawMessage) {
    try {
      final Map<String, dynamic> data =
          jsonDecode(rawMessage.toString()) as Map<String, dynamic>;

      // Check if this is a response to a request with an id
      if (data.containsKey('id') && data['id'] != null) {
        final id = data['id'];
        final int? reqId = id is int ? id : int.tryParse(id.toString());
        if (reqId != null && _pendingRequests.containsKey(reqId)) {
          final completer = _pendingRequests.remove(reqId)!;
          if (data['error'] != null) {
            final err = data['error'];
            final msg = err is Map ? err['message'] : err.toString();
            final code = err is Map ? err['code'] : 0;
            completer.completeError(
              RpcException(
                code: code ?? 0,
                message: msg?.toString() ?? 'RPC Error',
              ),
            );
          } else {
            completer.complete(data['result']);
          }
          return;
        }
      }

      // Check if this is a streaming notification (method: event.downloadUpdated)
      final method = data['method'] as String?;
      if (method == 'event.downloadUpdated') {
        final params = data['params'] as Map<String, dynamic>?;
        if (params != null) {
          final seq = (params['sequence'] as num?)?.toInt() ?? 0;
          if (seq > _lastSequence) {
            // Gap detection: if seq > lastSequence + 1, we missed messages; trigger resync
            if (_lastSequence > 0 && seq > _lastSequence + 1) {
              // Trigger automatic list resync in background
              unawaited(listDownloads());
            }
            _lastSequence = seq;

            final dlJson = params['download'] as Map<String, dynamic>?;
            if (dlJson != null) {
              final item = RpcDto.downloadFromSnapshotJson(dlJson);
              if (!_eventsController.isClosed) {
                _eventsController.add(
                  DownloadUpdatedEvent(sequence: seq, download: item),
                );
              }
            }
          }
        }
      } else if (method == 'server.searchResult') {
        final params = data['params'] as Map<String, dynamic>?;
        if (params != null) {
          final rawFiles = params['files'] as List<dynamic>? ?? [];
          final files = rawFiles
              .whereType<Map<String, dynamic>>()
              .map((j) => SearchResultFileModel.fromJson(j))
              .toList();
          if (!_searchResultController.isClosed) {
            _searchResultController.add(files);
          }
        }
      } else if (method == 'server.foundSources') {
        final params = data['params'] as Map<String, dynamic>?;
        if (params != null && !_foundSourcesController.isClosed) {
          _foundSourcesController.add(params);
        }
      } else if (method == 'event.peerSessionUpdated') {
        final params = data['params'] as Map<String, dynamic>?;
        if (params != null) {
          final sessionJson = params['session'] as Map<String, dynamic>?;
          if (sessionJson != null && !_peerSessionController.isClosed) {
            _peerSessionController.add(PeerSessionModel.fromJson(sessionJson));
          }
        }
      } else if (method == 'server.sessionUpdated') {
        final params = data['params'] as Map<String, dynamic>?;
        if (params != null) {
          final sessionJson = params['session'] as Map<String, dynamic>?;
          if (sessionJson != null && !_serverSessionController.isClosed) {
            _serverSessionController.add(
              ServerSessionModel.fromJson(sessionJson),
            );
          }
        }
      } else if (method == 'event.snapshotRequired') {
        // Server broadcast buffer lagged — events were dropped.
        unawaited(_resync());
      }
    } catch (_) {
      // Ignore unparseable frames
    }
  }

  Future<dynamic> _call(
    String method, [
    Map<String, dynamic> params = const {},
  ]) {
    final channel = _channel;
    if (channel == null) {
      return Future.error(StateError('Not connected to daemon'));
    }

    final id = _nextId++;
    final completer = Completer<dynamic>();
    _pendingRequests[id] = completer;

    final requestPayload = jsonEncode({
      'jsonrpc': '2.0',
      'id': id,
      'method': method,
      'params': params,
    });

    channel.sink.add(requestPayload);

    return completer.future.timeout(
      const Duration(seconds: 15),
      onTimeout: () {
        _pendingRequests.remove(id);
        throw TimeoutException('RPC request $method (id: $id) timed out');
      },
    );
  }

  @override
  Future<List<DownloadItem>> listDownloads() async {
    final resp = await _call('downloads.list', {});
    if (resp is Map<String, dynamic>) {
      final seq = (resp['sequence'] as num?)?.toInt() ?? 0;
      _lastSequence = max(_lastSequence, seq);

      final rawList = resp['downloads'] as List<dynamic>? ?? const [];
      return rawList
          .map(
            (e) => RpcDto.downloadFromSnapshotJson(e as Map<String, dynamic>),
          )
          .toList();
    }
    return [];
  }

  @override
  Future<DownloadItem?> getDownload(String id) async {
    final resp = await _call('downloads.get', {'id': id});
    if (resp is Map<String, dynamic>) {
      return RpcDto.downloadFromSnapshotJson(resp);
    }
    return null;
  }

  @override
  Future<String> addEd2kLink(
    String link, {
    String? targetDirectory,
    String? allocationPolicy,
    String? categoryId,
  }) async {
    final params = <String, dynamic>{
      'link': link,
      'targetDirectory': targetDirectory ?? '.',
      'allocationPolicy': allocationPolicy ?? 'sparse',
    };
    if (categoryId != null && categoryId.isNotEmpty) {
      params['categoryId'] = categoryId;
    }
    final resp = await _call('downloads.addEd2kLink', params);

    if (resp is Map<String, dynamic> && resp.containsKey('downloadId')) {
      return resp['downloadId'] as String;
    }
    throw Exception('Unexpected response adding eD2K link: $resp');
  }

  @override
  Future<DownloadItem> pauseDownload(String id) async {
    final resp = await _call('downloads.pause', {'id': id});
    return RpcDto.downloadFromSnapshotJson(resp as Map<String, dynamic>);
  }

  @override
  Future<DownloadItem> resumeDownload(String id) async {
    final resp = await _call('downloads.resume', {'id': id});
    return RpcDto.downloadFromSnapshotJson(resp as Map<String, dynamic>);
  }

  @override
  Future<void> removeDownload(String id, {bool deleteFiles = false}) async {
    await _call('downloads.remove', {'id': id, 'deleteFiles': deleteFiles});
  }

  @override
  Future<DownloadItem> injectRange(
    String id,
    int offset,
    String hexData,
  ) async {
    final resp = await _call('debug.injectRange', {
      'id': id,
      'offset': offset,
      'dataHex': hexData,
    });
    return RpcDto.downloadFromSnapshotJson(resp as Map<String, dynamic>);
  }

  @override
  Future<void> stopDaemon() async {
    await _call('system.stop', {});
  }

  @override
  Future<void> disconnect() async {
    _explicitDisconnect = true;
    _connectGeneration++;
    _reconnectTimer?.cancel();
    _keepAliveTimer?.cancel();
    await _teardownChannel();
    _setConnectionState(DaemonConnectionState.disconnected);
  }

  @override
  Future<LabStatus> getLabStatus() async {
    final resp = await _call('lab.status', {});
    return LabStatus.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<LabFixtureInfo> createLabFixture(String fixtureType) async {
    final resp = await _call('lab.createFixture', {'fixtureType': fixtureType});
    return LabFixtureInfo.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<LabPeerInfo> startLabPeer() async {
    final resp = await _call('lab.startLoopbackPeer', {});
    return LabPeerInfo.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<void> stopLabPeer() async {
    await _call('lab.stopLoopbackPeer', {});
  }

  @override
  Future<LabCleanupReport> cleanupLabTestData() async {
    final resp = await _call('lab.cleanupTestData', {});
    return LabCleanupReport.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<void> connectDownloadToSource(
    String downloadId,
    String peerAddr,
  ) async {
    try {
      await _call('ed2k.probeSource', {
        'downloadId': downloadId,
        'peerAddr': peerAddr,
      });
      await _call('ed2k.startDownload', {
        'downloadId': downloadId,
        'peerAddr': peerAddr,
      });
    } catch (_) {
      await _call('lab.addPeerSource', {
        'downloadId': downloadId,
        'peerAddr': peerAddr,
      });
    }
  }

  @override
  Future<DownloadItem> setDownloadDestination(
    String id,
    String targetDirectory,
  ) async {
    final resp = await _call('downloads.setDestination', {
      'id': id,
      'targetDirectory': targetDirectory,
    });
    return RpcDto.downloadFromSnapshotJson(resp as Map<String, dynamic>);
  }

  @override
  Future<DownloadItem> retryDownloadConnection(String id) async {
    final resp = await _call('downloads.retryConnection', {'id': id});
    return RpcDto.downloadFromSnapshotJson(resp as Map<String, dynamic>);
  }

  @override
  Future<Map<String, dynamic>> getSettings() async {
    final resp = await _call('settings.get', {});
    return resp as Map<String, dynamic>;
  }

  @override
  Future<void> setDefaultDownloadDirectory(String path) async {
    await _call('settings.set', {'defaultDownloadDir': path});
  }

  @override
  Future<void> resetDefaultDownloadDirectory() async {
    await _call('settings.resetDownloadDir', {});
  }

  @override
  Future<List<HistoryEntry>> listHistory({
    String? eventType,
    String? status,
    int? fromTimestampMs,
    int? toTimestampMs,
    int? limit,
    int? offset,
  }) async {
    final params = <String, dynamic>{};
    if (eventType != null) params['eventType'] = eventType;
    if (status != null) params['status'] = status;
    if (fromTimestampMs != null) params['fromTimestampMs'] = fromTimestampMs;
    if (toTimestampMs != null) params['toTimestampMs'] = toTimestampMs;
    if (limit != null) params['limit'] = limit;
    if (offset != null) params['offset'] = offset;

    final resp = await _call('history.list', params);
    if (resp is List) {
      return resp
          .map((e) => RpcDto.historyEntryFromJson(e as Map<String, dynamic>))
          .toList();
    }
    return const [];
  }

  @override
  Future<HistoryStats> getHistoryStats() async {
    final resp = await _call('history.stats', {});
    return RpcDto.historyStatsFromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<int> clearHistory() async {
    final resp = await _call('history.clear', {});
    if (resp is Map<String, dynamic>) {
      return (resp['clearedCount'] as num?)?.toInt() ?? 0;
    }
    return 0;
  }

  @override
  Future<String> exportHistory(
    String format, {
    String? eventType,
    String? status,
  }) async {
    final params = <String, dynamic>{'format': format};
    if (eventType != null) params['eventType'] = eventType;
    if (status != null) params['status'] = status;

    final resp = await _call('history.export', params);
    if (resp is Map<String, dynamic>) {
      return resp['content'] as String? ?? '';
    }
    return '';
  }

  @override
  Future<void> setHistoryRetentionDays(int days) async {
    await _call('settings.setRetentionDays', {'retentionDays': days});
  }

  @override
  Future<List<CategoryItem>> listCategories() async {
    final resp = await _call('categories.list');
    if (resp is List<dynamic>) {
      return resp
          .map((e) => RpcDto.categoryFromJson(e as Map<String, dynamic>))
          .toList();
    }
    return [];
  }

  @override
  Future<CategoryItem> createCategory({
    required String name,
    required String color,
    String? defaultDestination,
    String? defaultPriority,
  }) async {
    final params = <String, dynamic>{'name': name, 'color': color};
    if (defaultDestination != null && defaultDestination.isNotEmpty) {
      params['defaultDestination'] = defaultDestination;
    }
    if (defaultPriority != null && defaultPriority.isNotEmpty) {
      params['defaultPriority'] = defaultPriority;
    }

    final resp = await _call('categories.create', params);
    return RpcDto.categoryFromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<CategoryItem> updateCategory(
    String id, {
    String? name,
    String? color,
    String? defaultDestination,
    String? defaultPriority,
    int? sortOrder,
  }) async {
    final params = <String, dynamic>{'id': id};
    if (name != null) params['name'] = name;
    if (color != null) params['color'] = color;
    if (defaultDestination != null) {
      params['defaultDestination'] = defaultDestination;
    }
    if (defaultPriority != null) params['defaultPriority'] = defaultPriority;
    if (sortOrder != null) params['sortOrder'] = sortOrder;

    final resp = await _call('categories.update', params);
    return RpcDto.categoryFromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<void> deleteCategory(String id) async {
    await _call('categories.delete', {'id': id});
  }

  @override
  Future<void> reorderCategories(List<String> categoryIds) async {
    await _call('categories.reorder', {'categoryIds': categoryIds});
  }

  @override
  Future<DownloadItem> setDownloadCategory(
    String downloadId,
    String? categoryId,
  ) async {
    final resp = await _call('downloads.setCategory', {
      'id': downloadId,
      'categoryId': categoryId,
    });
    return RpcDto.downloadFromSnapshotJson(resp as Map<String, dynamic>);
  }

  @override
  Future<PartMetInspectionResult> inspectPartMet(
    String metPath, {
    String? partPath,
  }) async {
    final params = <String, dynamic>{'metPath': metPath};
    if (partPath != null && partPath.isNotEmpty) {
      params['partPath'] = partPath;
    }
    final resp = await _call('downloads.inspectPartMet', params);
    return PartMetInspectionResult.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<ImportPartMetResult> importPartMet(
    String metPath, {
    String? partPath,
    String? targetDirectory,
    String? categoryId,
  }) async {
    final params = <String, dynamic>{'metPath': metPath};
    if (partPath != null && partPath.isNotEmpty) {
      params['partPath'] = partPath;
    }
    if (targetDirectory != null && targetDirectory.isNotEmpty) {
      params['targetDirectory'] = targetDirectory;
    }
    if (categoryId != null && categoryId.isNotEmpty) {
      params['categoryId'] = categoryId;
    }
    final resp = await _call('downloads.importPartMet', params);
    return ImportPartMetResult.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<NetworkPolicySnapshot> getNetworkPolicy() async {
    final resp = await _call('network.getPolicy');
    return NetworkPolicySnapshot.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<NetworkPolicySnapshot> setNetworkProfile(
    NetworkProfile profile,
  ) async {
    final resp = await _call('network.setProfile', {
      'profile': profile.toKebabCase(),
    });
    return NetworkPolicySnapshot.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<void> disconnectAll() async {
    await _call('network.disconnectAll');
  }

  @override
  Future<void> enableNetwork() async {
    await _call('network.enable');
  }

  @override
  Future<List<NetworkAuditEntryItem>> getNetworkAuditLog({
    int limit = 50,
  }) async {
    final resp = await _call('network.getAuditLog', {'limit': limit});
    final list =
        (resp as Map<String, dynamic>)['entries'] as List<dynamic>? ?? [];
    return list
        .map((e) => NetworkAuditEntryItem.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<NetworkPolicySnapshot> setResourceBudgets(
    ResourceBudgetsModel budgets,
  ) async {
    final resp = await _call('network.setBudgets', {
      'budgets': budgets.toJson(),
    });
    return NetworkPolicySnapshot.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<FilterStatusModel> getFilterStatus() async {
    final resp = await _call('filter.getStatus');
    return FilterStatusModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<FilterStatusModel> importFilterFile(String filePath) async {
    final resp = await _call('filter.importFile', {'filePath': filePath});
    return FilterStatusModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<FilterStatusModel> setFilterEnabled(bool enabled) async {
    final resp = await _call('filter.setEnabled', {'enabled': enabled});
    return FilterStatusModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<FilterStatusModel> clearFilter() async {
    final resp = await _call('filter.clear');
    return FilterStatusModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<RateLimitsModel> getRateLimits() async {
    final resp = await _call('limits.get');
    return RateLimitsModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<RateLimitsModel> setRateLimits(RateLimitsModel limits) async {
    final resp = await _call('limits.set', limits.toJson());
    return RateLimitsModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<ServerInspectReport> inspectServerMet(String filePath) async {
    final resp = await _call('servers.inspectMet', {'filePath': filePath});
    return ServerInspectReport.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<ServerImportResult> importServerMet(String filePath) async {
    final resp = await _call('servers.importMet', {'filePath': filePath});
    return ServerImportResult.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<List<Ed2kServerModel>> listServers() async {
    final resp = await _call('servers.list');
    final rawList =
        (resp as Map<String, dynamic>)['servers'] as List<dynamic>? ?? const [];
    return rawList
        .map((e) => Ed2kServerModel.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<bool> setServerEnabled(String address, bool enabled) async {
    final resp = await _call('servers.setEnabled', {
      'address': address,
      'enabled': enabled,
    });
    return (resp as Map<String, dynamic>)['updated'] as bool? ?? false;
  }

  @override
  Future<bool> deleteServer(String address) async {
    final resp = await _call('servers.delete', {'address': address});
    return (resp as Map<String, dynamic>)['deleted'] as bool? ?? false;
  }

  @override
  Future<int> clearServers() async {
    final resp = await _call('servers.clear');
    return (resp as Map<String, dynamic>)['deletedCount'] as int? ?? 0;
  }

  @override
  Future<ServerSessionModel> connectServer(String address) async {
    final resp = await _call('servers.connect', {'address': address});
    return ServerSessionModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<ServerSessionModel> disconnectServer() async {
    final resp = await _call('servers.disconnect');
    return ServerSessionModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<ServerSessionModel> getServerStatus() async {
    final resp = await _call('servers.status');
    return ServerSessionModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<Map<String, dynamic>> searchServer({
    required String term,
    int? minSize,
    int? maxSize,
    String? extension,
    bool udpOnly = false,
  }) async {
    final params = <String, dynamic>{'term': term};
    if (minSize != null) params['minSize'] = minSize;
    if (maxSize != null) params['maxSize'] = maxSize;
    if (extension != null) params['extension'] = extension;
    if (udpOnly) params['udpOnly'] = true;
    final resp = await _call('servers.search', params);
    return resp is Map<String, dynamic> ? resp : <String, dynamic>{};
  }

  @override
  Future<void> queryServerSources({
    required String fileHash,
    int? fileSize,
  }) async {
    final params = <String, dynamic>{'fileHash': fileHash};
    if (fileSize != null) params['fileSize'] = fileSize;
    await _call('servers.querySources', params);
  }

  @override
  Future<List<PeerSourceModel>> listSources(String fileHash) async {
    final resp = await _call('sources.list', {'fileHash': fileHash});
    final list = resp as List<dynamic>? ?? [];
    return list
        .whereType<Map<String, dynamic>>()
        .map((j) => PeerSourceModel.fromJson(j))
        .toList();
  }

  @override
  Future<PeerSourceModel> addManualSource(
    String fileHash,
    String address,
  ) async {
    final resp = await _call('sources.addManual', {
      'fileHash': fileHash,
      'address': address,
    });
    return PeerSourceModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<PeerSessionModel> connectPeerSource(
    String fileHash,
    String address, {
    bool force = false,
  }) async {
    final params = <String, dynamic>{'fileHash': fileHash, 'address': address};
    if (force) {
      params['force'] = true;
    }
    final resp = await _call('sources.connect', params);
    return PeerSessionModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<PeerSessionModel> disconnectPeerSource({String? address}) async {
    final params = address != null
        ? <String, dynamic>{'address': address}
        : const <String, dynamic>{};
    final resp = await _call('sources.disconnect', params);
    return PeerSessionModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<PeerSessionModel> getPeerSessionStatus() async {
    final resp = await _call('sources.status');
    return PeerSessionModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<SwarmStatusModel> getSwarmStatus({String? fileHash}) async {
    final params = fileHash != null
        ? <String, dynamic>{'fileHash': fileHash}
        : const <String, dynamic>{};
    final resp = await _call('sources.swarmStatus', params);
    return SwarmStatusModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<int> clearSources(String fileHash) async {
    final resp = await _call('sources.clear', {'fileHash': fileHash});
    return (resp as Map<String, dynamic>)['removed'] as int? ?? 0;
  }

  @override
  Future<List<SharedFileModel>> listSharedFiles() async {
    final resp = await _call('shares.list', {});
    if (resp is List) {
      return resp
          .map((e) => SharedFileModel.fromJson(e as Map<String, dynamic>))
          .toList();
    }
    return [];
  }

  @override
  Future<SharedFileModel> addSharedFile({
    required String fileHash,
    required String fileName,
    required int fileSize,
    required String filePath,
  }) async {
    final resp = await _call('shares.add', {
      'fileHash': fileHash,
      'fileName': fileName,
      'fileSize': fileSize,
      'filePath': filePath,
    });
    return SharedFileModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<void> removeSharedFile(String fileHash) async {
    await _call('shares.remove', {'fileHash': fileHash});
  }

  @override
  Future<SharedFileModel> setSharedFileEnabled(
    String fileHash,
    bool enabled,
  ) async {
    final resp = await _call('shares.setEnabled', {
      'fileHash': fileHash,
      'enabled': enabled,
    });
    return SharedFileModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<InboundListenerStatusModel> startListener({int port = 0}) async {
    final resp = await _call('listener.start', {'port': port});
    return InboundListenerStatusModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<InboundListenerStatusModel> stopListener() async {
    final resp = await _call('listener.stop', {});
    return InboundListenerStatusModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<InboundListenerStatusModel> getListenerStatus() async {
    final resp = await _call('listener.status', {});
    return InboundListenerStatusModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<Map<String, dynamic>> listUploads() async {
    final resp = await _call('uploads.list', {});
    return resp as Map<String, dynamic>? ?? {};
  }

  @override
  Future<UploadSessionModel?> getUploadStatus() async {
    final resp = await _call('uploads.status', {});
    if (resp == null || resp is! Map<String, dynamic>) {
      return null;
    }
    return UploadSessionModel.fromJson(resp);
  }

  @override
  Future<void> cancelUpload(String clientAddress) async {
    await _call('uploads.cancel', {'clientAddress': clientAddress});
  }

  @override
  Future<SecureIdentityStatusModel> getIdentityStatus() async {
    final resp = await _call('identity.status', {});
    return SecureIdentityStatusModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<SecureIdentityStatusModel> rotateIdentity() async {
    final resp = await _call('identity.rotate', {});
    return SecureIdentityStatusModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<List<PeerCreditModel>> listPeerCredits({int limit = 50}) async {
    final resp = await _call('credits.list', {'limit': limit});
    final list =
        (resp as Map<String, dynamic>?)?['credits'] as List<dynamic>? ?? [];
    return list
        .whereType<Map<String, dynamic>>()
        .map((j) => PeerCreditModel.fromJson(j))
        .toList();
  }

  @override
  Future<PeerCreditStatsModel> getPeerCreditStats() async {
    final resp = await _call('credits.stats', {});
    return PeerCreditStatsModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<KadStatusModel> getKadStatus() async {
    final resp = await _call('kad.status', {});
    return KadStatusModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<Map<String, dynamic>> startKadUdp({String? bindAddress}) async {
    final params = bindAddress != null
        ? {'bindAddress': bindAddress}
        : <String, dynamic>{};
    final resp = await _call('kad.startUdp', params);
    return resp as Map<String, dynamic>? ?? {};
  }

  @override
  Future<Map<String, dynamic>> stopKadUdp() async {
    final resp = await _call('kad.stopUdp', {});
    return resp as Map<String, dynamic>? ?? {};
  }

  @override
  Future<Map<String, dynamic>> pingKadContact({
    required String ip,
    required int port,
    String? kadId,
  }) async {
    final params = <String, dynamic>{'ip': ip, 'port': port};
    if (kadId != null) {
      params['kadId'] = kadId;
    }
    final resp = await _call('kad.ping', params);
    return resp as Map<String, dynamic>? ?? {};
  }

  @override
  Future<Map<String, dynamic>> bootstrapKad({
    required String ip,
    required int port,
  }) async {
    final resp = await _call('kad.bootstrap', {'ip': ip, 'port': port});
    return resp as Map<String, dynamic>? ?? {};
  }

  @override
  Future<List<KadContactModel>> findKadNode(String targetId) async {
    final resp = await _call('kad.findNode', {'targetId': targetId});
    final list =
        (resp as Map<String, dynamic>?)?['contacts'] as List<dynamic>? ?? [];
    return list
        .whereType<Map<String, dynamic>>()
        .map((j) => KadContactModel.fromJson(j))
        .toList();
  }

  @override
  Future<NodesDatInspectReportModel> inspectNodesDat(String filePath) async {
    final resp = await _call('kad.inspectNodesDat', {'path': filePath});
    return NodesDatInspectReportModel.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<Map<String, dynamic>> importNodesDat(String filePath) async {
    final resp = await _call('kad.importNodesDat', {'path': filePath});
    return resp as Map<String, dynamic>? ?? {};
  }

  @override
  Future<List<KadContactModel>> listKadContacts({int limit = 100}) async {
    final resp = await _call('kad.listContacts', {'limit': limit});
    final list =
        (resp as Map<String, dynamic>?)?['contacts'] as List<dynamic>? ?? [];
    return list
        .whereType<Map<String, dynamic>>()
        .map((j) => KadContactModel.fromJson(j))
        .toList();
  }

  @override
  Future<int> clearKadContacts() async {
    final resp = await _call('kad.clearContacts', {});
    return (resp as Map<String, dynamic>?)?['cleared'] as int? ?? 0;
  }

  @override
  Future<List<PeerSourceModel>> searchKadSources({
    required String fileHash,
    int? fileSize,
  }) async {
    final resp = await _call('kad.searchSources', {
      'fileHash': fileHash,
      'fileSize': ?fileSize,
    });
    final list =
        (resp as Map<String, dynamic>?)?['sources'] as List<dynamic>? ?? [];
    return list
        .whereType<Map<String, dynamic>>()
        .map((j) => PeerSourceModel.fromJson(j))
        .toList();
  }

  @override
  Future<List<SearchResultFileModel>> searchKadKeywords({
    required String term,
    int? minSize,
    int? maxSize,
  }) async {
    final resp = await _call('kad.searchKeywords', {
      'term': term,
      'minSize': ?minSize,
      'maxSize': ?maxSize,
    });
    final list =
        (resp as Map<String, dynamic>?)?['results'] as List<dynamic>? ?? [];
    return list
        .whereType<Map<String, dynamic>>()
        .map((j) => SearchResultFileModel.fromJson(j))
        .toList();
  }

  @override
  Future<int> publishKadSource({
    required String fileHash,
    required int fileSize,
    int tcpPort = 39735,
  }) async {
    final resp = await _call('kad.publishSource', {
      'fileHash': fileHash,
      'fileSize': fileSize,
      'tcpPort': tcpPort,
    });
    return (resp as Map<String, dynamic>?)?['publishedCount'] as int? ?? 0;
  }

  @override
  Future<int> publishKadKeywords({
    required String fileHash,
    required String fileName,
    required int fileSize,
  }) async {
    final resp = await _call('kad.publishKeywords', {
      'fileHash': fileHash,
      'fileName': fileName,
      'fileSize': fileSize,
    });
    return (resp as Map<String, dynamic>?)?['publishedCount'] as int? ?? 0;
  }

  @override
  Future<UpnpStatusModel> getUpnpStatus() async {
    final resp = await _call('upnp.status', {});
    return UpnpStatusModel.fromJson(resp as Map<String, dynamic>? ?? {});
  }

  @override
  Future<UpnpStatusModel> setUpnpEnabled(bool enabled) async {
    final resp = await _call('upnp.setEnabled', {'enabled': enabled});
    return UpnpStatusModel.fromJson(resp as Map<String, dynamic>? ?? {});
  }

  @override
  Future<UpnpStatusModel> mapUpnpPorts({
    int tcpPort = 39735,
    int udpPort = 39737,
  }) async {
    final resp = await _call('upnp.mapPorts', {
      'tcpPort': tcpPort,
      'udpPort': udpPort,
    });
    return UpnpStatusModel.fromJson(resp as Map<String, dynamic>? ?? {});
  }

  @override
  Future<UpnpStatusModel> unmapUpnpPorts() async {
    final resp = await _call('upnp.unmapPorts', {});
    return UpnpStatusModel.fromJson(resp as Map<String, dynamic>? ?? {});
  }

  @override
  Future<Map<String, dynamic>> updateServersFromUrl(String url) async {
    final resp = await _call('servers.updateFromUrl', {'url': url});
    return resp as Map<String, dynamic>? ?? {};
  }

  @override
  Future<Map<String, dynamic>> bootstrapKadFromUrl(String url) async {
    final resp = await _call('kad.bootstrapFromUrl', {'url': url});
    return resp as Map<String, dynamic>? ?? {};
  }

  @override
  void dispose() {
    _explicitDisconnect = true;
    _connectGeneration++;
    _reconnectTimer?.cancel();
    _keepAliveTimer?.cancel();
    disconnect();
    _connectionController.close();
    _eventsController.close();
    _searchResultController.close();
    _foundSourcesController.close();
    _peerSessionController.close();
    _serverSessionController.close();
  }
}

abstract class RpcErrorCode {
  static const int parseError = -32700;
  static const int invalidRequest = -32600;
  static const int methodNotFound = -32601;
  static const int invalidParams = -32602;
  static const int internalError = -32603;

  // Application and security error codes (-32000 .. -32099)
  static const int authenticationFailed = -32000;
  static const int protocolMismatch = -32001;
  static const int forbiddenInInstalledMode = -32002;
  static const int unsafeCleanupTarget = -32003;
  static const int fixtureDownloadMismatch = -32004;
  static const int loopbackPeerUnavailable = -32005;
  static const int businessError = -32010;
}

class RpcException implements Exception {
  final int code;
  final String message;

  const RpcException({required this.code, required this.message});

  @override
  String toString() => 'RpcException(code: $code, message: $message)';
}
