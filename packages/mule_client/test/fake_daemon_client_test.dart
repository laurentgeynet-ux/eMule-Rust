import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';

void main() {
  group('FakeMuleDaemonClient Tests', () {
    late FakeMuleDaemonClient client;

    setUp(() {
      client = FakeMuleDaemonClient(enableAutoSimulation: false);
    });

    tearDown(() {
      client.dispose();
    });

    test('connect establishes session and returns daemon info', () async {
      expect(client.currentConnectionState, DaemonConnectionState.disconnected);

      final info = await client.connect(url: Uri.parse('ws://127.0.0.1:4660/rpc'));

      expect(client.currentConnectionState, DaemonConnectionState.connected);
      expect(info.protocolVersion, 1);
      expect(info.capabilities, contains('downloads'));
    });

    test('listDownloads returns initial seeded items', () async {
      await client.connect(url: Uri.parse('ws://127.0.0.1:4660/rpc'));
      final list = await client.listDownloads();

      expect(list.length, 3);
      expect(list.any((d) => d.fileName == 'Debian_12_netinst.iso'), isTrue);
      expect(list.any((d) => d.status == DownloadStatus.downloading), isTrue);
      expect(list.any((d) => d.status == DownloadStatus.paused), isTrue);
      expect(list.any((d) => d.status == DownloadStatus.completed), isTrue);
    });

    test('addEd2kLink adds new item and emits update', () async {
      await client.connect(url: Uri.parse('ws://127.0.0.1:4660/rpc'));

      const link = 'ed2k://|file|test_archive.zip|1000000|0123456789abcdef0123456789abcdef|/';
      final futureEvent = client.downloadEvents.first;

      final id = await client.addEd2kLink(link);
      expect(id, startsWith('fake-'));

      final event = await futureEvent;
      expect(event.download.id, id);
      expect(event.download.fileName, 'test_archive.zip');
      expect(event.download.totalBytes, 1000000);

      final dl = await client.getDownload(id);
      expect(dl, isNotNull);
      expect(dl!.fileName, 'test_archive.zip');
    });

    test('pauseDownload and resumeDownload update state', () async {
      await client.connect(url: Uri.parse('ws://127.0.0.1:4660/rpc'));
      final list = await client.listDownloads();
      final active = list.firstWhere((d) => d.status == DownloadStatus.downloading);

      final paused = await client.pauseDownload(active.id);
      expect(paused.status, DownloadStatus.paused);
      expect(paused.downloadRateBps, 0);

      final resumed = await client.resumeDownload(active.id);
      expect(resumed.status, DownloadStatus.downloading);
      expect(resumed.downloadRateBps, greaterThan(0));
    });

    test('injectRange simulates data arrival and marks completed on full bytes', () async {
      await client.connect(url: Uri.parse('ws://127.0.0.1:4660/rpc'));
      final list = await client.listDownloads();
      final target = list.firstWhere((d) => d.fileName == 'Blender_Benchmark_Scene.blend');

      // Inject 4 bytes (8 hex characters)
      final updated = await client.injectRange(target.id, target.receivedBytes, 'AABBCCDD');
      expect(updated.receivedBytes, target.receivedBytes + 4);

      // Inject remaining bytes to complete
      final remaining = target.totalBytes - updated.receivedBytes;
      final hexRemaining = '00' * remaining;
      final completed = await client.injectRange(target.id, updated.receivedBytes, hexRemaining);

      expect(completed.receivedBytes, target.totalBytes);
      expect(completed.progress, 1.0);
      expect(completed.status, DownloadStatus.completed);
    });

    test('seeded download exposes coherent receivedMask and pendingRanges', () async {
      await client.connect(url: Uri.parse('ws://127.0.0.1:4660/rpc'));
      final list = await client.listDownloads();
      final active = list.firstWhere((d) => d.status == DownloadStatus.downloading);

      // Pending ranges stay within file bounds and overlap partial parts.
      for (final r in active.pendingRanges) {
        expect(r.start, greaterThanOrEqualTo(0));
        expect(r.end, lessThanOrEqualTo(active.totalBytes));
        expect(r.end, greaterThan(r.start));
      }

      // Partial parts carry a non-trivial mask; verified parts are full.
      for (final p in active.parts) {
        if (p.state == 'downloading') {
          expect(p.receivedMask, isNot(0));
          expect(p.receivedMask, isNot(-1));
        }
      }

      final completed = list.firstWhere((d) => d.status == DownloadStatus.completed);
      expect(completed.pendingRanges, isEmpty);
      expect(completed.parts.every((p) => p.receivedMask == -1), isTrue);
    });

    test('removeDownload deletes item from collection', () async {
      await client.connect(url: Uri.parse('ws://127.0.0.1:4660/rpc'));
      final list = await client.listDownloads();
      final targetId = list.first.id;

      await client.removeDownload(targetId);
      final remaining = await client.listDownloads();
      expect(remaining.any((d) => d.id == targetId), isFalse);
    });
  });
}
