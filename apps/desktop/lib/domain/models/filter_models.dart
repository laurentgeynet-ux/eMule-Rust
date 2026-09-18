class FilterReportModel {
  final int totalLines;
  final int commentOrEmptyLines;
  final int validEntriesParsed;
  final int invalidLines;
  final int coalescedCount;
  final int finalRangeCount;
  final List<String> warnings;

  const FilterReportModel({
    required this.totalLines,
    required this.commentOrEmptyLines,
    required this.validEntriesParsed,
    required this.invalidLines,
    required this.coalescedCount,
    required this.finalRangeCount,
    required this.warnings,
  });

  factory FilterReportModel.fromJson(Map<String, dynamic> json) {
    return FilterReportModel(
      totalLines: json['total_lines'] as int? ?? 0,
      commentOrEmptyLines: json['comment_or_empty_lines'] as int? ?? 0,
      validEntriesParsed: json['valid_entries_parsed'] as int? ?? 0,
      invalidLines: json['invalid_lines'] as int? ?? 0,
      coalescedCount: json['coalesced_count'] as int? ?? 0,
      finalRangeCount: json['final_range_count'] as int? ?? 0,
      warnings: (json['warnings'] as List<dynamic>?)
              ?.map((e) => e.toString())
              .toList() ??
          const [],
    );
  }
}

class FilterStatusModel {
  final bool enabled;
  final int rangeCount;
  final int hitsCount;
  final FilterReportModel? lastImportReport;

  const FilterStatusModel({
    required this.enabled,
    required this.rangeCount,
    required this.hitsCount,
    this.lastImportReport,
  });

  factory FilterStatusModel.fromJson(Map<String, dynamic> json) {
    final reportJson = json['last_import_report'] as Map<String, dynamic>?;
    return FilterStatusModel(
      enabled: json['enabled'] as bool? ?? true,
      rangeCount: json['range_count'] as int? ?? 0,
      hitsCount: json['hits_count'] as int? ?? 0,
      lastImportReport:
          reportJson != null ? FilterReportModel.fromJson(reportJson) : null,
    );
  }
}

class RateLimitsModel {
  final int downloadBytesPerSec;
  final int uploadBytesPerSec;
  final int connectionsPerMinute;
  final int serverRequestsPerSec;
  final int kadPacketsPerSec;

  const RateLimitsModel({
    required this.downloadBytesPerSec,
    required this.uploadBytesPerSec,
    required this.connectionsPerMinute,
    required this.serverRequestsPerSec,
    required this.kadPacketsPerSec,
  });

  factory RateLimitsModel.fromJson(Map<String, dynamic> json) {
    return RateLimitsModel(
      downloadBytesPerSec: json['downloadBytesPerSec'] as int? ?? 0,
      uploadBytesPerSec: json['uploadBytesPerSec'] as int? ?? 0,
      connectionsPerMinute: json['connectionsPerMinute'] as int? ?? 120,
      serverRequestsPerSec: json['serverRequestsPerSec'] as int? ?? 10,
      kadPacketsPerSec: json['kadPacketsPerSec'] as int? ?? 50,
    );
  }

  Map<String, dynamic> toJson() => {
        'downloadBytesPerSec': downloadBytesPerSec,
        'uploadBytesPerSec': uploadBytesPerSec,
        'connectionsPerMinute': connectionsPerMinute,
        'serverRequestsPerSec': serverRequestsPerSec,
        'kadPacketsPerSec': kadPacketsPerSec,
      };
}
