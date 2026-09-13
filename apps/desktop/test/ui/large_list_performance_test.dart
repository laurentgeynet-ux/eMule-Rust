import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:desktop/domain/models/download_item.dart';
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
  Future<void> disconnect() async {}

  @override
  Future<List<DownloadItem>> listDownloads() async => _items;

  @override
  Future<DownloadItem?> getDownload(String id) async =>
      _items.firstWhere((d) => d.id == id);

  @override
  Future<String> addEd2kLink(String link, {String? targetDirectory, String? allocationPolicy}) async => 'new-id';

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
