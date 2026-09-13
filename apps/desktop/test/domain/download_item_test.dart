import 'package:flutter_test/flutter_test.dart';
import 'package:desktop/domain/models/download_item.dart';
import 'package:desktop/ui/core/formatters.dart';

void main() {
  group('DownloadItem and Status Domain Tests', () {
    test('DownloadStatus property helpers work as expected', () {
      expect(DownloadStatus.downloading.isActive, isTrue);
      expect(DownloadStatus.allocating.isActive, isTrue);
      expect(DownloadStatus.verifying.isActive, isTrue);
      expect(DownloadStatus.completing.isActive, isTrue);
      expect(DownloadStatus.paused.isActive, isFalse);
      expect(DownloadStatus.paused.isPaused, isTrue);
      expect(DownloadStatus.completed.isCompleted, isTrue);
      expect(DownloadStatus.failed.isFailed, isTrue);
    });

    test('DownloadItem remainingBytes and progress clamping work', () {
      final now = DateTime.now();
      final item = DownloadItem(
        id: 'test-1',
        fileName: 'test.iso',
        totalBytes: 1000,
        receivedBytes: 600,
        verifiedBytes: 500,
        progress: 0.6,
        status: DownloadStatus.downloading,
        priority: DownloadPriority.normal,
        sourceCount: 5,
        downloadRateBps: 100,
        uploadedBytes: 0,
        createdAt: now,
        updatedAt: now,
      );

      expect(item.remainingBytes, 400);

      // copyWith with out-of-bound progress is clamped
      final updated = item.copyWith(progress: 1.5, receivedBytes: 1000);
      expect(updated.progress, 1.0);
      expect(updated.remainingBytes, 0);

      final clampedLow = item.copyWith(progress: -0.2);
      expect(clampedLow.progress, 0.0);
    });
  });

  group('Formatters Tests', () {
    test('formatBytes formats cleanly across magnitudes', () {
      expect(Formatters.formatBytes(500), '500 B');
      expect(Formatters.formatBytes(1536), '1.5 KB');
      expect(Formatters.formatBytes(10485760), '10.0 MB');
      expect(Formatters.formatBytes(1073741824), '1.00 GB');
    });

    test('formatSpeed formats transfer rates', () {
      expect(Formatters.formatSpeed(0), '0 B/s');
      expect(Formatters.formatSpeed(512), '512 B/s');
      expect(Formatters.formatSpeed(204800), '200.0 KB/s');
      expect(Formatters.formatSpeed(2621440), '2.50 MB/s');
    });

    test('formatEta calculates remaining time string', () {
      expect(Formatters.formatEta(0, 1000), '∞');
      expect(Formatters.formatEta(1000, 0), '∞');
      expect(Formatters.formatEta(450, 10), '45s');
      expect(Formatters.formatEta(1250, 10), '2m 5s');
      expect(Formatters.formatEta(7300, 1), '2h 1m');
    });

    test('formatPercent displays clamped percentage', () {
      expect(Formatters.formatPercent(0.0), '0.0%');
      expect(Formatters.formatPercent(0.4285), '42.9%');
      expect(Formatters.formatPercent(1.0), '100.0%');
      expect(Formatters.formatPercent(1.5), '100.0%');
    });
  });
}
