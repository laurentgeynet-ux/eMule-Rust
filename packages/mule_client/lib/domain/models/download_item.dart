import 'package:flutter/foundation.dart';

enum DownloadStatus {
  queued,
  waitingForSources,
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
      case DownloadStatus.waitingForSources:
        return 'Waiting for Sources';
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
      this == DownloadStatus.waitingForSources ||
      this == DownloadStatus.verifying ||
      this == DownloadStatus.completing;

  bool get isPaused => this == DownloadStatus.paused;
  bool get isCompleted => this == DownloadStatus.completed;
  bool get isFailed => this == DownloadStatus.failed;

  static DownloadStatus fromString(String? status) {
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

  static DownloadPriority fromString(String? priority) {
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
}

@immutable
class DownloadPartInfo {
  final int partIndex;
  final int startOffset;
  final int endOffset;
  final int size;
  final int receivedBytes;
  final bool isVerified;
  final String state; // 'verified', 'completed', 'downloading', 'missing'
  final int availableSourceCount; // sources advertising this part (eMule blue/red)

  const DownloadPartInfo({
    required this.partIndex,
    required this.startOffset,
    required this.endOffset,
    required this.size,
    required this.receivedBytes,
    required this.isVerified,
    required this.state,
    this.availableSourceCount = 0,
  });

  factory DownloadPartInfo.fromJson(Map<String, dynamic> json) {
    return DownloadPartInfo(
      partIndex: (json['partIndex'] as num?)?.toInt() ?? 0,
      startOffset: (json['startOffset'] as num?)?.toInt() ?? 0,
      endOffset: (json['endOffset'] as num?)?.toInt() ?? 0,
      size: (json['size'] as num?)?.toInt() ?? 0,
      receivedBytes: (json['receivedBytes'] as num?)?.toInt() ?? 0,
      isVerified: json['isVerified'] as bool? ?? false,
      state: json['state'] as String? ?? 'missing',
      availableSourceCount: (json['availableSourceCount'] as num?)?.toInt() ?? 0,
    );
  }

  double get progress => size > 0 ? (receivedBytes / size).clamp(0.0, 1.0) : 0.0;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is DownloadPartInfo &&
          runtimeType == other.runtimeType &&
          partIndex == other.partIndex &&
          receivedBytes == other.receivedBytes &&
          isVerified == other.isVerified &&
          state == other.state;

  @override
  int get hashCode =>
      partIndex.hashCode ^ receivedBytes.hashCode ^ isVerified.hashCode ^ state.hashCode;
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
  final String? ed2kHash;
  final String? aichRoot;
  final String? ed2kLink;
  final String? tempPath;
  final String? targetPath;
  final int averageRateBps;
  final int? etaSeconds;
  final int connectionAttempts;
  final String? lastError;
  final DateTime? lastStatusChange;
  final List<DownloadPartInfo> parts;
  final String? categoryId;
  /// Per-file source cap (eMule FT_MAXSOURCES); null = global limit.
  final int? maxSources;

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
    this.ed2kHash,
    this.aichRoot,
    this.ed2kLink,
    this.tempPath,
    this.targetPath,
    this.averageRateBps = 0,
    this.etaSeconds,
    this.connectionAttempts = 0,
    this.lastError,
    this.lastStatusChange,
    this.parts = const [],
    this.categoryId,
    this.maxSources,
  });

  factory DownloadItem.fromJson(Map<String, dynamic> json) {
    final rawProgress = (json['progress'] as num?)?.toDouble() ?? 0.0;
    final createdAtMs = (json['createdAtUnixMs'] as num?)?.toInt() ?? 0;
    final updatedAtMs = (json['updatedAtUnixMs'] as num?)?.toInt() ?? 0;

    final partsRaw = json['parts'] as List<dynamic>?;
    final parts = partsRaw != null
        ? partsRaw.map((e) => DownloadPartInfo.fromJson(e as Map<String, dynamic>)).toList()
        : const <DownloadPartInfo>[];

    final lastStatusChangeMs = (json['lastStatusChangeUnixMs'] as num?)?.toInt();

    return DownloadItem(
      id: json['id'] as String? ?? '',
      fileName: json['fileName'] as String? ?? 'Unnamed',
      totalBytes: (json['totalBytes'] as num?)?.toInt() ?? 0,
      receivedBytes: (json['receivedBytes'] as num?)?.toInt() ?? 0,
      verifiedBytes: (json['verifiedBytes'] as num?)?.toInt() ?? 0,
      progress: rawProgress.clamp(0.0, 1.0),
      status: DownloadStatus.fromString(json['status'] as String?),
      priority: DownloadPriority.fromString(json['priority'] as String?),
      sourceCount: (json['sourceCount'] as num?)?.toInt() ?? 0,
      downloadRateBps: (json['downloadRateBps'] as num?)?.toInt() ?? 0,
      uploadedBytes: (json['uploadedBytes'] as num?)?.toInt() ?? 0,
      createdAt: createdAtMs > 0
          ? DateTime.fromMillisecondsSinceEpoch(createdAtMs)
          : DateTime.now(),
      updatedAt: updatedAtMs > 0
          ? DateTime.fromMillisecondsSinceEpoch(updatedAtMs)
          : DateTime.now(),
      ed2kHash: json['ed2kHash'] as String?,
      aichRoot: json['aichRoot'] as String?,
      ed2kLink: json['ed2kLink'] as String?,
      tempPath: json['tempPath'] as String?,
      targetPath: json['targetPath'] as String?,
      averageRateBps: (json['averageRateBps'] as num?)?.toInt() ?? 0,
      etaSeconds: (json['etaSeconds'] as num?)?.toInt(),
      connectionAttempts: (json['connectionAttempts'] as num?)?.toInt() ?? 0,
      lastError: json['lastError'] as String?,
      lastStatusChange: lastStatusChangeMs != null && lastStatusChangeMs > 0
          ? DateTime.fromMillisecondsSinceEpoch(lastStatusChangeMs)
          : null,
      parts: parts,
      categoryId: json['categoryId'] as String?,
      maxSources: (json['maxSources'] as num?)?.toInt(),
    );
  }

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
    String? ed2kHash,
    String? aichRoot,
    String? ed2kLink,
    String? tempPath,
    String? targetPath,
    int? averageRateBps,
    int? etaSeconds,
    int? connectionAttempts,
    String? lastError,
    DateTime? lastStatusChange,
    List<DownloadPartInfo>? parts,
    String? categoryId,
    bool clearCategoryId = false,
    int? maxSources,
    bool clearMaxSources = false,
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
      ed2kHash: ed2kHash ?? this.ed2kHash,
      aichRoot: aichRoot ?? this.aichRoot,
      ed2kLink: ed2kLink ?? this.ed2kLink,
      tempPath: tempPath ?? this.tempPath,
      targetPath: targetPath ?? this.targetPath,
      averageRateBps: averageRateBps ?? this.averageRateBps,
      etaSeconds: etaSeconds ?? this.etaSeconds,
      connectionAttempts: connectionAttempts ?? this.connectionAttempts,
      lastError: lastError ?? this.lastError,
      lastStatusChange: lastStatusChange ?? this.lastStatusChange,
      parts: parts ?? this.parts,
      categoryId: clearCategoryId ? null : (categoryId ?? this.categoryId),
      maxSources: clearMaxSources ? null : (maxSources ?? this.maxSources),
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
          targetPath == other.targetPath &&
          categoryId == other.categoryId &&
          updatedAt == other.updatedAt;

  @override
  int get hashCode =>
      id.hashCode ^
      receivedBytes.hashCode ^
      progress.hashCode ^
      status.hashCode ^
      (categoryId?.hashCode ?? 0) ^
      updatedAt.hashCode;
}
