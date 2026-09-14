import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:desktop/domain/models/category_item.dart';
import 'package:desktop/domain/models/download_item.dart';
import 'package:desktop/domain/models/history_entry.dart';
import 'package:desktop/domain/models/history_stats.dart';
import 'package:desktop/domain/models/lab_models.dart';
import 'package:desktop/domain/models/part_met_models.dart';
import 'package:desktop/domain/services/mule_daemon_client.dart';
import 'package:desktop/domain/models/daemon_info.dart';
import 'package:desktop/domain/models/daemon_event.dart';
import 'package:desktop/main.dart';
import 'package:desktop/ui/features/downloads/view_models/downloads_view_model.dart';
import 'package:desktop/ui/features/downloads/widgets/download_row.dart';

class LargeListMockClient implements MuleDaemonClient {
  final List<DownloadItem> _items = [];

  LargeListMockClient(int count) {
    final now = DateTime.now();
    for (var i = 1; i <= count; i++) {
      _items.add(DownloadItem(
        id: 'bulk-item-$i',
        fileName: 'archive_part_$i.tar.gz',
        totalBytes: 50000000 + i * 1000,
        receivedBytes: (i % 2 == 0) ? 25000000 : 50000000 + i * 1000,
        verifiedBytes: (i % 2 == 0) ? 25000000 : 50000000 + i * 1000,
        progress: (i % 2 == 0) ? 0.5 : 1.0,
        status: (i % 2 == 0) ? DownloadStatus.downloading : DownloadStatus.completed,
        priority: DownloadPriority.normal,
        sourceCount: (i % 20) + 1,
        downloadRateBps: (i % 2 == 0) ? 500000 : 0,
        uploadedBytes: 10000,
        createdAt: now.subtract(Duration(minutes: i)),
        updatedAt: now,
      ));
    }
  }

  @override
  DaemonInfo? get daemonInfo => null;

  @override
  DaemonConnectionState get currentConnectionState => DaemonConnectionState.connected;

  @override
  Stream<DaemonConnectionState> get connectionState => const Stream.empty();

  @override
  Stream<DownloadUpdatedEvent> get downloadEvents => const Stream.empty();

  @override
  Future<DaemonInfo> connect({required Uri url, String? token}) async {
    return const DaemonInfo(
      version: '1.0.0',
      protocolVersion: 1,
      instanceId: 'large-list-inst',
      sessionId: 'sess',
      capabilities: ['downloads'],
    );
  }

  @override
  Future<void> stopDaemon() async {}

  @override
  Future<void> disconnect() async {}

  @override
  Future<List<DownloadItem>> listDownloads() async => _items;

  @override
  Future<DownloadItem?> getDownload(String id) async =>
      _items.firstWhere((d) => d.id == id);

  @override
  Future<String> addEd2kLink(String link, {String? targetDirectory, String? allocationPolicy, String? categoryId}) async => 'new-id';

  @override
  Future<DownloadItem> pauseDownload(String id) async =>
      _items.firstWhere((d) => d.id == id);

  @override
  Future<DownloadItem> resumeDownload(String id) async =>
      _items.firstWhere((d) => d.id == id);

  @override
  Future<void> removeDownload(String id, {bool deleteFiles = false}) async {}

  @override
  Future<DownloadItem> injectRange(String id, int offset, String hexData) async =>
      _items.firstWhere((d) => d.id == id);

  @override
  Future<LabStatus> getLabStatus() async => const LabStatus(
        workdirMode: 'Portable',
        workdirPath: 'test_path',
        isPortable: true,
        fixtures: [],
      );

  @override
  Future<LabFixtureInfo> createLabFixture(String fixtureType) async =>
      const LabFixtureInfo(
        fileName: 'test.bin',
        filePath: 'test.bin',
        sizeBytes: 1024,
        ed2kHash: '0123456789abcdef0123456789abcdef',
        ed2kLink: 'ed2k://|file|test.bin|1024|0123456789abcdef0123456789abcdef|/',
      );

  @override
  Future<LabPeerInfo> startLabPeer() async => const LabPeerInfo(
        peerAddr: '127.0.0.1:4662',
        sharedCount: 1,
        status: 'running',
      );

  @override
  Future<void> stopLabPeer() async {}

  @override
  Future<LabCleanupReport> cleanupLabTestData() async => const LabCleanupReport(
        success: true,
        cleanedFolders: ['fixtures'],
        deletedFilesCount: 0,
        freedBytes: 0,
        message: 'Mock cleanup',
      );

  @override
  Future<void> connectDownloadToSource(String downloadId, String peerAddr) async {}

  @override
  Future<DownloadItem> setDownloadDestination(String id, String targetDirectory) async =>
      _items.firstWhere((d) => d.id == id);

  @override
  Future<DownloadItem> retryDownloadConnection(String id) async =>
      _items.firstWhere((d) => d.id == id);

  @override
  Future<Map<String, dynamic>> getSettings() async => {
        'defaultDownloadDir': 'downloads',
        'portableDownloadDir': 'downloads',
        'isCustom': false,
      };

  @override
  Future<void> setDefaultDownloadDirectory(String path) async {}

  @override
  Future<void> resetDefaultDownloadDirectory() async {}

  @override
  Future<List<HistoryEntry>> listHistory({
    String? eventType,
    String? status,
    int? fromTimestampMs,
    int? toTimestampMs,
    int? limit,
    int? offset,
  }) async => [];

  @override
  Future<HistoryStats> getHistoryStats() async => const HistoryStats(
        sessionCompletedCount: 0,
        sessionCompletedBytes: 0,
        last24hCompletedCount: 0,
        last24hCompletedBytes: 0,
        last7dCompletedCount: 0,
        last7dCompletedBytes: 0,
        totalCompletedCount: 0,
        totalCompletedBytes: 0,
        totalFailedCount: 0,
        totalResumesCount: 0,
        totalIntegrityFailures: 0,
      );

  @override
  Future<int> clearHistory() async => 0;

  @override
  Future<String> exportHistory(String format, {String? eventType, String? status}) async => '';

  @override
  Future<void> setHistoryRetentionDays(int days) async {}

  @override
  Future<List<CategoryItem>> listCategories() async => [];

  @override
  Future<CategoryItem> createCategory({
    required String name,
    required String color,
    String? defaultDestination,
    String? defaultPriority,
  }) async =>
      CategoryItem(
        id: 'mock-cat',
        name: name,
        color: color,
        createdAt: DateTime.now(),
        updatedAt: DateTime.now(),
      );

  @override
  Future<CategoryItem> updateCategory(
    String id, {
    String? name,
    String? color,
    String? defaultDestination,
    String? defaultPriority,
    int? sortOrder,
  }) async =>
      CategoryItem(
        id: id,
        name: name ?? 'Updated',
        color: color ?? '#2196F3',
        createdAt: DateTime.now(),
        updatedAt: DateTime.now(),
      );

  @override
  Future<void> deleteCategory(String id) async {}

  @override
  Future<void> reorderCategories(List<String> categoryIds) async {}

  @override
  Future<DownloadItem> setDownloadCategory(String downloadId, String? categoryId) async =>
      _items.firstWhere((d) => d.id == downloadId);

  @override
  Future<PartMetInspectionResult> inspectPartMet(String metPath, {String? partPath}) async =>
      PartMetInspectionResult(
        fileName: 'mock.bin',
        totalSize: 1000,
        ed2kHash: '00000000000000000000000000000000',
        partCount: 1,
        partMetPath: metPath,
        partFileExists: true,
        estimatedTransferredBytes: 1000,
      );

  @override
  Future<ImportPartMetResult> importPartMet(
    String metPath, {
    String? partPath,
    String? targetDirectory,
    String? categoryId,
  }) async =>
      ImportPartMetResult(
        download: _items.first,
        totalParts: 1,
        verifiedParts: 1,
        corruptedParts: 0,
        transferredBytes: 1000,
        totalSize: 1000,
      );

  @override
  void dispose() {}
}

void main() {
  testWidgets('Large list virtualization with 500 items renders and scrolls without lag',
      (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    final mockClient = LargeListMockClient(500);
    final viewModel = DownloadsViewModel(client: mockClient);
    await viewModel.initialize(url: Uri.parse('ws://mock:4660/rpc'));

    await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
    await tester.pumpAndSettle();

    // Verify initial viewport renders visible items
    expect(find.byType(DownloadRow), findsWidgets);
    expect(find.text('archive_part_1.tar.gz'), findsOneWidget);

    // Scroll down 2000 pixels
    await tester.drag(find.byType(ListView), const Offset(0, -2000));
    await tester.pumpAndSettle();

    // Elements off screen should be unmounted (virtualized)
    expect(find.byType(DownloadRow), findsWidgets);
    expect(find.text('archive_part_1.tar.gz'), findsNothing);

    // Filter to active items only
    viewModel.setFilter(DownloadFilter.active);
    await tester.pumpAndSettle();

    expect(viewModel.filteredDownloads.length, 250);
  });
}
