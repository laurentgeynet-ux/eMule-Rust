enum ServerFilterStatus {
  allowed,
  blockedByFilter,
  disabled;

  static ServerFilterStatus fromString(String? val) {
    switch (val) {
      case 'blocked_by_filter':
        return ServerFilterStatus.blockedByFilter;
      case 'disabled':
        return ServerFilterStatus.disabled;
      case 'allowed':
      default:
        return ServerFilterStatus.allowed;
    }
  }

  String toApiString() {
    switch (this) {
      case ServerFilterStatus.blockedByFilter:
        return 'blocked_by_filter';
      case ServerFilterStatus.disabled:
        return 'disabled';
      case ServerFilterStatus.allowed:
        return 'allowed';
    }
  }
}

class Ed2kServerModel {
  final String address;
  final String ip;
  final int port;
  final String name;
  final String description;
  final int ping;
  final int preference;
  final int failedCount;
  final String? dynIp;
  final String? version;
  final ServerFilterStatus filterStatus;
  final int? lastSeenUnixMs;
  final int createdAtUnixMs;
  final int updatedAtUnixMs;

  const Ed2kServerModel({
    required this.address,
    required this.ip,
    required this.port,
    required this.name,
    this.description = '',
    this.ping = 0,
    this.preference = 0,
    this.failedCount = 0,
    this.dynIp,
    this.version,
    this.filterStatus = ServerFilterStatus.allowed,
    this.lastSeenUnixMs,
    this.createdAtUnixMs = 0,
    this.updatedAtUnixMs = 0,
  });

  factory Ed2kServerModel.fromJson(Map<String, dynamic> json) {
    return Ed2kServerModel(
      address: json['address'] as String? ?? '${json['ip']}:${json['port']}',
      ip: json['ip'] as String? ?? '',
      port: json['port'] as int? ?? 0,
      name: json['name'] as String? ?? '',
      description: json['description'] as String? ?? '',
      ping: json['ping'] as int? ?? 0,
      preference: json['preference'] as int? ?? 0,
      failedCount: json['failedCount'] as int? ?? 0,
      dynIp: json['dynIp'] as String?,
      version: json['version'] as String?,
      filterStatus: ServerFilterStatus.fromString(json['filterStatus'] as String?),
      lastSeenUnixMs: json['lastSeenUnixMs'] as int?,
      createdAtUnixMs: json['createdAtUnixMs'] as int? ?? 0,
      updatedAtUnixMs: json['updatedAtUnixMs'] as int? ?? 0,
    );
  }
}

class ServerInspectReport {
  final String filePath;
  final int totalCount;
  final int allowedCount;
  final int blockedCount;
  final List<Ed2kServerModel> servers;

  const ServerInspectReport({
    required this.filePath,
    required this.totalCount,
    required this.allowedCount,
    required this.blockedCount,
    required this.servers,
  });

  factory ServerInspectReport.fromJson(Map<String, dynamic> json) {
    final rawServers = json['servers'] as List<dynamic>? ?? const [];
    return ServerInspectReport(
      filePath: json['filePath'] as String? ?? '',
      totalCount: json['totalCount'] as int? ?? 0,
      allowedCount: json['allowedCount'] as int? ?? 0,
      blockedCount: json['blockedCount'] as int? ?? 0,
      servers: rawServers
          .map((e) => Ed2kServerModel.fromJson(e as Map<String, dynamic>))
          .toList(),
    );
  }
}

class ServerImportResult {
  final bool success;
  final int imported;
  final int totalCount;
  final int allowedCount;
  final int blockedCount;

  const ServerImportResult({
    required this.success,
    required this.imported,
    required this.totalCount,
    required this.allowedCount,
    required this.blockedCount,
  });

  factory ServerImportResult.fromJson(Map<String, dynamic> json) {
    return ServerImportResult(
      success: json['success'] as bool? ?? false,
      imported: json['imported'] as int? ?? 0,
      totalCount: json['totalCount'] as int? ?? 0,
      allowedCount: json['allowedCount'] as int? ?? 0,
      blockedCount: json['blockedCount'] as int? ?? 0,
    );
  }
}
