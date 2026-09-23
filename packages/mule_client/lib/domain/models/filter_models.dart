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
  /// Effective limit applied to the download bucket — already capped by
  /// the eMule upload/download ratio when the upload limit is < 20 KB/s.
  final int downloadBytesPerSec;
  final int uploadBytesPerSec;
  final int connectionsPerMinute;
  final int serverRequestsPerSec;
  final int kadPacketsPerSec;

  /// User-configured limits as persisted (pre-ratio) — read-only,
  /// never sent back via `limits.set`.
  final int downloadConfiguredBytesPerSec;
  final int uploadConfiguredBytesPerSec;

  /// true when `downloadBytesPerSec` is currently capped by the eMule
  /// upload/download ratio (« don't be a Lam3r :) »).
  final bool downloadRatioApplied;

  /// USS (Upload Speed Sense) state — when enabled, the upload bucket is
  /// driven by the dynamic control loop and `uploadBytesPerSec` reflects
  /// its current working point, not the configured limit.
  final bool ussEnabled;
  final int ussCurrentLimitBytesPerSec;

  const RateLimitsModel({
    required this.downloadBytesPerSec,
    required this.uploadBytesPerSec,
    required this.connectionsPerMinute,
    required this.serverRequestsPerSec,
    required this.kadPacketsPerSec,
    this.downloadConfiguredBytesPerSec = 0,
    this.uploadConfiguredBytesPerSec = 0,
    this.downloadRatioApplied = false,
    this.ussEnabled = false,
    this.ussCurrentLimitBytesPerSec = 0,
  });

  factory RateLimitsModel.fromJson(Map<String, dynamic> json) {
    final dl = json['downloadBytesPerSec'] as int? ?? 0;
    final ul = json['uploadBytesPerSec'] as int? ?? 0;
    final uss = json['uss'];
    return RateLimitsModel(
      downloadBytesPerSec: dl,
      uploadBytesPerSec: ul,
      connectionsPerMinute: json['connectionsPerMinute'] as int? ?? 120,
      serverRequestsPerSec: json['serverRequestsPerSec'] as int? ?? 10,
      kadPacketsPerSec: json['kadPacketsPerSec'] as int? ?? 50,
      downloadConfiguredBytesPerSec:
          json['downloadConfiguredBytesPerSec'] as int? ?? dl,
      uploadConfiguredBytesPerSec:
          json['uploadConfiguredBytesPerSec'] as int? ?? ul,
      downloadRatioApplied: json['downloadRatioApplied'] as bool? ?? false,
      ussEnabled: uss is Map<String, dynamic>
          ? uss['enabled'] as bool? ?? false
          : false,
      ussCurrentLimitBytesPerSec: uss is Map<String, dynamic>
          ? uss['currentLimitBytesPerSec'] as int? ?? 0
          : 0,
    );
  }

  /// Only the configurable fields are sent — the ratio fields are
  /// daemon-computed read-outs.
  Map<String, dynamic> toJson() => {
        'downloadBytesPerSec': downloadBytesPerSec,
        'uploadBytesPerSec': uploadBytesPerSec,
        'connectionsPerMinute': connectionsPerMinute,
        'serverRequestsPerSec': serverRequestsPerSec,
        'kadPacketsPerSec': kadPacketsPerSec,
      };
}
