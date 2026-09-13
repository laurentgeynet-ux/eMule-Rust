import 'package:flutter/foundation.dart';

enum DownloadStatus {
  queued,
  allocating,
  downloading,
  paused,
  verifying,
  completing,
  completed,
  failed;

  String get label {
    switch (this) {
      case DownloadStatus.queued:
        return 'Queued';
      case DownloadStatus.allocating:
        return 'Allocating';
      case DownloadStatus.downloading:
        return 'Downloading';
      case DownloadStatus.paused:
        return 'Paused';
      case DownloadStatus.verifying:
        return 'Verifying';
      case DownloadStatus.completing:
        return 'Completing';
      case DownloadStatus.completed:
        return 'Completed';
      case DownloadStatus.failed:
        return 'Failed';
    }
  }

  bool get isActive =>
      this == DownloadStatus.downloading ||
      this == DownloadStatus.allocating ||
      this == DownloadStatus.verifying ||
      this == DownloadStatus.completing;

  bool get isPaused => this == DownloadStatus.paused;
  bool get isCompleted => this == DownloadStatus.completed;
  bool get isFailed => this == DownloadStatus.failed;
}

enum DownloadPriority {
  low,
  normal,
  high;

  String get label {
    switch (this) {
      case DownloadPriority.low:
        return 'Low';
      case DownloadPriority.normal:
        return 'Normal';
      case DownloadPriority.high:
        return 'High';
    }
  }
}

@immutable
class DownloadItem {
  final String id;
  final String fileName;
  final int totalBytes;
  final int receivedBytes;
  final int verifiedBytes;
  final double progress; // strictly 0.0 .. 1.0
  final DownloadStatus status;
  final DownloadPriority priority;
  final int sourceCount;
  final int downloadRateBps;
  final int uploadedBytes;
  final DateTime createdAt;
  final DateTime updatedAt;

  const DownloadItem({
    required this.id,
    required this.fileName,
    required this.totalBytes,
    required this.receivedBytes,
    required this.verifiedBytes,
    required this.progress,
    required this.status,
    required this.priority,
    required this.sourceCount,
    required this.downloadRateBps,
    required this.uploadedBytes,
    required this.createdAt,
    required this.updatedAt,
  });

  int get remainingBytes => (totalBytes > receivedBytes) ? totalBytes - receivedBytes : 0;

  DownloadItem copyWith({
    String? id,
    String? fileName,
    int? totalBytes,
    int? receivedBytes,
    int? verifiedBytes,
    double? progress,
    DownloadStatus? status,
    DownloadPriority? priority,
    int? sourceCount,
    int? downloadRateBps,
    int? uploadedBytes,
    DateTime? createdAt,
    DateTime? updatedAt,
  }) {
    return DownloadItem(
      id: id ?? this.id,
      fileName: fileName ?? this.fileName,
      totalBytes: totalBytes ?? this.totalBytes,
      receivedBytes: receivedBytes ?? this.receivedBytes,
      verifiedBytes: verifiedBytes ?? this.verifiedBytes,
      progress: (progress ?? this.progress).clamp(0.0, 1.0),
      status: status ?? this.status,
      priority: priority ?? this.priority,
      sourceCount: sourceCount ?? this.sourceCount,
      downloadRateBps: downloadRateBps ?? this.downloadRateBps,
      uploadedBytes: uploadedBytes ?? this.uploadedBytes,
      createdAt: createdAt ?? this.createdAt,
      updatedAt: updatedAt ?? this.updatedAt,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is DownloadItem &&
          runtimeType == other.runtimeType &&
          id == other.id &&
          receivedBytes == other.receivedBytes &&
          verifiedBytes == other.verifiedBytes &&
          progress == other.progress &&
          status == other.status &&
          priority == other.priority &&
          sourceCount == other.sourceCount &&
          downloadRateBps == other.downloadRateBps &&
          updatedAt == other.updatedAt;

  @override
  int get hashCode =>
      id.hashCode ^
      receivedBytes.hashCode ^
      progress.hashCode ^
      status.hashCode ^
      updatedAt.hashCode;
}
