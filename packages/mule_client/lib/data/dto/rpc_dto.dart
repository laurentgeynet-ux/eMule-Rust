// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import '../../domain/models/category_item.dart';
import '../../domain/models/daemon_info.dart';
import '../../domain/models/download_item.dart';
import '../../domain/models/history_entry.dart';
import '../../domain/models/history_stats.dart';

class RpcDto {
  static DownloadStatus parseStatus(String? status) {
    switch (status?.toLowerCase()) {
      case 'waiting_for_sources':
        return DownloadStatus.waitingForSources;
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
      case 'auto':
        return DownloadPriority.auto;
      case 'normal':
      default:
        return DownloadPriority.normal;
    }
  }

  static DownloadPartInfo partInfoFromJson(Map<String, dynamic> json) {
    return DownloadPartInfo(
      partIndex: (json['partIndex'] as num?)?.toInt() ?? 0,
      startOffset: (json['startOffset'] as num?)?.toInt() ?? 0,
      endOffset: (json['endOffset'] as num?)?.toInt() ?? 0,
      size: (json['size'] as num?)?.toInt() ?? 0,
      receivedBytes: (json['receivedBytes'] as num?)?.toInt() ?? 0,
      isVerified: json['isVerified'] as bool? ?? false,
      state: json['state'] as String? ?? 'missing',
    );
  }

  static DownloadItem downloadFromSnapshotJson(Map<String, dynamic> json) {
    return DownloadItem.fromJson(json);
  }

  static CategoryItem categoryFromJson(Map<String, dynamic> json) {
    final createdAtMs = (json['createdAt'] as num?)?.toInt() ?? 0;
    final updatedAtMs = (json['updatedAt'] as num?)?.toInt() ?? 0;

    return CategoryItem(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? 'Unnamed',
      color: json['color'] as String? ?? '#2196F3',
      sortOrder: (json['sortOrder'] as num?)?.toInt() ?? 0,
      defaultDestination: json['defaultDestination'] as String?,
      defaultPriority: json['defaultPriority'] as String?,
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
      workdirMode: json['workdirMode'] as String?,
      workdirPath: json['workdirPath'] as String?,
      defaultDownloadDir: json['defaultDownloadDir'] as String?,
    );
  }

  static HistoryEntry historyEntryFromJson(Map<String, dynamic> json) {
    final timestampMs = (json['timestampUnixMs'] as num?)?.toInt() ?? 0;
    return HistoryEntry(
      id: (json['id'] as num?)?.toInt() ?? 0,
      downloadId: json['downloadId'] as String? ?? '',
      fileName: json['fileName'] as String? ?? 'Unnamed',
      totalBytes: (json['totalSize'] as num?)?.toInt() ?? 0,
      transferredBytes: (json['transferredBytes'] as num?)?.toInt() ?? 0,
      ed2kHash: json['ed2kHash'] as String? ?? '',
      eventType: json['eventType'] as String? ?? '',
      status: json['status'] as String? ?? '',
      durationSecs: (json['durationSecs'] as num?)?.toInt() ?? 0,
      connectionAttempts: (json['connectionAttempts'] as num?)?.toInt() ?? 0,
      resumeCount: (json['resumeCount'] as num?)?.toInt() ?? 0,
      verificationResult: json['verificationResult'] as String? ?? 'unverified',
      errorMessage: json['errorMessage'] as String?,
      timestamp: timestampMs > 0
          ? DateTime.fromMillisecondsSinceEpoch(timestampMs)
          : DateTime.now(),
    );
  }

  static HistoryStats historyStatsFromJson(Map<String, dynamic> json) {
    return HistoryStats(
      totalCompletedCount: (json['totalCompletedCount'] as num?)?.toInt() ?? 0,
      totalCompletedBytes: (json['totalCompletedBytes'] as num?)?.toInt() ?? 0,
      totalFailedCount: (json['totalFailedCount'] as num?)?.toInt() ?? 0,
      totalResumesCount: (json['totalResumesCount'] as num?)?.toInt() ?? 0,
      totalIntegrityFailures: (json['totalIntegrityFailures'] as num?)?.toInt() ?? 0,
      sessionCompletedBytes: (json['sessionCompletedBytes'] as num?)?.toInt() ?? 0,
      sessionCompletedCount: (json['sessionCompletedCount'] as num?)?.toInt() ?? 0,
      last24hCompletedBytes: (json['last24hCompletedBytes'] as num?)?.toInt() ?? 0,
      last24hCompletedCount: (json['last24hCompletedCount'] as num?)?.toInt() ?? 0,
      last7dCompletedBytes: (json['last7dCompletedBytes'] as num?)?.toInt() ?? 0,
      last7dCompletedCount: (json['last7dCompletedCount'] as num?)?.toInt() ?? 0,
    );
  }
}
