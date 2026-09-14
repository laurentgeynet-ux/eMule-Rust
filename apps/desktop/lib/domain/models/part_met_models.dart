import 'download_item.dart';

class PartMetInspectionResult {
  final String fileName;
  final int totalSize;
  final String ed2kHash;
  final String? aichHash;
  final int partCount;
  final String partMetPath;
  final String? partPath;
  final bool partFileExists;
  final int? partFileSize;
  final int estimatedTransferredBytes;

  const PartMetInspectionResult({
    required this.fileName,
    required this.totalSize,
    required this.ed2kHash,
    this.aichHash,
    required this.partCount,
    required this.partMetPath,
    this.partPath,
    required this.partFileExists,
    this.partFileSize,
    required this.estimatedTransferredBytes,
  });

  factory PartMetInspectionResult.fromJson(Map<String, dynamic> json) {
    return PartMetInspectionResult(
      fileName: json['fileName'] as String? ?? 'unknown',
      totalSize: (json['totalSize'] as num?)?.toInt() ?? 0,
      ed2kHash: json['ed2kHash'] as String? ?? '',
      aichHash: json['aichHash'] as String?,
      partCount: (json['partCount'] as num?)?.toInt() ?? 0,
      partMetPath: json['partMetPath'] as String? ?? '',
      partPath: json['partPath'] as String?,
      partFileExists: json['partFileExists'] as bool? ?? false,
      partFileSize: (json['partFileSize'] as num?)?.toInt(),
      estimatedTransferredBytes:
          (json['estimatedTransferredBytes'] as num?)?.toInt() ?? 0,
    );
  }
}

class ImportPartMetResult {
  final DownloadItem download;
  final int totalParts;
  final int verifiedParts;
  final int corruptedParts;
  final int transferredBytes;
  final int totalSize;

  final String? report;

  const ImportPartMetResult({
    required this.download,
    required this.totalParts,
    required this.verifiedParts,
    required this.corruptedParts,
    required this.transferredBytes,
    required this.totalSize,
    this.report,
  });

  factory ImportPartMetResult.fromJson(Map<String, dynamic> json) {
    return ImportPartMetResult(
      download: DownloadItem.fromJson(json['download'] as Map<String, dynamic>),
      totalParts: (json['totalParts'] as num?)?.toInt() ?? 0,
      verifiedParts: (json['verifiedParts'] as num?)?.toInt() ?? 0,
      corruptedParts: (json['corruptedParts'] as num?)?.toInt() ?? 0,
      transferredBytes: (json['transferredBytes'] as num?)?.toInt() ?? 0,
      totalSize: (json['totalSize'] as num?)?.toInt() ?? 0,
      report: json['report'] as String?,
    );
  }
}
