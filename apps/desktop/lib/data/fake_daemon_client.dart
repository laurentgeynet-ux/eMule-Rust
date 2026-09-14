import 'dart:async';
import 'dart:math';

import '../../domain/models/daemon_event.dart';
import '../../domain/models/daemon_info.dart';
import '../../domain/models/download_item.dart';
import '../../domain/models/lab_models.dart';
import '../../domain/services/mule_daemon_client.dart';

class FakeMuleDaemonClient implements MuleDaemonClient {
  final Map<String, DownloadItem> _downloads = {};
  final List<LabFixtureInfo> _fixtures = [];
  String? _activePeer;
  final StreamController<DaemonConnectionState> _connectionController =
      StreamController<DaemonConnectionState>.broadcast();
  final StreamController<DownloadUpdatedEvent> _eventsController =
      StreamController<DownloadUpdatedEvent>.broadcast();

  DaemonConnectionState _state = DaemonConnectionState.disconnected;
  Timer? _simulationTimer;
  int _sequence = 1;
  final bool enableAutoSimulation;

  FakeMuleDaemonClient({this.enableAutoSimulation = true}) {
    _seedDefaultDownloads();
  }

  void _seedDefaultDownloads() {
    final now = DateTime.now();

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
    );

    _downloads[d1.id] = d1;
    _downloads[d2.id] = d2;
    _downloads[d3.id] = d3;
  }

  @override
  DaemonConnectionState get currentConnectionState => _state;

  @override
  Stream<DaemonConnectionState> get connectionState => _connectionController.stream;

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

    return const DaemonInfo(
      version: '0.1.0-fake',
      protocolVersion: 1,
      instanceId: 'fake-instance-id',
      sessionId: 'fake-session-id',
      capabilities: ['downloads', 'download-events', 'local-simulation', 'lab-tools'],
      workdirMode: 'Portable',
      workdirPath: 'C:\\Fake\\Portable\\data',
    );
  }

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
  }) async {
    final parts = link.split('|');
    String fileName = 'download_${DateTime.now().millisecondsSinceEpoch}.bin';
    int totalBytes = 10000000;

    if (parts.length >= 5 && parts[1].toLowerCase() == 'file') {
      fileName = Uri.decodeComponent(parts[2]);
      totalBytes = int.tryParse(parts[3]) ?? 10000000;
    }

    final id = 'fake-${DateTime.now().millisecondsSinceEpoch}-${Random().nextInt(9999)}';
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
    );

    _downloads[id] = item;
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
  }

  @override
  Future<DownloadItem> injectRange(String id, int offset, String hexData) async {
    final existing = _downloads[id];
    if (existing == null) throw Exception('Download $id not found');

    final bytesCount = hexData.length ~/ 2;
    final newReceived = min(existing.totalBytes, existing.receivedBytes + bytesCount);
    final newProgress = existing.totalBytes > 0 ? newReceived / existing.totalBytes : 1.0;
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

  void _emitUpdate(DownloadItem item) {
    _sequence += 1;
    if (!_eventsController.isClosed) {
      _eventsController.add(DownloadUpdatedEvent(
        sequence: _sequence,
        download: item,
      ));
    }
  }

  void _startSimulation() {
    _simulationTimer = Timer.periodic(const Duration(milliseconds: 1000), (timer) {
      for (final item in _downloads.values.toList()) {
        if (item.status == DownloadStatus.downloading) {
          final added = 350000 + Random().nextInt(400000);
          final newReceived = min(item.totalBytes, item.receivedBytes + added);
          final isDone = newReceived >= item.totalBytes;
          final newProgress = item.totalBytes > 0 ? newReceived / item.totalBytes : 1.0;

          final updated = item.copyWith(
            receivedBytes: newReceived,
            verifiedBytes: min(newReceived, item.verifiedBytes + added),
            progress: newProgress,
            downloadRateBps: isDone ? 0 : (added * 1.0).toInt(),
            status: isDone ? DownloadStatus.completed : DownloadStatus.downloading,
            updatedAt: DateTime.now(),
          );

          _downloads[item.id] = updated;
          _emitUpdate(updated);
        }
      }
    });
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
    _activePeer = '127.0.0.1:4662';
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
  Future<void> connectDownloadToSource(String downloadId, String peerAddr) async {
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

  @override
  void dispose() {
    _simulationTimer?.cancel();
    _connectionController.close();
    _eventsController.close();
  }
}
