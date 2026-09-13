import '../../domain/models/daemon_info.dart';
import '../../domain/models/download_item.dart';

class RpcDto {
  static DownloadStatus parseStatus(String? status) {
    switch (status?.toLowerCase()) {
      case 'allocating':
        return DownloadStatus.allocating;
      case 'downloading':
        return DownloadStatus.downloading;
      case 'paused':
        return DownloadStatus.paused;
      case 'verifying':
        return DownloadStatus.verifying;
      case 'completing':
        return DownloadStatus.completing;
      case 'completed':
        return DownloadStatus.completed;
      case 'failed':
        return DownloadStatus.failed;
      case 'queued':
      default:
        return DownloadStatus.queued;
    }
  }

  static DownloadPriority parsePriority(String? priority) {
    switch (priority?.toLowerCase()) {
      case 'low':
        return DownloadPriority.low;
      case 'high':
        return DownloadPriority.high;
      case 'normal':
      default:
        return DownloadPriority.normal;
    }
  }

  static DownloadItem downloadFromSnapshotJson(Map<String, dynamic> json) {
    final rawProgress = (json['progress'] as num?)?.toDouble() ?? 0.0;
    final createdAtMs = (json['createdAtUnixMs'] as num?)?.toInt() ?? 0;
    final updatedAtMs = (json['updatedAtUnixMs'] as num?)?.toInt() ?? 0;

    return DownloadItem(
      id: json['id'] as String? ?? '',
      fileName: json['fileName'] as String? ?? 'Unnamed',
      totalBytes: (json['totalBytes'] as num?)?.toInt() ?? 0,
      receivedBytes: (json['receivedBytes'] as num?)?.toInt() ?? 0,
      verifiedBytes: (json['verifiedBytes'] as num?)?.toInt() ?? 0,
      progress: rawProgress.clamp(0.0, 1.0),
      status: parseStatus(json['status'] as String?),
      priority: parsePriority(json['priority'] as String?),
      sourceCount: (json['sourceCount'] as num?)?.toInt() ?? 0,
      downloadRateBps: (json['downloadRateBps'] as num?)?.toInt() ?? 0,
      uploadedBytes: (json['uploadedBytes'] as num?)?.toInt() ?? 0,
      createdAt: createdAtMs > 0
          ? DateTime.fromMillisecondsSinceEpoch(createdAtMs)
          : DateTime.now(),
      updatedAt: updatedAtMs > 0
          ? DateTime.fromMillisecondsSinceEpoch(updatedAtMs)
          : DateTime.now(),
    );
  }

  static DaemonInfo daemonInfoFromJson(Map<String, dynamic> json) {
    final caps = (json['capabilities'] as List<dynamic>?)
            ?.map((e) => e.toString())
            .toList() ??
        const [];

    return DaemonInfo(
      version: json['daemonVersion'] as String? ?? 'unknown',
      protocolVersion: (json['protocolVersion'] as num?)?.toInt() ?? 1,
      instanceId: json['instanceId'] as String? ?? '',
      sessionId: json['sessionId'] as String? ?? '',
      capabilities: caps,
    );
  }
}
