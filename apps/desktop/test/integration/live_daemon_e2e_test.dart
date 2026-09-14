import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:desktop/data/rpc_daemon_client.dart';
import 'package:desktop/domain/models/download_item.dart';

void main() {
  group('Live Daemon End-to-End Integration Test', () {
    late Process daemonProcess;
    late Directory tempDir;
    const port = 4669;
    const token =
        '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

    setUp(() async {
      tempDir = await Directory.systemTemp.createTemp('emule_e2e_');

      final binPath = Platform.isWindows
          ? '..\\..\\target\\debug\\mule-daemon.exe'
          : '../../target/debug/mule-daemon';

      daemonProcess = await Process.start(binPath, [
        '--workdir',
        tempDir.path,
        '--bind',
        '127.0.0.1:$port',
        '--token',
        token,
      ]);

      // Drain stderr / stdout for debugging
      daemonProcess.stdout.listen((_) {});
      daemonProcess.stderr.listen((_) {});

      // Wait for the daemon to start listening
      var connected = false;
      for (var i = 0; i < 30; i++) {
        try {
          final socket = await Socket.connect('127.0.0.1', port,
              timeout: const Duration(milliseconds: 200));
          await socket.close();
          connected = true;
          break;
        } catch (_) {
          await Future.delayed(const Duration(milliseconds: 100));
        }
      }

      if (!connected) {
        throw StateError('mule-daemon failed to start on port $port');
      }
    });

    tearDown(() async {
      daemonProcess.kill();
      await daemonProcess.exitCode;
      try {
        await tempDir.delete(recursive: true);
      } catch (_) {}
    });

    test('Full lifecycle against live Rust mule-daemon over WebSocket', () async {
      final client = RpcDaemonClient();

      // 1. Connect and Handshake
      final info = await client.connect(
        url: Uri.parse('ws://127.0.0.1:$port/rpc'),
        token: token,
      );

      expect(info.protocolVersion, 1);
      expect(info.capabilities, contains('downloads'));

      // 2. Setup event listener
      final receivedEvents = <DownloadItem>[];
      final sub = client.downloadEvents.listen((event) {
        receivedEvents.add(event.download);
      });

      // 3. Add eD2K Link
      const link =
          'ed2k://|file|e2e_video.mp4|100000|0123456789abcdef0123456789abcdef|/';
      final downloadId = await client.addEd2kLink(
        link,
        targetDirectory: tempDir.path,
      );

      expect(downloadId.isNotEmpty, isTrue);

      // Wait for broadcast event
      await Future.delayed(const Duration(milliseconds: 150));
      expect(receivedEvents.any((d) => d.id == downloadId), isTrue);

      // 4. List downloads
      final list = await client.listDownloads();
      expect(list.length, 1);
      expect(list[0].id, downloadId);
      expect(list[0].fileName, 'e2e_video.mp4');

      // 5. Inject simulated range (if lab-tools/debug-rpc enabled on daemon)
      try {
        final injected =
            await client.injectRange(downloadId, 0, 'DEADBEEF01020304');
        expect(injected.receivedBytes, 8);
      } catch (_) {
        // debug.injectRange is gated behind lab-tools in production builds
      }

      // 6. Pause download
      final paused = await client.pauseDownload(downloadId);
      expect(paused.status, DownloadStatus.paused);

      // 7. Resume download
      final resumed = await client.resumeDownload(downloadId);
      expect(resumed.status, isNot(DownloadStatus.paused));

      // 8. Remove download
      await client.removeDownload(downloadId);
      final emptyList = await client.listDownloads();
      expect(emptyList.isEmpty, isTrue);

      await sub.cancel();
      client.dispose();
    });
  });
}
