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

enum ServerConnectionStatus {
  disconnected,
  connecting,
  connected,
  disconnecting;

  static ServerConnectionStatus fromString(String? val) {
    switch (val) {
      case 'connecting':
        return ServerConnectionStatus.connecting;
      case 'connected':
        return ServerConnectionStatus.connected;
      case 'disconnecting':
        return ServerConnectionStatus.disconnecting;
      case 'disconnected':
      default:
        return ServerConnectionStatus.disconnected;
    }
  }

  String toDisplayString() {
    switch (this) {
      case ServerConnectionStatus.connecting:
        return 'Connexion en cours...';
      case ServerConnectionStatus.connected:
        return 'Connecté';
      case ServerConnectionStatus.disconnecting:
        return 'Déconnexion...';
      case ServerConnectionStatus.disconnected:
        return 'Déconnecté';
    }
  }
}

class ServerSessionModel {
  final ServerConnectionStatus status;
  final String? serverAddress;
  final String? serverName;
  final String? serverDesc;
  final int? clientId;
  final bool isHighId;
  final String? welcomeMessage;
  final int? currentUsers;
  final int? currentFiles;
  final int? connectedAtUnixMs;
  final int? pingMs;

  const ServerSessionModel({
    required this.status,
    this.serverAddress,
    this.serverName,
    this.serverDesc,
    this.clientId,
    this.isHighId = false,
    this.welcomeMessage,
    this.currentUsers,
    this.currentFiles,
    this.connectedAtUnixMs,
    this.pingMs,
  });

  factory ServerSessionModel.fromJson(Map<String, dynamic> json) {
    return ServerSessionModel(
      status: ServerConnectionStatus.fromString(json['status'] as String?),
      serverAddress: json['serverAddress'] as String?,
      serverName: json['serverName'] as String?,
      serverDesc: json['serverDesc'] as String?,
      clientId: json['clientId'] as int?,
      isHighId: json['isHighId'] as bool? ?? false,
      welcomeMessage: json['welcomeMessage'] as String?,
      currentUsers: json['currentUsers'] as int?,
      currentFiles: json['currentFiles'] as int?,
      connectedAtUnixMs: json['connectedAtUnixMs'] as int?,
      pingMs: json['pingMs'] as int?,
    );
  }

  static const disconnected = ServerSessionModel(
    status: ServerConnectionStatus.disconnected,
  );
}

class SearchResultFileModel {
  final String fileHash;
  final String fileName;
  final int fileSize;
  final int sourceCount;

  const SearchResultFileModel({
    required this.fileHash,
    required this.fileName,
    required this.fileSize,
    this.sourceCount = 1,
  });

  factory SearchResultFileModel.fromJson(Map<String, dynamic> json) {
    return SearchResultFileModel(
      fileHash: json['fileHash'] as String? ?? '',
      fileName: json['fileName'] as String? ?? 'Unknown',
      fileSize: (json['fileSize'] as num?)?.toInt() ?? 0,
      sourceCount: (json['sourceCount'] as num?)?.toInt() ?? 1,
    );
  }

  String get ed2kLink => 'ed2k://|file|$fileName|$fileSize|$fileHash|/';
}

class FoundSourcePeerModel {
  final String address;
  final int ip;
  final int port;

  const FoundSourcePeerModel({
    required this.address,
    required this.ip,
    required this.port,
  });

  factory FoundSourcePeerModel.fromJson(Map<String, dynamic> json) {
    return FoundSourcePeerModel(
      address: json['address'] as String? ?? '',
      ip: (json['ip'] as num?)?.toInt() ?? 0,
      port: (json['port'] as num?)?.toInt() ?? 0,
    );
  }
}

