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
import '../../domain/services/mule_daemon_client.dart';
import 'dto/rpc_dto.dart';

class RpcDaemonClient implements MuleDaemonClient {
  WebSocketChannel? _channel;
  StreamSubscription? _subscription;

  int _nextId = 1;
  int _lastSequence = 0;
  DaemonConnectionState _state = DaemonConnectionState.disconnected;
  DaemonInfo? _daemonInfo;

  final Map<int, Completer<dynamic>> _pendingRequests = {};
  final StreamController<DaemonConnectionState> _connectionController =
      StreamController<DaemonConnectionState>.broadcast();
  final StreamController<DownloadUpdatedEvent> _eventsController =
      StreamController<DownloadUpdatedEvent>.broadcast();

  // Factory constructor or provider callback for custom channel (useful for testing)
  final WebSocketChannel Function(Uri uri)? channelFactory;

  RpcDaemonClient({this.channelFactory});

  @override
  DaemonConnectionState get currentConnectionState => _state;

  @override
  Stream<DaemonConnectionState> get connectionState =>
      _connectionController.stream;

  @override
  Stream<DownloadUpdatedEvent> get downloadEvents => _eventsController.stream;

  @override
  DaemonInfo? get daemonInfo => _daemonInfo;
  int get lastSequence => _lastSequence;

  @override
  Future<DaemonInfo> connect({required Uri url, String? token}) async {
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
          _setConnectionState(DaemonConnectionState.error);
        },
        onDone: () {
          _setConnectionState(DaemonConnectionState.disconnected);
        },
      );

      // Ensure underlying socket connection succeeds before proceeding
      await channel.ready.timeout(const Duration(seconds: 2));

      // Perform Handshake
      final helloResp = await _call('system.hello', {
        'protocolVersion': 1,
        'token': token ?? '',
      });

      _daemonInfo = RpcDto.daemonInfoFromJson(helloResp as Map<String, dynamic>);

      // Subscribe to real-time events
      await _call('events.subscribe', {});

      _setConnectionState(DaemonConnectionState.connected);
      return _daemonInfo!;
    } catch (e) {
      _setConnectionState(DaemonConnectionState.error);
      await disconnect();
      rethrow;
    }
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
                RpcException(code: code ?? 0, message: msg?.toString() ?? 'RPC Error'));
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
                _eventsController.add(DownloadUpdatedEvent(
                  sequence: seq,
                  download: item,
                ));
              }
            }
          }
        }
      }
    } catch (_) {
      // Ignore unparseable frames
    }
  }

  Future<dynamic> _call(String method, [Map<String, dynamic> params = const {}]) {
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
          .map((e) => RpcDto.downloadFromSnapshotJson(e as Map<String, dynamic>))
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
    await _call('downloads.remove', {
      'id': id,
      'deleteFiles': deleteFiles,
    });
  }

  @override
  Future<DownloadItem> injectRange(String id, int offset, String hexData) async {
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
    await _subscription?.cancel();
    _subscription = null;

    try {
      await _channel?.sink.close().timeout(const Duration(milliseconds: 500), onTimeout: () {});
    } catch (_) {}
    _channel = null;

    for (final c in _pendingRequests.values) {
      if (!c.isCompleted) {
        c.completeError(StateError('Client disconnected'));
      }
    }
    _pendingRequests.clear();

    _setConnectionState(DaemonConnectionState.disconnected);
  }

  @override
  Future<LabStatus> getLabStatus() async {
    final resp = await _call('lab.status', {});
    return LabStatus.fromJson(resp as Map<String, dynamic>);
  }

  @override
  Future<LabFixtureInfo> createLabFixture(String fixtureType) async {
    final resp = await _call('lab.createFixture', {
      'fixtureType': fixtureType,
    });
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
  Future<void> connectDownloadToSource(String downloadId, String peerAddr) async {
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
  Future<DownloadItem> setDownloadDestination(String id, String targetDirectory) async {
    final resp = await _call('downloads.setDestination', {
      'id': id,
      'targetDirectory': targetDirectory,
    });
    return RpcDto.downloadFromSnapshotJson(resp as Map<String, dynamic>);
  }

  @override
  Future<DownloadItem> retryDownloadConnection(String id) async {
    final resp = await _call('downloads.retryConnection', {
      'id': id,
    });
    return RpcDto.downloadFromSnapshotJson(resp as Map<String, dynamic>);
  }

  @override
  Future<Map<String, dynamic>> getSettings() async {
    final resp = await _call('settings.get', {});
    return resp as Map<String, dynamic>;
  }

  @override
  Future<void> setDefaultDownloadDirectory(String path) async {
    await _call('settings.set', {
      'defaultDownloadDir': path,
    });
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
  Future<String> exportHistory(String format, {String? eventType, String? status}) async {
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
    await _call('settings.setRetentionDays', {
      'retentionDays': days,
    });
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
    final params = <String, dynamic>{
      'name': name,
      'color': color,
    };
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
    if (defaultDestination != null) params['defaultDestination'] = defaultDestination;
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
  Future<DownloadItem> setDownloadCategory(String downloadId, String? categoryId) async {
    final resp = await _call('downloads.setCategory', {
      'id': downloadId,
      'categoryId': categoryId,
    });
    return RpcDto.downloadFromSnapshotJson(resp as Map<String, dynamic>);
  }

  @override
  Future<PartMetInspectionResult> inspectPartMet(String metPath, {String? partPath}) async {
    final params = <String, dynamic>{
      'metPath': metPath,
    };
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
    final params = <String, dynamic>{
      'metPath': metPath,
    };
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
  Future<NetworkPolicySnapshot> setNetworkProfile(NetworkProfile profile) async {
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
  Future<List<NetworkAuditEntryItem>> getNetworkAuditLog({int limit = 50}) async {
    final resp = await _call('network.getAuditLog', {'limit': limit});
    final list = (resp as Map<String, dynamic>)['entries'] as List<dynamic>? ?? [];
    return list
        .map((e) => NetworkAuditEntryItem.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<NetworkPolicySnapshot> setResourceBudgets(ResourceBudgetsModel budgets) async {
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
  void dispose() {
    disconnect();
    _connectionController.close();
    _eventsController.close();
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

