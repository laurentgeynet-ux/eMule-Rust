import 'package:app_shell/ui/features/downloads/view_models/downloads_store_controller.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';

DownloadItem _item(
  String id, {
  required double progress,
  required DownloadStatus status,
  DateTime? createdAt,
}) {
  final now = createdAt ?? DateTime(2026, 1, 1);
  return DownloadItem(
    id: id,
    fileName: '$id.bin',
    totalBytes: 1000,
    receivedBytes: (progress * 1000).round(),
    verifiedBytes: 0,
    progress: progress,
    status: status,
    priority: DownloadPriority.normal,
    sourceCount: 0,
    downloadRateBps: 0,
    uploadedBytes: 0,
    createdAt: now,
    updatedAt: now,
  );
}

void main() {
  group('compareDownloadsForList', () {
    test('active downloads sort by progress descending', () {
      final list = [
        _item('low', progress: 0.10, status: DownloadStatus.downloading),
        _item('mid', progress: 0.50, status: DownloadStatus.downloading),
        _item('top', progress: 0.95, status: DownloadStatus.downloading),
      ]..sort(compareDownloadsForList);

      expect(list.map((d) => d.id), ['top', 'mid', 'low']);
    });

    test('completed downloads sink to the bottom regardless of progress', () {
      final list = [
        _item('done-old', progress: 1.0, status: DownloadStatus.completed),
        _item('active', progress: 0.02, status: DownloadStatus.downloading),
        _item('done-new', progress: 1.0, status: DownloadStatus.completed,
            createdAt: DateTime(2026, 1, 2)),
      ]..sort(compareDownloadsForList);

      expect(list.first.id, 'active');
      // Parmi les terminés (progress à égalité), les plus récents d'abord.
      expect(list.sublist(1).map((d) => d.id), ['done-new', 'done-old']);
    });

    test('paused/failed keep their progress rank with actives', () {
      final list = [
        _item('paused', progress: 0.40, status: DownloadStatus.paused),
        _item('failed', progress: 0.80, status: DownloadStatus.failed),
        _item('live', progress: 0.60, status: DownloadStatus.downloading),
      ]..sort(compareDownloadsForList);

      expect(list.map((d) => d.id), ['failed', 'live', 'paused']);
    });
  });
}
