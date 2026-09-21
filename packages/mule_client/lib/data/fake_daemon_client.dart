import 'dart:async';
import 'dart:convert';
import 'dart:math';

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

class FakeMuleDaemonClient implements MuleDaemonClient {
  final Map<String, DownloadItem> _downloads = {};
  final Map<String, CategoryItem> _categories = {};
  final Map<String, Ed2kServerModel> _servers = {};
  final List<LabFixtureInfo> _fixtures = [];
  final List<HistoryEntry> _history = [];
  int _fakeRetentionDays = 30;
  String? _activePeer;
  NetworkProfile _fakeProfile = NetworkProfile.publicEd2kKadPreview;
  NetworkSubsystemState _fakeNetState = NetworkSubsystemState.previewActive;
  ResourceBudgetsModel _fakeBudgets = const ResourceBudgetsModel(
    maxActivePeerConnections: 20,
    maxActiveServerConnections: 2,
    maxActiveListeners: 2,
    connectTimeoutMs: 10000,
    handshakeTimeoutMs: 15000,
    maxFrameSizeBytes: 10485760,
    maxTotalBufferBytes: 33554432,
    rateLimitPerMin: 60,
  );
  final List<NetworkAuditEntryItem> _fakeAuditLog = [];
  FilterStatusModel _fakeFilterStatus = const FilterStatusModel(
    enabled: true,
    rangeCount: 0,
    hitsCount: 0,
  );
  RateLimitsModel _fakeRateLimits = const RateLimitsModel(
    downloadBytesPerSec: 0,
    uploadBytesPerSec: 0,
    connectionsPerMinute: 120,
    serverRequestsPerSec: 10,
    kadPacketsPerSec: 50,
  );
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
  final Map<String, List<PeerSourceModel>> _peerSources = {};
  PeerSessionModel _peerSession = PeerSessionModel.disconnected;

  @override
  Stream<PeerSessionModel> get peerSessionEvents =>
      _peerSessionController.stream;


  @override
  Stream<UploadsSnapshotModel> get uploadsEvents => _uploadsController.stream;

  @override
  Stream<DaemonStatsModel> get statsEvents => _statsController.stream;

  @override
  Stream<DownloadLifecycleEvent> get downloadLifecycleEvents =>
      _lifecycleController.stream;

  @override
  Stream<int> get kadStatusEvents => _kadStatusController.stream;

  @override
  Stream<int> get historyEvents => _historyController.stream;

  @override
  Stream<SettingsChangedEvent> get settingsEvents => _settingsController.stream;

  @override
  Stream<int> get networkPolicyEvents => _networkPolicyController.stream;
  @override
  Stream<ServerSessionModel> get serverSessionEvents =>
      _serverSessionController.stream;

  @override
  Stream<List<SearchResultFileModel>> get searchResultEvents =>
      _searchResultController.stream;

  @override
  Stream<Map<String, dynamic>> get foundSourcesEvents =>
      _foundSourcesController.stream;

  DaemonConnectionState _state = DaemonConnectionState.disconnected;
  Timer? _simulationTimer;
  int _sequence = 1;
  final bool enableAutoSimulation;

  FakeMuleDaemonClient({this.enableAutoSimulation = true}) {
    _seedDefaultDownloads();
  }

  static List<DownloadPartInfo> _generateParts(
    int totalBytes,
    int receivedBytes,
    bool isCompleted,
  ) {
    const partSize = 9728000;
    final count = totalBytes > 0 ? (totalBytes / partSize).ceil() : 1;
    final list = <DownloadPartInfo>[];

    for (int i = 0; i < count; i++) {
      final start = i * partSize;
      final end = min((i + 1) * partSize, totalBytes);
      final size = end - start;

      int partReceived = 0;
      if (isCompleted) {
        partReceived = size;
      } else if (receivedBytes >= end) {
        partReceived = size;
      } else if (receivedBytes > start) {
        partReceived = receivedBytes - start;
      }

      final isVerified = isCompleted || (partReceived == size && i % 2 == 0);
      final state = isVerified
          ? 'verified'
          : (partReceived == size
                ? 'completed'
                : (partReceived > 0 ? 'downloading' : 'missing'));

      list.add(
        DownloadPartInfo(
          partIndex: i,
          startOffset: start,
          endOffset: end,
          size: size,
          receivedBytes: partReceived,
          isVerified: isVerified,
          state: state,
        ),
      );
    }
    return list;
  }

  void _seedDefaultDownloads() {
    final now = DateTime.now();

    final catIso = CategoryItem(
      id: 'cat-iso',
      name: 'Linux ISOs',
      color: '#E91E63',
      sortOrder: 0,
      defaultDestination: 'downloads/ISOs',
      defaultPriority: 'high',
      createdAt: now.subtract(const Duration(days: 2)),
      updatedAt: now.subtract(const Duration(days: 2)),
    );
    final catMedia = CategoryItem(
      id: 'cat-media',
      name: 'Media & 3D',
      color: '#00BCD4',
      sortOrder: 1,
      defaultDestination: 'downloads/Media',
      defaultPriority: 'normal',
      createdAt: now.subtract(const Duration(days: 2)),
      updatedAt: now.subtract(const Duration(days: 2)),
    );
    _categories[catIso.id] = catIso;
    _categories[catMedia.id] = catMedia;

    final d1 = DownloadItem(
      id: '00000000-0000-0000-0000-000000000001',
      fileName: 'Debian_12_netinst.iso',
      totalBytes: 654311424,
      receivedBytes: 550000000,
      verifiedBytes: 540000000,
      progress: 550000000 / 654311424,
      status: DownloadStatus.downloading,
      priority: DownloadPriority.high,
      sourceCount: 24,
      downloadRateBps: 2850000,
      uploadedBytes: 15400000,
      createdAt: now.subtract(const Duration(minutes: 45)),
      updatedAt: now,
      ed2kHash: '31d6cfe0d16ae931b73c59d7e0c089c0',
      ed2kLink: 'ed2k://|file|Debian_12_netinst.iso|654311424|31d6cfe0d16ae931b73c59d7e0c089c0|/',
      tempPath: 'data/temp/00000000-0000-0000-0000-000000000001.part',
      targetPath: 'downloads/Debian_12_netinst.iso',
      averageRateBps: 2600000,
      etaSeconds: 40,
      connectionAttempts: 1,
      lastStatusChange: now.subtract(const Duration(minutes: 45)),
      parts: _generateParts(654311424, 550000000, false),
      categoryId: catIso.id,
    );

    final d2 = DownloadItem(
      id: '00000000-0000-0000-0000-000000000002',
      fileName: 'Blender_Benchmark_Scene.blend',
      totalBytes: 94371840,
      receivedBytes: 40000000,
      verifiedBytes: 38912000,
      progress: 40000000 / 94371840,
      status: DownloadStatus.paused,
      priority: DownloadPriority.normal,
      sourceCount: 7,
      downloadRateBps: 0,
      uploadedBytes: 2048000,
      createdAt: now.subtract(const Duration(hours: 2)),
      updatedAt: now.subtract(const Duration(minutes: 20)),
      ed2kHash: '42d6cfe0d16ae931b73c59d7e0c089c1',
      ed2kLink: 'ed2k://|file|Blender_Benchmark_Scene.blend|94371840|42d6cfe0d16ae931b73c59d7e0c089c1|/',
      tempPath: 'data/temp/00000000-0000-0000-0000-000000000002.part',
      targetPath: 'downloads/Blender_Benchmark_Scene.blend',
      averageRateBps: 0,
      connectionAttempts: 2,
      lastStatusChange: now.subtract(const Duration(minutes: 20)),
      parts: _generateParts(94371840, 40000000, false),
      categoryId: catMedia.id,
    );

    final d3 = DownloadItem(
      id: '00000000-0000-0000-0000-000000000003',
      fileName: 'Linux_Kernel_6.10.tar.xz',
      totalBytes: 148897280,
      receivedBytes: 148897280,
      verifiedBytes: 148897280,
      progress: 1.0,
      status: DownloadStatus.completed,
      priority: DownloadPriority.normal,
      sourceCount: 42,
      downloadRateBps: 0,
      uploadedBytes: 89400000,
      createdAt: now.subtract(const Duration(days: 1)),
      updatedAt: now.subtract(const Duration(hours: 5)),
      ed2kHash: '53d6cfe0d16ae931b73c59d7e0c089c2',
      ed2kLink: 'ed2k://|file|Linux_Kernel_6.10.tar.xz|148897280|53d6cfe0d16ae931b73c59d7e0c089c2|/',
      tempPath: 'data/temp/00000000-0000-0000-0000-000000000003.part',
      targetPath: 'downloads/Linux_Kernel_6.10.tar.xz',
      averageRateBps: 3400000,
      connectionAttempts: 1,
      lastStatusChange: now.subtract(const Duration(hours: 5)),
      parts: _generateParts(148897280, 148897280, true),
    );

    _downloads[d1.id] = d1;
    _downloads[d2.id] = d2;
    _downloads[d3.id] = d3;

    _history.add(
      HistoryEntry(
        id: 1,
        downloadId: d3.id,
        fileName: d3.fileName,
        totalBytes: d3.totalBytes,
        transferredBytes: d3.totalBytes,
        ed2kHash: d3.ed2kHash ?? '',
        eventType: 'completed',
        status: 'completed',
        durationSecs: 48,
        connectionAttempts: 1,
        resumeCount: 0,
        verificationResult: 'verified_ok',
        errorMessage: null,
        timestamp: now.subtract(const Duration(hours: 5)),
      ),
    );
    _history.add(
      HistoryEntry(
        id: 2,
        downloadId: d2.id,
        fileName: d2.fileName,
        totalBytes: d2.totalBytes,
        transferredBytes: d2.receivedBytes,
        ed2kHash: d2.ed2kHash ?? '',
        eventType: 'resumed',
        status: 'downloading',
        durationSecs: 12,
        connectionAttempts: 2,
        resumeCount: 1,
        verificationResult: 'unverified',
        errorMessage: null,
        timestamp: now.subtract(const Duration(hours: 1)),
      ),
    );
  }

  @override
  DaemonConnectionState get currentConnectionState => _state;

  @override
  Stream<DaemonConnectionState> get connectionState =>
      _connectionController.stream;

  @override
  Stream<DownloadUpdatedEvent> get downloadEvents => _eventsController.stream;

  @override
  Future<DaemonInfo> connect({required Uri url, String? token}) async {
    _state = DaemonConnectionState.connecting;
    _connectionController.add(_state);

    await Future.delayed(const Duration(milliseconds: 100));

    _state = DaemonConnectionState.connected;
    _connectionController.add(_state);

    if (enableAutoSimulation && _simulationTimer == null) {
      _startSimulation();
    }

    _daemonInfo = const DaemonInfo(
      version: '0.1.0-fake',
      protocolVersion: 1,
      instanceId: 'fake-instance-id',
      sessionId: 'fake-session-id',
      capabilities: [
        'downloads',
        'download-events',
        'lab-tools',
        'servers',
        'server-connection',
        'server-search',
        'server-sources',
      ],
      workdirMode: 'Portable',
      workdirPath: 'C:\\Fake\\Portable\\data',
      defaultDownloadDir: 'C:\\Fake\\Portable\\downloads',
    );
    return _daemonInfo!;
  }

  DaemonInfo? _daemonInfo;

  @override
  DaemonInfo? get daemonInfo => _daemonInfo;

  @override
  Future<void> stopDaemon() async {
    await disconnect();
  }

  @override
  Future<void> disconnect() async {
    _simulationTimer?.cancel();
    _simulationTimer = null;
    _state = DaemonConnectionState.disconnected;
    _connectionController.add(_state);
  }

  @override
  Future<List<DownloadItem>> listDownloads() async {
    return _downloads.values.toList();
  }

  @override
  Future<DownloadItem?> getDownload(String id) async {
    return _downloads[id];
  }

  @override
  Future<String> addEd2kLink(
    String link, {
    String? targetDirectory,
    String? allocationPolicy,
    String? categoryId,
  }) async {
    final parts = link.split('|');
    String fileName = 'download_${DateTime.now().millisecondsSinceEpoch}.bin';
    int totalBytes = 10000000;

    if (parts.length >= 5 && parts[1].toLowerCase() == 'file') {
      fileName = Uri.decodeComponent(parts[2]);
      totalBytes = int.tryParse(parts[3]) ?? 10000000;
    }

    final id =
        'fake-${DateTime.now().millisecondsSinceEpoch}-${Random().nextInt(9999)}';
    final now = DateTime.now();

    final item = DownloadItem(
      id: id,
      fileName: fileName,
      totalBytes: totalBytes,
      receivedBytes: 0,
      verifiedBytes: 0,
      progress: 0.0,
      status: DownloadStatus.downloading,
      priority: DownloadPriority.normal,
      sourceCount: 1,
      downloadRateBps: 150000,
      uploadedBytes: 0,
      createdAt: now,
      updatedAt: now,
      targetPath: targetDirectory != null ? '$targetDirectory/$fileName' : null,
      categoryId: categoryId,
    );

    _downloads[id] = item;
    _emitLifecycle(DownloadLifecycleKind.added, download: item);
    _emitUpdate(item);
    return id;
  }

  @override
  Future<DownloadItem> pauseDownload(String id) async {
    final existing = _downloads[id];
    if (existing == null) throw Exception('Download $id not found');

    final updated = existing.copyWith(
      status: DownloadStatus.paused,
      downloadRateBps: 0,
      updatedAt: DateTime.now(),
    );

    _downloads[id] = updated;
    _emitUpdate(updated);
    return updated;
  }

  @override
  Future<DownloadItem> resumeDownload(String id) async {
    final existing = _downloads[id];
    if (existing == null) throw Exception('Download $id not found');

    final updated = existing.copyWith(
      status: DownloadStatus.downloading,
      downloadRateBps: 512000,
      updatedAt: DateTime.now(),
    );

    _downloads[id] = updated;
    _emitUpdate(updated);
    return updated;
  }

  @override
  Future<void> removeDownload(String id, {bool deleteFiles = false}) async {
    _downloads.remove(id);
    _emitLifecycle(DownloadLifecycleKind.removed, id: id);
  }

  @override
  Future<DownloadItem> injectRange(
    String id,
    int offset,
    String hexData,
  ) async {
    final existing = _downloads[id];
    if (existing == null) throw Exception('Download $id not found');

    final bytesCount = hexData.length ~/ 2;
    final newReceived = min(
      existing.totalBytes,
      existing.receivedBytes + bytesCount,
    );
    final newProgress = existing.totalBytes > 0
        ? newReceived / existing.totalBytes
        : 1.0;
    final isDone = newReceived >= existing.totalBytes;

    final updated = existing.copyWith(
      receivedBytes: newReceived,
      verifiedBytes: newReceived,
      progress: newProgress,
      status: isDone ? DownloadStatus.completed : existing.status,
      downloadRateBps: isDone ? 0 : existing.downloadRateBps,
      updatedAt: DateTime.now(),
    );

    _downloads[id] = updated;
    _emitUpdate(updated);
    return updated;
  }

  final StreamController<UploadsSnapshotModel> _uploadsController =
      StreamController<UploadsSnapshotModel>.broadcast();
  final StreamController<DaemonStatsModel> _statsController =
      StreamController<DaemonStatsModel>.broadcast();
  final StreamController<DownloadLifecycleEvent> _lifecycleController =
      StreamController<DownloadLifecycleEvent>.broadcast();
  final StreamController<int> _kadStatusController =
      StreamController<int>.broadcast();
  final StreamController<int> _historyController =
      StreamController<int>.broadcast();
  final StreamController<SettingsChangedEvent> _settingsController =
      StreamController<SettingsChangedEvent>.broadcast();
  final StreamController<int> _networkPolicyController =
      StreamController<int>.broadcast();
  int _simTick = 0;

  void _emitLifecycle(
    DownloadLifecycleKind kind, {
    DownloadItem? download,
    String? id,
    String? reason,
  }) {
    if (_lifecycleController.isClosed) return;
    _sequence += 1;
    _lifecycleController.add(
      DownloadLifecycleEvent(
        sequence: _sequence,
        kind: kind,
        download: download,
        id: id,
        reason: reason,
      ),
    );
  }

  void _emitUpdate(DownloadItem item) {
    _sequence += 1;
    if (!_eventsController.isClosed) {
      _eventsController.add(
        DownloadUpdatedEvent(sequence: _sequence, download: item),
      );
    }
  }

  void _startSimulation() {
    _simulationTimer = Timer.periodic(const Duration(milliseconds: 1000), (
      timer,
    ) {
      for (final item in _downloads.values.toList()) {
        if (item.status == DownloadStatus.downloading) {
          final added = 350000 + Random().nextInt(400000);
          final newReceived = min(item.totalBytes, item.receivedBytes + added);
          final isDone = newReceived >= item.totalBytes;
          final newProgress = item.totalBytes > 0
              ? newReceived / item.totalBytes
              : 1.0;

          final updated = item.copyWith(
            receivedBytes: newReceived,
            verifiedBytes: min(newReceived, item.verifiedBytes + added),
            progress: newProgress,
            downloadRateBps: isDone ? 0 : (added * 1.0).toInt(),
            status: isDone
                ? DownloadStatus.completed
                : DownloadStatus.downloading,
            updatedAt: DateTime.now(),
          );

          _downloads[item.id] = updated;
          _emitUpdate(updated);
          if (isDone) {
            _emitLifecycle(
              DownloadLifecycleKind.completed,
              download: updated,
            );
          }
        }
      }
      _emitUploadsTick();
      _emitStatsTick();
    });
  }

  /// Simulated `event.statsTick`: rates follow the fake upload rate and the
  /// sum of downloading items' `downloadRateBps`.
  void _emitStatsTick() {
    if (_statsController.isClosed) return;
    final downloadBps = _downloads.values
        .where((d) => d.status == DownloadStatus.downloading)
        .fold<int>(0, (sum, d) => sum + d.downloadRateBps);
    final uploadBps = _fakeActiveUpload?.uploadRateBps ?? 0;
    _statsController.add(
      DaemonStatsModel(
        downloadBps: downloadBps,
        uploadBps: uploadBps,
        downloadsActive: _downloads.values
            .where((d) => d.status == DownloadStatus.downloading)
            .length,
        peersConnected: _activePeer != null ? 1 : 0,
        serverConnected: _state == DaemonConnectionState.connected,
        kadContacts: 42,
        kadUdpBound: true,
      ),
    );
  }

  /// Emits a simulated `upload.uploadsUpdated` snapshot each tick: one
  /// active upload oscillating ~50–250 kB/s plus a small waiting queue.
  void _emitUploadsTick() {
    if (_uploadsController.isClosed) return;
    _simTick += 1;
    final rate = 50000 + Random().nextInt(200000);
    final active = [
      UploadSessionModel(
        clientAddress: '203.0.113.7:4662',
        fileHash: 'aabbccddeeff00112233445566778899',
        fileName: 'shared-sample.iso',
        transferredBytes: _simTick * rate,
        uploadRateBps: rate,
        state: 'uploading',
        startedAtUnixMs:
            DateTime.now().millisecondsSinceEpoch - _simTick * 1000,
      ),
    ];
    final queue = [
      UploadSessionModel(
        clientAddress: '198.51.100.23:4662',
        fileHash: '11223344556677889900aabbccddeeff',
        fileName: 'queued-part.bin',
        transferredBytes: 0,
        uploadRateBps: 0,
        state: 'waiting',
        queueRank: 1,
        startedAtUnixMs: DateTime.now().millisecondsSinceEpoch,
      ),
    ];
    _uploadsController.add(
      UploadsSnapshotModel(active: active, queue: queue, uploadRateBps: rate),
    );
  }

  @override
  Future<LabStatus> getLabStatus() async {
    return LabStatus(
      workdirMode: 'Portable',
      workdirPath: r'C:\Fake\Portable\data',
      isPortable: true,
      activeLoopbackPeer: _activePeer,
      fixtures: List.unmodifiable(_fixtures),
    );
  }

  @override
  Future<LabFixtureInfo> createLabFixture(String fixtureType) async {
    final fileName = fixtureType == 'part_9728000'
        ? 'lab_part_9728000.bin'
        : (fixtureType == 'sample_1mb' ? 'lab_sample_1mb.bin' : 'lab_test.txt');
    final sizeBytes = fixtureType == 'part_9728000'
        ? 9728000
        : (fixtureType == 'sample_1mb' ? 1048576 : 4096);
    final ed2kHash = fixtureType == 'part_9728000'
        ? '0123456789abcdef0123456789abcdef'
        : (fixtureType == 'sample_1mb'
              ? '123456789abcdef0123456789abcdef0'
              : 'abcdef0123456789abcdef0123456789');
    final ed2kLink = 'ed2k://|file|$fileName|$sizeBytes|$ed2kHash|/';

    final fixture = LabFixtureInfo(
      fileName: fileName,
      filePath: 'C:\\Fake\\Portable\\data\\fixtures\\$fileName',
      sizeBytes: sizeBytes,
      ed2kHash: ed2kHash,
      ed2kLink: ed2kLink,
      status: 'ready',
    );

    _fixtures.removeWhere((f) => f.fileName == fileName);
    _fixtures.add(fixture);
    return fixture;
  }

  @override
  Future<LabPeerInfo> startLabPeer() async {
    _activePeer = '127.0.0.1:39735';
    return LabPeerInfo(
      peerAddr: _activePeer!,
      sharedCount: _fixtures.isEmpty ? 1 : _fixtures.length,
      status: 'running',
    );
  }

  @override
  Future<void> stopLabPeer() async {
    _activePeer = null;
  }

  @override
  Future<LabCleanupReport> cleanupLabTestData() async {
    final count = _fixtures.length;
    _fixtures.clear();
    _activePeer = null;
    return LabCleanupReport(
      success: true,
      cleanedFolders: ['fixtures', 'temp', 'part'],
      deletedFilesCount: count,
      freedBytes: count * 1048576,
      message: 'Cleaned $count fixtures',
    );
  }

  @override
  Future<void> connectDownloadToSource(
    String downloadId,
    String peerAddr,
  ) async {
    final existing = _downloads[downloadId];
    if (existing != null) {
      final updated = existing.copyWith(
        sourceCount: existing.sourceCount + 1,
        status: DownloadStatus.downloading,
        downloadRateBps: max(existing.downloadRateBps, 350000),
        updatedAt: DateTime.now(),
      );
      _downloads[downloadId] = updated;
      _emitUpdate(updated);
    }
  }

  String? _fakeCustomDownloadDir;

  @override
  Future<DownloadItem> setDownloadDestination(
    String id,
    String targetDirectory,
  ) async {
    final existing = _downloads[id];
    if (existing == null) throw Exception('Download $id not found');
    final updated = existing.copyWith(
      targetPath: '$targetDirectory/${existing.fileName}',
      updatedAt: DateTime.now(),
    );
    _downloads[id] = updated;
    _emitUpdate(updated);
    return updated;
  }

  @override
  Future<DownloadItem> setDownloadPriority(String id, String priority) async {
    final existing = _downloads[id];
    if (existing == null) throw Exception('Download $id not found');
    final updated = existing.copyWith(
      priority: DownloadPriority.fromString(priority),
      updatedAt: DateTime.now(),
    );
    _downloads[id] = updated;
    _emitUpdate(updated);
    return updated;
  }

  @override
  Future<DownloadItem> retryDownloadConnection(String id) async {
    final existing = _downloads[id];
    if (existing == null) throw Exception('Download $id not found');
    final updated = existing.copyWith(
      connectionAttempts: existing.connectionAttempts + 1,
      lastError: null,
      status: DownloadStatus.downloading,
      updatedAt: DateTime.now(),
      lastStatusChange: DateTime.now(),
    );
    _downloads[id] = updated;
    _emitUpdate(updated);
    return updated;
  }

  @override
  Future<Map<String, dynamic>> getSettings() async {
    return {
      'defaultDownloadDir': _fakeCustomDownloadDir ?? 'downloads',
      'portableDownloadDir': 'downloads',
      'isCustom': _fakeCustomDownloadDir != null,
      'historyRetentionDays': _fakeRetentionDays,
    };
  }

  @override
  Future<void> setDefaultDownloadDirectory(String path) async {
    _fakeCustomDownloadDir = path;
  }

  @override
  Future<void> resetDefaultDownloadDirectory() async {
    _fakeCustomDownloadDir = null;
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
    var filtered = _history.where((e) {
      if (eventType != null &&
          eventType.isNotEmpty &&
          e.eventType != eventType) {
        return false;
      }
      if (status != null && status.isNotEmpty && e.status != status) {
        return false;
      }
      if (fromTimestampMs != null &&
          e.timestamp.millisecondsSinceEpoch < fromTimestampMs) {
        return false;
      }
      if (toTimestampMs != null &&
          e.timestamp.millisecondsSinceEpoch > toTimestampMs) {
        return false;
      }
      return true;
    }).toList();

    filtered.sort((a, b) => b.timestamp.compareTo(a.timestamp));
    if (offset != null && offset > 0) {
      if (offset >= filtered.length) return const [];
      filtered = filtered.sublist(offset);
    }
    if (limit != null && filtered.length > limit) {
      filtered = filtered.sublist(0, limit);
    }
    return filtered;
  }

  @override
  Future<HistoryStats> getHistoryStats() async {
    final completed = _history.where((e) => e.eventType == 'completed');
    final completedCount = completed.length;
    final completedBytes = completed.fold<int>(
      0,
      (sum, e) => sum + e.totalBytes,
    );
    final failedCount = _history.where((e) => e.eventType == 'failed').length;
    final resumesCount = _history.where((e) => e.eventType == 'resumed').length;
    final integrityFailures = _history
        .where((e) => e.verificationResult == 'hash_mismatch')
        .length;

    return HistoryStats(
      totalCompletedCount: completedCount,
      totalCompletedBytes: completedBytes,
      totalFailedCount: failedCount,
      totalResumesCount: resumesCount,
      totalIntegrityFailures: integrityFailures,
      sessionCompletedBytes: completedBytes,
      sessionCompletedCount: completedCount,
      last24hCompletedBytes: completedBytes,
      last24hCompletedCount: completedCount,
      last7dCompletedBytes: completedBytes,
      last7dCompletedCount: completedCount,
    );
  }

  @override
  Future<int> clearHistory() async {
    final count = _history.length;
    _history.clear();
    return count;
  }

  @override
  Future<String> exportHistory(
    String format, {
    String? eventType,
    String? status,
  }) async {
    final entries = await listHistory(eventType: eventType, status: status);
    if (format.toLowerCase() == 'csv') {
      final buffer = StringBuffer(
        'id,download_id,file_name,total_size,transferred_bytes,ed2k_hash,event_type,status,duration_secs,connection_attempts,resume_count,verification_result,error_message,timestamp_unix_ms\n',
      );
      for (final e in entries) {
        buffer.writeln(
          '${e.id},${e.downloadId},"${e.fileName}",${e.totalBytes},${e.transferredBytes},${e.ed2kHash},${e.eventType},${e.status},${e.durationSecs},${e.connectionAttempts},${e.resumeCount},${e.verificationResult},"${e.errorMessage ?? ''}",${e.timestamp.millisecondsSinceEpoch}',
        );
      }
      return buffer.toString();
    } else {
      return jsonEncode(
        entries
            .map(
              (e) => {
                'id': e.id,
                'downloadId': e.downloadId,
                'fileName': e.fileName,
                'totalSize': e.totalBytes,
                'transferredBytes': e.transferredBytes,
                'ed2kHash': e.ed2kHash,
                'eventType': e.eventType,
                'status': e.status,
                'durationSecs': e.durationSecs,
                'connectionAttempts': e.connectionAttempts,
                'resumeCount': e.resumeCount,
                'verificationResult': e.verificationResult,
                'errorMessage': e.errorMessage,
                'timestampUnixMs': e.timestamp.millisecondsSinceEpoch,
              },
            )
            .toList(),
      );
    }
  }

  @override
  Future<void> setHistoryRetentionDays(int days) async {
    _fakeRetentionDays = days;
  }

  @override
  Future<List<CategoryItem>> listCategories() async {
    final list = _categories.values.toList()
      ..sort((a, b) => a.sortOrder.compareTo(b.sortOrder));
    return list;
  }

  @override
  Future<CategoryItem> createCategory({
    required String name,
    required String color,
    String? defaultDestination,
    String? defaultPriority,
  }) async {
    final id =
        'fake-cat-${DateTime.now().millisecondsSinceEpoch}-${_categories.length}';
    final now = DateTime.now();
    final cat = CategoryItem(
      id: id,
      name: name,
      color: color,
      sortOrder: _categories.length,
      defaultDestination: defaultDestination,
      defaultPriority: defaultPriority,
      createdAt: now,
      updatedAt: now,
    );
    _categories[id] = cat;
    return cat;
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
    final existing = _categories[id];
    if (existing == null) throw Exception('Category not found: $id');

    final updated = existing.copyWith(
      name: name,
      color: color,
      defaultDestination: defaultDestination,
      defaultPriority: defaultPriority,
      sortOrder: sortOrder,
      updatedAt: DateTime.now(),
    );
    _categories[id] = updated;
    return updated;
  }

  @override
  Future<void> deleteCategory(String id) async {
    _categories.remove(id);
    // Invariant: reset assigned downloads categoryId to null without deleting them
    for (final dlId in _downloads.keys.toList()) {
      final dl = _downloads[dlId]!;
      if (dl.categoryId == id) {
        final updatedDl = dl.copyWith(clearCategoryId: true);
        _downloads[dlId] = updatedDl;
        _eventsController.add(
          DownloadUpdatedEvent(sequence: _sequence++, download: updatedDl),
        );
      }
    }
  }

  @override
  Future<void> reorderCategories(List<String> categoryIds) async {
    for (int i = 0; i < categoryIds.length; i++) {
      final id = categoryIds[i];
      final cat = _categories[id];
      if (cat != null) {
        _categories[id] = cat.copyWith(sortOrder: i);
      }
    }
  }

  @override
  Future<DownloadItem> setDownloadCategory(
    String downloadId,
    String? categoryId,
  ) async {
    final dl = _downloads[downloadId];
    if (dl == null) throw Exception('Download not found: $downloadId');
    if (categoryId != null && !_categories.containsKey(categoryId)) {
      throw Exception('Category not found: $categoryId');
    }
    final updated = dl.copyWith(
      categoryId: categoryId,
      clearCategoryId: categoryId == null,
    );
    _downloads[downloadId] = updated;
    _eventsController.add(
      DownloadUpdatedEvent(sequence: _sequence++, download: updated),
    );
    return updated;
  }

  @override
  Future<PartMetInspectionResult> inspectPartMet(
    String metPath, {
    String? partPath,
  }) async {
    final fileName = metPath
        .split(RegExp(r'[\\/]'))
        .last
        .replaceAll('.part.met', '.bin');
    return PartMetInspectionResult(
      fileName: fileName.isEmpty ? 'sample_imported.bin' : fileName,
      totalSize: 9728000,
      ed2kHash: '31d6cfe0d16ae931b73c59d7e0c089c0',
      partCount: 1,
      partMetPath: metPath,
      partPath: partPath ?? metPath.replaceAll('.met', ''),
      partFileExists: true,
      partFileSize: 9728000,
      estimatedTransferredBytes: 9728000,
    );
  }

  @override
  Future<ImportPartMetResult> importPartMet(
    String metPath, {
    String? partPath,
    String? targetDirectory,
    String? categoryId,
  }) async {
    final inspection = await inspectPartMet(metPath, partPath: partPath);
    final id = 'dl-import-${_downloads.length + 1}';
    final targetDir = targetDirectory ?? _fakeCustomDownloadDir ?? 'downloads';
    final dl = DownloadItem(
      id: id,
      fileName: inspection.fileName,
      totalBytes: inspection.totalSize,
      receivedBytes: inspection.estimatedTransferredBytes,
      verifiedBytes: inspection.estimatedTransferredBytes,
      progress: 1.0,
      status: DownloadStatus.paused, // Strictly Paused!
      priority: DownloadPriority.normal,
      sourceCount: 0,
      downloadRateBps: 0,
      uploadedBytes: 0,
      createdAt: DateTime.now(),
      updatedAt: DateTime.now(),
      categoryId: categoryId,
      tempPath: 'data/temp/$id.part',
      targetPath: '$targetDir/${inspection.fileName}',
      ed2kHash: inspection.ed2kHash,
    );
    _downloads[id] = dl;
    _eventsController.add(
      DownloadUpdatedEvent(sequence: _sequence++, download: dl),
    );

    final report =
        "=== Emule-Sion .part.met Import Audit Report ===\n"
        "Format Version: 0xE0 (Standard eMule)\n"
        "File Name: ${inspection.fileName}\n"
        "Declared Total Size: ${inspection.totalSize} bytes\n"
        "Announced Parts: ${inspection.partCount}\n"
        "Verified Valid Parts: ${inspection.partCount}\n"
        "Rejected/Corrupted Parts: 0\n"
        "Rejection Detail: None (All complete parts passed integrity check)\n"
        "Network Status: STRICTLY PAUSED (sourceCount: 0, zero network activity)\n"
        "Confinement: Isolated under data/temp/$id.part (Source file untouched)";

    return ImportPartMetResult(
      download: dl,
      totalParts: inspection.partCount,
      verifiedParts: inspection.partCount,
      corruptedParts: 0,
      transferredBytes: inspection.estimatedTransferredBytes,
      totalSize: inspection.totalSize,
      report: report,
    );
  }

  @override
  Future<NetworkPolicySnapshot> getNetworkPolicy() async {
    return NetworkPolicySnapshot(
      profile: _fakeProfile,
      state: _fakeNetState,
      budgets: _fakeBudgets,
      activeConnections: const NetworkActiveConnections(),
    );
  }

  @override
  Future<NetworkPolicySnapshot> setNetworkProfile(
    NetworkProfile profile,
  ) async {
    _fakeProfile = profile;
    _fakeAuditLog.insert(
      0,
      NetworkAuditEntryItem(
        id: _fakeAuditLog.length + 1,
        timestampEpochMs: DateTime.now().millisecondsSinceEpoch,
        eventType: 'profile-changed',
        profile: profile,
        decision: 'allowed',
        reason: 'Profile switched to ${profile.toKebabCase()}',
      ),
    );
    return getNetworkPolicy();
  }

  @override
  Future<void> disconnectAll() async {
    _fakeNetState = NetworkSubsystemState.disabled;
    _fakeAuditLog.insert(
      0,
      NetworkAuditEntryItem(
        id: _fakeAuditLog.length + 1,
        timestampEpochMs: DateTime.now().millisecondsSinceEpoch,
        eventType: 'kill-switch-triggered',
        profile: _fakeProfile,
        decision: 'allowed',
        reason: 'Kill switch activated',
      ),
    );
  }

  @override
  Future<void> enableNetwork() async {
    _fakeNetState = NetworkSubsystemState.previewActive;
    _fakeAuditLog.insert(
      0,
      NetworkAuditEntryItem(
        id: _fakeAuditLog.length + 1,
        timestampEpochMs: DateTime.now().millisecondsSinceEpoch,
        eventType: 'network-enabled',
        profile: _fakeProfile,
        decision: 'allowed',
        reason: 'Network policy re-enabled',
      ),
    );
  }

  @override
  Future<List<NetworkAuditEntryItem>> getNetworkAuditLog({
    int limit = 50,
  }) async {
    return _fakeAuditLog.take(limit).toList();
  }

  @override
  Future<NetworkPolicySnapshot> setResourceBudgets(
    ResourceBudgetsModel budgets,
  ) async {
    _fakeBudgets = budgets;
    return getNetworkPolicy();
  }

  @override
  Future<FilterStatusModel> getFilterStatus() async {
    return _fakeFilterStatus;
  }

  @override
  Future<FilterStatusModel> importFilterFile(String filePath) async {
    final report = FilterReportModel(
      totalLines: 1500,
      commentOrEmptyLines: 100,
      validEntriesParsed: 1400,
      invalidLines: 0,
      coalescedCount: 150,
      finalRangeCount: 1250,
      warnings: const [],
    );
    _fakeFilterStatus = FilterStatusModel(
      enabled: true,
      rangeCount: 1250,
      hitsCount: 0,
      lastImportReport: report,
    );
    return _fakeFilterStatus;
  }

  @override
  Future<FilterStatusModel> setFilterEnabled(bool enabled) async {
    _fakeFilterStatus = FilterStatusModel(
      enabled: enabled,
      rangeCount: _fakeFilterStatus.rangeCount,
      hitsCount: _fakeFilterStatus.hitsCount,
      lastImportReport: _fakeFilterStatus.lastImportReport,
    );
    return _fakeFilterStatus;
  }

  @override
  Future<FilterStatusModel> clearFilter() async {
    _fakeFilterStatus = const FilterStatusModel(
      enabled: true,
      rangeCount: 0,
      hitsCount: 0,
    );
    return _fakeFilterStatus;
  }

  @override
  Future<RateLimitsModel> getRateLimits() async {
    return _fakeRateLimits;
  }

  @override
  Future<RateLimitsModel> setRateLimits(RateLimitsModel limits) async {
    _fakeRateLimits = limits;
    return _fakeRateLimits;
  }

  @override
  Future<ServerInspectReport> inspectServerMet(String filePath) async {
    final s1 = Ed2kServerModel(
      address: '176.103.48.36:4184',
      ip: '176.103.48.36',
      port: 4184,
      name: 'eDonkeyServer No2',
      description: 'Public ed2k server',
      ping: 28,
      preference: 10,
      filterStatus: ServerFilterStatus.allowed,
    );
    final s2 = Ed2kServerModel(
      address: '10.20.30.40:4661',
      ip: '10.20.30.40',
      port: 4661,
      name: 'Filtered Rouge Server',
      description: 'Blocked test entry',
      ping: 999,
      preference: 1,
      filterStatus: ServerFilterStatus.blockedByFilter,
    );
    return ServerInspectReport(
      filePath: filePath,
      totalCount: 2,
      allowedCount: 1,
      blockedCount: 1,
      servers: [s1, s2],
    );
  }

  @override
  Future<ServerImportResult> importServerMet(String filePath) async {
    final report = await inspectServerMet(filePath);
    for (final s in report.servers) {
      _servers[s.address] = s;
    }
    return ServerImportResult(
      success: true,
      imported: report.servers.length,
      totalCount: report.totalCount,
      allowedCount: report.allowedCount,
      blockedCount: report.blockedCount,
    );
  }

  @override
  Future<List<Ed2kServerModel>> listServers() async {
    final list = _servers.values.toList();
    list.sort((a, b) => b.preference.compareTo(a.preference));
    return list;
  }

  @override
  Future<bool> setServerEnabled(String address, bool enabled) async {
    final existing = _servers[address];
    if (existing == null) return false;
    _servers[address] = Ed2kServerModel(
      address: existing.address,
      ip: existing.ip,
      port: existing.port,
      name: existing.name,
      description: existing.description,
      ping: existing.ping,
      preference: existing.preference,
      failedCount: existing.failedCount,
      dynIp: existing.dynIp,
      version: existing.version,
      filterStatus: enabled
          ? ServerFilterStatus.allowed
          : ServerFilterStatus.disabled,
      lastSeenUnixMs: existing.lastSeenUnixMs,
      createdAtUnixMs: existing.createdAtUnixMs,
      updatedAtUnixMs: DateTime.now().millisecondsSinceEpoch,
    );
    return true;
  }

  @override
  Future<bool> deleteServer(String address) async {
    return _servers.remove(address) != null;
  }

  @override
  Future<int> clearServers() async {
    final count = _servers.length;
    _servers.clear();
    return count;
  }

  ServerSessionModel _serverSession = ServerSessionModel.disconnected;

  @override
  Future<ServerSessionModel> connectServer(String address) async {
    final s = _servers[address];
    _serverSession = ServerSessionModel(
      status: ServerConnectionStatus.connected,
      serverAddress: address,
      serverName: s?.name ?? 'Serveur eD2K de test',
      serverDesc: s?.description ?? 'Serveur simulé pour le laboratoire',
      clientId: 25000000,
      isHighId: true,
      welcomeMessage: 'Bienvenue sur le serveur simulé Emule-Sion !',
      currentUsers: 1500,
      currentFiles: 60000,
      connectedAtUnixMs: DateTime.now().millisecondsSinceEpoch,
      pingMs: 25,
    );
    return _serverSession;
  }

  @override
  Future<ServerSessionModel> disconnectServer() async {
    _serverSession = ServerSessionModel.disconnected;
    return _serverSession;
  }

  @override
  Future<ServerSessionModel> getServerStatus() async {
    return _serverSession;
  }

  @override
  Future<Map<String, dynamic>> searchServer({
    required String term,
    int? minSize,
    int? maxSize,
    String? extension,
    bool udpOnly = false,
  }) async {
    final results = [
      SearchResultFileModel(
        fileHash: '0123456789abcdef0123456789abcdef',
        fileName: '$term-release.iso',
        fileSize: 734003200,
        sourceCount: 42,
      ),
      SearchResultFileModel(
        fileHash: 'fedcba9876543210fedcba9876543210',
        fileName: '$term-documentation.pdf',
        fileSize: 15728640,
        sourceCount: 15,
      ),
    ];
    Future.microtask(() {
      if (!_searchResultController.isClosed) {
        _searchResultController.add(results);
      }
    });
    return <String, dynamic>{
      'sent': true,
      'udpTargets': const [],
    };
  }

  @override
  Future<void> queryServerSources({
    required String fileHash,
    int? fileSize,
  }) async {
    final sources = [
      {'address': '192.168.1.50:39735', 'ip': 12345, 'port': 39735},
      {'address': '10.0.0.12:39735', 'ip': 67890, 'port': 39735},
    ];
    Future.microtask(() {
      if (!_foundSourcesController.isClosed) {
        _foundSourcesController.add({'fileHash': fileHash, 'sources': sources});
      }
    });
  }

  @override
  Future<List<PeerSourceModel>> listSources(String fileHash) async {
    return _peerSources[fileHash] ?? [];
  }

  @override
  Future<PeerSourceModel> addManualSource(
    String fileHash,
    String address,
  ) async {
    final parts = address.split(':');
    final port = parts.length > 1 ? int.tryParse(parts[1]) ?? 39735 : 39735;
    final src = PeerSourceModel(
      fileHash: fileHash,
      address: address,
      ip: 0x7F000001,
      port: port,
      origin: PeerSourceOrigin.manual,
      discoveredAtUnixMs: DateTime.now().millisecondsSinceEpoch,
      lastSeenUnixMs: DateTime.now().millisecondsSinceEpoch,
      status: PeerSourceStatus.allowed,
    );
    _peerSources.putIfAbsent(fileHash, () => []).add(src);
    return src;
  }

  @override
  Future<PeerSessionModel> connectPeerSource(
    String fileHash,
    String address, {
    bool force = false,
  }) async {
    _peerSession = PeerSessionModel(
      status: PeerConnectionStatus.transferring,
      fileHash: fileHash,
      peerAddress: address,
      currentPart: 0,
      transferRateBps: 245760,
      transferredBytes: 184320,
      connectedAtUnixMs: DateTime.now().millisecondsSinceEpoch,
    );
    if (!_peerSessionController.isClosed) {
      _peerSessionController.add(_peerSession);
    }
    return _peerSession;
  }

  @override
  Future<PeerSessionModel> disconnectPeerSource({String? address}) async {
    _peerSession = PeerSessionModel.disconnected;
    if (!_peerSessionController.isClosed) {
      _peerSessionController.add(_peerSession);
    }
    return _peerSession;
  }

  @override
  Future<PeerSessionModel> getPeerSessionStatus() async {
    return _peerSession;
  }

  @override
  Future<SwarmStatusModel> getSwarmStatus({String? fileHash}) async {
    final activePeers = _peerSession.status != PeerConnectionStatus.disconnected
        ? [_peerSession]
        : <PeerSessionModel>[];
    return SwarmStatusModel(
      fileHash: fileHash ?? _peerSession.fileHash,
      activePeerCount: activePeers.length,
      peers: activePeers,
      inFlightBlocks: const [],
    );
  }

  @override
  Future<int> clearSources(String fileHash) async {
    final count = _peerSources[fileHash]?.length ?? 0;
    _peerSources.remove(fileHash);
    return count;
  }

  final List<SharedFileModel> _fakeSharedFiles = [];
  InboundListenerStatusModel _fakeListenerStatus =
      InboundListenerStatusModel.idle;
  UploadSessionModel? _fakeActiveUpload;

  @override
  Future<List<SharedFileModel>> listSharedFiles() async {
    return List.unmodifiable(_fakeSharedFiles);
  }

  @override
  Future<SharedFileModel> addSharedFile({
    required String fileHash,
    required String fileName,
    required int fileSize,
    required String filePath,
  }) async {
    _fakeSharedFiles.removeWhere((f) => f.fileHash == fileHash);
    final file = SharedFileModel(
      fileHash: fileHash,
      fileName: fileName,
      fileSize: fileSize,
      filePath: filePath,
      sharedAtUnixMs: DateTime.now().millisecondsSinceEpoch,
      enabled: true,
    );
    _fakeSharedFiles.add(file);
    return file;
  }

  @override
  Future<void> removeSharedFile(String fileHash) async {
    _fakeSharedFiles.removeWhere((f) => f.fileHash == fileHash);
  }

  @override
  Future<SharedFileModel> setSharedFileEnabled(
    String fileHash,
    bool enabled,
  ) async {
    final idx = _fakeSharedFiles.indexWhere((f) => f.fileHash == fileHash);
    if (idx != -1) {
      final cur = _fakeSharedFiles[idx];
      final updated = SharedFileModel(
        fileHash: cur.fileHash,
        fileName: cur.fileName,
        fileSize: cur.fileSize,
        filePath: cur.filePath,
        sharedAtUnixMs: cur.sharedAtUnixMs,
        enabled: enabled,
      );
      _fakeSharedFiles[idx] = updated;
      return updated;
    }
    throw Exception('File not found in shares');
  }

  @override
  Future<InboundListenerStatusModel> startListener({int port = 0}) async {
    _fakeListenerStatus = InboundListenerStatusModel(
      isListening: true,
      listenAddress: '127.0.0.1:${port == 0 ? 39735 : port}',
      activeUploads: _fakeActiveUpload != null ? 1 : 0,
      queuedUploads: 0,
      uploadRateBps: 51200,
    );
    return _fakeListenerStatus;
  }

  @override
  Future<InboundListenerStatusModel> stopListener() async {
    _fakeListenerStatus = InboundListenerStatusModel.idle;
    _fakeActiveUpload = null;
    return _fakeListenerStatus;
  }

  @override
  Future<InboundListenerStatusModel> getListenerStatus() async {
    return _fakeListenerStatus;
  }

  @override
  Future<Map<String, dynamic>> listUploads() async {
    return {
      'active': _fakeActiveUpload != null ? [_fakeActiveUpload] : [],
      'queued': [],
    };
  }

  @override
  Future<UploadSessionModel?> getUploadStatus() async {
    return _fakeActiveUpload;
  }

  @override
  Future<void> cancelUpload(String clientAddress) async {
    if (_fakeActiveUpload?.clientAddress == clientAddress) {
      _fakeActiveUpload = null;
    }
  }

  @override
  Future<SecureIdentityStatusModel> getIdentityStatus() async {
    return const SecureIdentityStatusModel(
      enabled: true,
      hasKey: true,
      pubkeyHash: '31d6cfe0d16ae931b73c59d7e0c089c0',
      keyBits: 384,
    );
  }

  @override
  Future<SecureIdentityStatusModel> rotateIdentity() async {
    return const SecureIdentityStatusModel(
      enabled: true,
      hasKey: true,
      pubkeyHash: 'rotated456789abcdef0123456789abc',
      keyBits: 384,
    );
  }

  final List<PeerCreditModel> _fakeCredits = [
    const PeerCreditModel(
      pubkeyHash: '0123456789abcdef0123456789abcdef',
      pubkeyHex: '0100abcdef12345678',
      uploadedBytes: 10485760,
      downloadedBytes: 52428800,
      creditMultiplier: 10.0,
      firstSeenUnixMs: 1700000000000,
      lastSeenUnixMs: 1700000100000,
    ),
  ];

  @override
  Future<List<PeerCreditModel>> listPeerCredits({int limit = 50}) async {
    return _fakeCredits.take(limit).toList();
  }

  @override
  Future<PeerCreditStatsModel> getPeerCreditStats() async {
    return PeerCreditStatsModel(
      totalPeers: _fakeCredits.length,
      totalUploadedBytes: _fakeCredits.fold(
        0,
        (sum, c) => sum + c.uploadedBytes,
      ),
      totalDownloadedBytes: _fakeCredits.fold(
        0,
        (sum, c) => sum + c.downloadedBytes,
      ),
      activeCreditedPeers: _fakeCredits.length,
    );
  }

  final List<KadContactModel> _fakeKadContacts = [
    const KadContactModel(
      kadId: '0123456789abcdef0123456789abcdef',
      ip: '127.0.0.1',
      udpPort: 39737,
      tcpPort: 39735,
      version: 2,
      verified: true,
      failCount: 0,
      lastSeenUnixMs: 1700000000000,
    ),
  ];

  bool _fakeKadUdpActive = false;
  String? _fakeKadUdpBindAddress;
  int _fakeKadPacketsSent = 0;
  int _fakeKadPacketsReceived = 0;

  @override
  Future<KadStatusModel> getKadStatus() async {
    return KadStatusModel(
      localId: 'deadbeefcafe0123456789abcdef0123',
      totalContacts: _fakeKadContacts.length,
      totalReplacements: 0,
      dbContactsCount: _fakeKadContacts.length,
      activeBuckets: 1,
      networkProfile: _fakeProfile.toKebabCase(),
      isUdpActive: _fakeKadUdpActive,
      udpBindAddress: _fakeKadUdpBindAddress,
      packetsSent: _fakeKadPacketsSent,
      packetsReceived: _fakeKadPacketsReceived,
      activeTransactions: 0,
    );
  }

  @override
  Future<Map<String, dynamic>> startKadUdp({String? bindAddress}) async {
    _fakeKadUdpActive = true;
    _fakeKadUdpBindAddress = bindAddress ?? '127.0.0.1:39737';
    return {
      'boundAddress': _fakeKadUdpBindAddress,
      'status': await getKadStatus(),
    };
  }

  @override
  Future<Map<String, dynamic>> stopKadUdp() async {
    _fakeKadUdpActive = false;
    _fakeKadUdpBindAddress = null;
    return {'stopped': true, 'status': await getKadStatus()};
  }

  @override
  Future<Map<String, dynamic>> pingKadContact({
    required String ip,
    required int port,
    String? kadId,
  }) async {
    if (!_fakeKadUdpActive) {
      throw Exception('Kad UDP socket is not running');
    }
    _fakeKadPacketsSent += 1;
    _fakeKadPacketsReceived += 1;
    final idx = _fakeKadContacts.indexWhere(
      (c) => c.ip == ip && c.udpPort == port,
    );
    if (idx != -1) {
      final old = _fakeKadContacts[idx];
      _fakeKadContacts[idx] = KadContactModel(
        kadId: old.kadId,
        ip: old.ip,
        udpPort: old.udpPort,
        tcpPort: old.tcpPort,
        version: old.version,
        verified: true,
        failCount: 0,
        lastSeenUnixMs: DateTime.now().millisecondsSinceEpoch,
      );
    }
    return {'success': true, 'latencyMs': 5, 'status': await getKadStatus()};
  }

  @override
  Future<Map<String, dynamic>> bootstrapKad({
    required String ip,
    required int port,
  }) async {
    if (!_fakeKadUdpActive) {
      throw Exception('Kad UDP socket is not running');
    }
    _fakeKadPacketsSent += 1;
    _fakeKadPacketsReceived += 1;
    return {
      'success': true,
      'receivedContacts': _fakeKadContacts.length,
      'status': await getKadStatus(),
    };
  }

  @override
  Future<List<KadContactModel>> findKadNode(String targetId) async {
    if (!_fakeKadUdpActive) {
      throw Exception('Kad UDP socket is not running');
    }
    _fakeKadPacketsSent += 1;
    _fakeKadPacketsReceived += 1;
    return _fakeKadContacts;
  }

  @override
  Future<NodesDatInspectReportModel> inspectNodesDat(String filePath) async {
    return NodesDatInspectReportModel(
      version: 2,
      declaredCount: _fakeKadContacts.length,
      validContactsCount: _fakeKadContacts.length,
      rejectedByPolicyCount: 0,
      rejectedByFilterCount: 0,
      sampleContacts: _fakeKadContacts,
    );
  }

  @override
  Future<Map<String, dynamic>> importNodesDat(String filePath) async {
    return {
      'importedCount': _fakeKadContacts.length,
      'status': {
        'localId': 'deadbeefcafe0123456789abcdef0123',
        'totalContacts': _fakeKadContacts.length,
        'totalReplacements': 0,
        'dbContactsCount': _fakeKadContacts.length,
        'activeBuckets': 1,
        'networkProfile': _fakeProfile.toKebabCase(),
      },
    };
  }

  @override
  Future<List<KadContactModel>> listKadContacts({int limit = 100}) async {
    return _fakeKadContacts.take(limit).toList();
  }

  @override
  Future<int> clearKadContacts() async {
    final count = _fakeKadContacts.length;
    _fakeKadContacts.clear();
    return count;
  }

  @override
  Future<List<PeerSourceModel>> searchKadSources({
    required String fileHash,
    int? fileSize,
  }) async {
    return [];
  }

  @override
  Future<List<SearchResultFileModel>> searchKadKeywords({
    required String term,
    int? minSize,
    int? maxSize,
  }) async {
    return [
      SearchResultFileModel(
        fileHash: 'deadbeef0123456789abcdef01234567',
        fileName: '$term-simulation-file.iso',
        fileSize: 1024 * 1024 * 700,
        sourceCount: 12,
      ),
      SearchResultFileModel(
        fileHash: 'cafebabe0123456789abcdef01234567',
        fileName: '$term-sample-archive.zip',
        fileSize: 1024 * 1024 * 120,
        sourceCount: 5,
      ),
    ];
  }

  @override
  Future<int> publishKadSource({
    required String fileHash,
    required int fileSize,
    int tcpPort = 39735,
  }) async {
    return 0;
  }

  @override
  Future<int> publishKadKeywords({
    required String fileHash,
    required String fileName,
    required int fileSize,
  }) async {
    return 1;
  }

  UpnpStatusModel _upnpStatus = const UpnpStatusModel(
    enabled: true,
    available: true,
    gatewayAddr: '192.168.1.1:5678',
    externalIp: '198.51.100.42',
    tcpMapped: true,
    udpMapped: true,
    tcpPort: 39735,
    udpPort: 39737,
  );

  @override
  Future<UpnpStatusModel> getUpnpStatus() async {
    return _upnpStatus;
  }

  @override
  Future<UpnpStatusModel> setUpnpEnabled(bool enabled) async {
    _upnpStatus = UpnpStatusModel(
      enabled: enabled,
      available: enabled ? _upnpStatus.available : false,
      gatewayAddr: enabled ? _upnpStatus.gatewayAddr : null,
      externalIp: enabled ? _upnpStatus.externalIp : null,
      tcpMapped: enabled,
      udpMapped: enabled,
      tcpPort: _upnpStatus.tcpPort,
      udpPort: _upnpStatus.udpPort,
      updatedAtUnixMs: DateTime.now().millisecondsSinceEpoch,
    );
    return _upnpStatus;
  }

  @override
  Future<UpnpStatusModel> mapUpnpPorts({
    int tcpPort = 39735,
    int udpPort = 39737,
  }) async {
    _upnpStatus = UpnpStatusModel(
      enabled: true,
      available: true,
      gatewayAddr: '192.168.1.1:5678',
      externalIp: '198.51.100.42',
      tcpMapped: true,
      udpMapped: true,
      tcpPort: tcpPort,
      udpPort: udpPort,
      updatedAtUnixMs: DateTime.now().millisecondsSinceEpoch,
    );
    return _upnpStatus;
  }

  @override
  Future<UpnpStatusModel> unmapUpnpPorts() async {
    _upnpStatus = UpnpStatusModel(
      enabled: _upnpStatus.enabled,
      available: _upnpStatus.available,
      gatewayAddr: _upnpStatus.gatewayAddr,
      externalIp: _upnpStatus.externalIp,
      tcpMapped: false,
      udpMapped: false,
      tcpPort: _upnpStatus.tcpPort,
      udpPort: _upnpStatus.udpPort,
      updatedAtUnixMs: DateTime.now().millisecondsSinceEpoch,
    );
    return _upnpStatus;
  }

  @override
  Future<Map<String, dynamic>> updateServersFromUrl(String url) async {
    return {
      'url': url,
      'totalCount': 5,
      'allowedCount': 5,
      'blockedCount': 0,
      'imported': 5,
    };
  }

  @override
  Future<Map<String, dynamic>> bootstrapKadFromUrl(String url) async {
    return {
      'url': url,
      'importedCount': 10,
      'status': {'activeContacts': 10},
    };
  }

  @override
  void dispose() {
    _simulationTimer?.cancel();
    _connectionController.close();
    _eventsController.close();
    _searchResultController.close();
    _foundSourcesController.close();
    _peerSessionController.close();
    _serverSessionController.close();
    _uploadsController.close();
    _statsController.close();
    _lifecycleController.close();
    _kadStatusController.close();
    _historyController.close();
    _settingsController.close();
    _networkPolicyController.close();
  }
}
