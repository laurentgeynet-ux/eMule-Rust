// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

void main() {
  group('DownloadItem and Status Domain Tests', () {
    test('DownloadStatus property helpers work as expected', () {
      expect(DownloadStatus.downloading.isActive, isTrue);
      expect(DownloadStatus.allocating.isActive, isTrue);
      expect(DownloadStatus.waitingForSources.isActive, isTrue);
      expect(DownloadStatus.waitingForSources.label, 'Waiting for Sources');
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

    test('fromJson parses receivedMask and pendingRanges (tuple form)', () {
      final item = DownloadItem.fromJson({
        'id': 'd1',
        'fileName': 'f.iso',
        'totalBytes': 9728000,
        'receivedBytes': 4864000,
        'progress': 0.5,
        'status': 'downloading',
        'parts': [
          {
            'partIndex': 0,
            'startOffset': 0,
            'endOffset': 9728000,
            'size': 9728000,
            'receivedBytes': 4864000,
            'isVerified': false,
            'state': 'downloading',
            'availableSourceCount': 3,
            'receivedMask': 4294967295, // first 32 of 64 cells
          }
        ],
        'pendingRanges': [
          [4864000, 5048320],
          {'start': 5200000, 'end': 5384320},
        ],
      });

      expect(item.parts, hasLength(1));
      expect(item.parts.first.receivedMask, 4294967295);
      expect(item.parts.first.availableSourceCount, 3);
      expect(item.pendingRanges, hasLength(2));
      expect(item.pendingRanges.first.start, 4864000);
      expect(item.pendingRanges.first.end, 5048320);
      expect(item.pendingRanges.last.start, 5200000);
    });

    test('fromJson defaults mask and pendingRanges when absent', () {
      final item = DownloadItem.fromJson({
        'id': 'd2',
        'fileName': 'f.iso',
        'totalBytes': 1000,
        'parts': [
          {'partIndex': 0, 'size': 1000, 'state': 'missing'}
        ],
      });

      expect(item.parts.first.receivedMask, 0);
      expect(item.pendingRanges, isEmpty);
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
