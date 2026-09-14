import 'dart:async';
import 'dart:convert';
import 'dart:math';

import '../../domain/models/category_item.dart';
import '../../domain/models/daemon_event.dart';
import '../../domain/models/daemon_info.dart';
import '../../domain/models/download_item.dart';
import '../../domain/models/history_entry.dart';
import '../../domain/models/history_stats.dart';
import '../../domain/models/lab_models.dart';
import '../../domain/models/part_met_models.dart';
import '../../domain/services/mule_daemon_client.dart';

class FakeMuleDaemonClient implements MuleDaemonClient {
  final Map<String, DownloadItem> _downloads = {};
  final Map<String, CategoryItem> _categories = {};
  final List<LabFixtureInfo> _fixtures = [];
  final List<HistoryEntry> _history = [];
  int _fakeRetentionDays = 30;
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

  static List<DownloadPartInfo> _generateParts(int totalBytes, int receivedBytes, bool isCompleted) {
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
          : (partReceived == size ? 'completed' : (partReceived > 0 ? 'downloading' : 'missing'));

      list.add(DownloadPartInfo(
        partIndex: i,
        startOffset: start,
        endOffset: end,
        size: size,
        receivedBytes: partReceived,
        isVerified: isVerified,
        state: state,
      ));
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

    _history.add(HistoryEntry(
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
    ));
    _history.add(HistoryEntry(
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
    ));
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

    _daemonInfo = const DaemonInfo(
      version: '0.1.0-fake',
      protocolVersion: 1,
      instanceId: 'fake-instance-id',
      sessionId: 'fake-session-id',
      capabilities: ['downloads', 'download-events', 'lab-tools'],
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
      targetPath: targetDirectory != null ? '$targetDirectory/$fileName' : null,
      categoryId: categoryId,
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

  String? _fakeCustomDownloadDir;

  @override
  Future<DownloadItem> setDownloadDestination(String id, String targetDirectory) async {
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
      if (eventType != null && eventType.isNotEmpty && e.eventType != eventType) {
        return false;
      }
      if (status != null && status.isNotEmpty && e.status != status) {
        return false;
      }
      if (fromTimestampMs != null && e.timestamp.millisecondsSinceEpoch < fromTimestampMs) {
        return false;
      }
      if (toTimestampMs != null && e.timestamp.millisecondsSinceEpoch > toTimestampMs) {
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
    final completedBytes = completed.fold<int>(0, (sum, e) => sum + e.totalBytes);
    final failedCount = _history.where((e) => e.eventType == 'failed').length;
    final resumesCount = _history.where((e) => e.eventType == 'resumed').length;
    final integrityFailures = _history.where((e) => e.verificationResult == 'hash_mismatch').length;

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
  Future<String> exportHistory(String format, {String? eventType, String? status}) async {
    final entries = await listHistory(eventType: eventType, status: status);
    if (format.toLowerCase() == 'csv') {
      final buffer = StringBuffer(
          'id,download_id,file_name,total_size,transferred_bytes,ed2k_hash,event_type,status,duration_secs,connection_attempts,resume_count,verification_result,error_message,timestamp_unix_ms\n');
      for (final e in entries) {
        buffer.writeln(
            '${e.id},${e.downloadId},"${e.fileName}",${e.totalBytes},${e.transferredBytes},${e.ed2kHash},${e.eventType},${e.status},${e.durationSecs},${e.connectionAttempts},${e.resumeCount},${e.verificationResult},"${e.errorMessage ?? ''}",${e.timestamp.millisecondsSinceEpoch}');
      }
      return buffer.toString();
    } else {
      return jsonEncode(entries
          .map((e) => {
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
              })
          .toList());
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
    final id = 'fake-cat-${DateTime.now().millisecondsSinceEpoch}-${_categories.length}';
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
        _eventsController.add(DownloadUpdatedEvent(sequence: _sequence++, download: updatedDl));
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
  Future<DownloadItem> setDownloadCategory(String downloadId, String? categoryId) async {
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
    _eventsController.add(DownloadUpdatedEvent(sequence: _sequence++, download: updated));
    return updated;
  }

  @override
  Future<PartMetInspectionResult> inspectPartMet(String metPath, {String? partPath}) async {
    final fileName = metPath.split(RegExp(r'[\\/]')).last.replaceAll('.part.met', '.bin');
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
    _eventsController.add(DownloadUpdatedEvent(sequence: _sequence++, download: dl));

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
  void dispose() {
    _simulationTimer?.cancel();
    _connectionController.close();
    _eventsController.close();
  }
}
