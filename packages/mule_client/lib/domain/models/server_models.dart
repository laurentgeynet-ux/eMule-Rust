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
      filterStatus: ServerFilterStatus.fromString(
        json['filterStatus'] as String?,
      ),
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

enum PeerSourceStatus {
  candidate,
  allowed,
  filtered,
  rejected,
  tried,
  connected,
  failed;

  static PeerSourceStatus fromString(String? val) {
    switch (val?.toLowerCase()) {
      case 'allowed':
        return PeerSourceStatus.allowed;
      case 'filtered':
        return PeerSourceStatus.filtered;
      case 'rejected':
        return PeerSourceStatus.rejected;
      case 'tried':
        return PeerSourceStatus.tried;
      case 'connected':
        return PeerSourceStatus.connected;
      case 'failed':
        return PeerSourceStatus.failed;
      case 'candidate':
      default:
        return PeerSourceStatus.candidate;
    }
  }

  String toApiString() {
    switch (this) {
      case PeerSourceStatus.candidate:
        return 'candidate';
      case PeerSourceStatus.allowed:
        return 'allowed';
      case PeerSourceStatus.filtered:
        return 'filtered';
      case PeerSourceStatus.rejected:
        return 'rejected';
      case PeerSourceStatus.tried:
        return 'tried';
      case PeerSourceStatus.connected:
        return 'connected';
      case PeerSourceStatus.failed:
        return 'failed';
    }
  }
}

enum PeerSourceOrigin {
  server,
  search,
  manual,
  kad;

  static PeerSourceOrigin fromString(String? val) {
    switch (val?.toLowerCase()) {
      case 'search':
        return PeerSourceOrigin.search;
      case 'manual':
        return PeerSourceOrigin.manual;
      case 'kad':
        return PeerSourceOrigin.kad;
      case 'server':
      default:
        return PeerSourceOrigin.server;
    }
  }

  String toApiString() {
    switch (this) {
      case PeerSourceOrigin.server:
        return 'server';
      case PeerSourceOrigin.search:
        return 'search';
      case PeerSourceOrigin.manual:
        return 'manual';
      case PeerSourceOrigin.kad:
        return 'kad';
    }
  }
}

class PeerSourceModel {
  final String fileHash;
  final String address;
  final int ip;
  final int port;
  final PeerSourceOrigin origin;
  final int discoveredAtUnixMs;
  final int lastSeenUnixMs;
  final PeerSourceStatus status;
  final int failCount;
  final String? lastError;
  final int? cooldownUntilUnixMs;
  final String? software;
  final List<int>? availability;

  const PeerSourceModel({
    required this.fileHash,
    required this.address,
    required this.ip,
    required this.port,
    required this.origin,
    required this.discoveredAtUnixMs,
    required this.lastSeenUnixMs,
    required this.status,
    this.failCount = 0,
    this.lastError,
    this.cooldownUntilUnixMs,
    this.software,
    this.availability,
  });

  factory PeerSourceModel.fromJson(Map<String, dynamic> json) {
    return PeerSourceModel(
      fileHash:
          json['fileHash'] as String? ?? json['file_hash'] as String? ?? '',
      address: json['address'] as String? ?? '',
      ip: (json['ip'] as num?)?.toInt() ?? 0,
      port: (json['port'] as num?)?.toInt() ?? 0,
      origin: PeerSourceOrigin.fromString(json['origin'] as String?),
      discoveredAtUnixMs:
          (json['discoveredAtUnixMs'] as num?)?.toInt() ??
          (json['discovered_at_unix_ms'] as num?)?.toInt() ??
          0,
      lastSeenUnixMs:
          (json['lastSeenUnixMs'] as num?)?.toInt() ??
          (json['last_seen_unix_ms'] as num?)?.toInt() ??
          0,
      status: PeerSourceStatus.fromString(json['status'] as String?),
      failCount:
          (json['failCount'] as num?)?.toInt() ??
          (json['fail_count'] as num?)?.toInt() ??
          0,
      lastError: json['lastError'] as String? ?? json['last_error'] as String?,
      cooldownUntilUnixMs:
          (json['cooldownUntilUnixMs'] as num?)?.toInt() ??
          (json['cooldown_until_unix_ms'] as num?)?.toInt(),
      software: json['software'] as String?,
      availability: (json['availability'] as List<dynamic>?)
          ?.map((e) => (e as num).toInt())
          .toList(),
    );
  }
}

enum PeerConnectionStatus {
  disconnected,
  connecting,
  handshaking,
  queued,
  transferring,
  disconnecting;

  static PeerConnectionStatus fromString(String? val) {
    switch (val?.toLowerCase()) {
      case 'connecting':
        return PeerConnectionStatus.connecting;
      case 'handshaking':
        return PeerConnectionStatus.handshaking;
      case 'queued':
        return PeerConnectionStatus.queued;
      case 'transferring':
        return PeerConnectionStatus.transferring;
      case 'disconnecting':
        return PeerConnectionStatus.disconnecting;
      case 'disconnected':
      default:
        return PeerConnectionStatus.disconnected;
    }
  }

  String toDisplayString() {
    switch (this) {
      case PeerConnectionStatus.disconnected:
        return 'Déconnecté';
      case PeerConnectionStatus.connecting:
        return 'Connexion...';
      case PeerConnectionStatus.handshaking:
        return 'Handshake eD2K...';
      case PeerConnectionStatus.queued:
        return 'En file d\'attente';
      case PeerConnectionStatus.transferring:
        return 'Téléchargement';
      case PeerConnectionStatus.disconnecting:
        return 'Déconnexion...';
    }
  }
}

class PeerSessionModel {
  final PeerConnectionStatus status;
  final String? fileHash;
  final String? peerAddress;
  final int? currentPart;
  final int transferRateBps;
  final int transferredBytes;
  final int? queueRank;
  final String? lastError;
  final String? failureReason;
  final int? connectedAtUnixMs;

  const PeerSessionModel({
    required this.status,
    this.fileHash,
    this.peerAddress,
    this.currentPart,
    this.transferRateBps = 0,
    this.transferredBytes = 0,
    this.queueRank,
    this.lastError,
    this.failureReason,
    this.connectedAtUnixMs,
  });

  factory PeerSessionModel.fromJson(Map<String, dynamic> json) {
    return PeerSessionModel(
      status: PeerConnectionStatus.fromString(json['status'] as String?),
      fileHash: json['fileHash'] as String? ?? json['file_hash'] as String?,
      peerAddress:
          json['peerAddress'] as String? ?? json['peer_address'] as String?,
      currentPart:
          (json['currentPart'] as num?)?.toInt() ??
          (json['current_part'] as num?)?.toInt(),
      transferRateBps:
          (json['transferRateBps'] as num?)?.toInt() ??
          (json['transfer_rate_bps'] as num?)?.toInt() ??
          0,
      transferredBytes:
          (json['transferredBytes'] as num?)?.toInt() ??
          (json['transferred_bytes'] as num?)?.toInt() ??
          0,
      queueRank:
          (json['queueRank'] as num?)?.toInt() ??
          (json['queue_rank'] as num?)?.toInt(),
      lastError: json['lastError'] as String? ?? json['last_error'] as String?,
      failureReason:
          json['failureReason'] as String? ?? json['failure_reason'] as String?,
      connectedAtUnixMs:
          (json['connectedAtUnixMs'] as num?)?.toInt() ??
          (json['connected_at_unix_ms'] as num?)?.toInt(),
    );
  }

  static const disconnected = PeerSessionModel(
    status: PeerConnectionStatus.disconnected,
  );
}

class InFlightBlockModel {
  final String peerId;
  final int partIndex;
  final int start;
  final int end;
  final int issuedAtUnixMs;
  final int deadlineUnixMs;

  const InFlightBlockModel({
    required this.peerId,
    required this.partIndex,
    required this.start,
    required this.end,
    required this.issuedAtUnixMs,
    required this.deadlineUnixMs,
  });

  factory InFlightBlockModel.fromJson(Map<String, dynamic> json) {
    return InFlightBlockModel(
      peerId: json['peerId'] as String? ?? json['peer_id'] as String? ?? '',
      partIndex:
          (json['partIndex'] as num?)?.toInt() ??
          (json['part_index'] as num?)?.toInt() ??
          0,
      start: (json['start'] as num?)?.toInt() ?? 0,
      end: (json['end'] as num?)?.toInt() ?? 0,
      issuedAtUnixMs:
          (json['issuedAtUnixMs'] as num?)?.toInt() ??
          (json['issued_at_unix_ms'] as num?)?.toInt() ??
          0,
      deadlineUnixMs:
          (json['deadlineUnixMs'] as num?)?.toInt() ??
          (json['deadline_unix_ms'] as num?)?.toInt() ??
          0,
    );
  }
}

class SwarmStatusModel {
  final String? fileHash;
  final int activePeerCount;
  final List<PeerSessionModel> peers;
  final List<InFlightBlockModel> inFlightBlocks;

  const SwarmStatusModel({
    this.fileHash,
    this.activePeerCount = 0,
    this.peers = const [],
    this.inFlightBlocks = const [],
  });

  factory SwarmStatusModel.fromJson(Map<String, dynamic> json) {
    final rawPeers = json['peers'] as List<dynamic>? ?? [];
    final peersList = rawPeers
        .map((p) => PeerSessionModel.fromJson(p as Map<String, dynamic>))
        .toList();

    final rawBlocks =
        json['inFlightBlocks'] as List<dynamic>? ??
        json['in_flight_blocks'] as List<dynamic>? ??
        [];
    final blocksList = rawBlocks
        .map((b) => InFlightBlockModel.fromJson(b as Map<String, dynamic>))
        .toList();

    return SwarmStatusModel(
      fileHash: json['fileHash'] as String? ?? json['file_hash'] as String?,
      activePeerCount:
          (json['activePeerCount'] as num?)?.toInt() ??
          (json['active_peer_count'] as num?)?.toInt() ??
          peersList.length,
      peers: peersList,
      inFlightBlocks: blocksList,
    );
  }

  static const empty = SwarmStatusModel();
}

class SharedFileModel {
  final String fileHash;
  final String fileName;
  final int fileSize;
  final String filePath;
  final int sharedAtUnixMs;
  final bool enabled;
  /// eMule upload priority: verylow|low|normal|high|veryhigh|powershare.
  final String upPriority;

  const SharedFileModel({
    required this.fileHash,
    required this.fileName,
    required this.fileSize,
    required this.filePath,
    required this.sharedAtUnixMs,
    this.enabled = true,
    this.upPriority = 'normal',
  });

  factory SharedFileModel.fromJson(Map<String, dynamic> json) {
    return SharedFileModel(
      fileHash:
          json['fileHash'] as String? ?? json['file_hash'] as String? ?? '',
      fileName:
          json['fileName'] as String? ?? json['file_name'] as String? ?? '',
      fileSize:
          (json['fileSize'] as num?)?.toInt() ??
          (json['file_size'] as num?)?.toInt() ??
          0,
      filePath:
          json['filePath'] as String? ?? json['file_path'] as String? ?? '',
      sharedAtUnixMs:
          (json['sharedAtUnixMs'] as num?)?.toInt() ??
          (json['shared_at_unix_ms'] as num?)?.toInt() ??
          0,
      enabled: json['enabled'] as bool? ?? true,
      upPriority: json['upPriority'] as String? ??
          json['up_priority'] as String? ??
          'normal',
    );
  }
}

/// Friend entry (eMule `friends.met`) — a userhash granted a dedicated
/// upload slot bypassing the queue.
class FriendModel {
  final String userHash;
  final String name;
  final int addedAtUnixMs;

  const FriendModel({
    required this.userHash,
    this.name = '',
    this.addedAtUnixMs = 0,
  });

  factory FriendModel.fromJson(Map<String, dynamic> json) {
    return FriendModel(
      userHash: json['userHash'] as String? ?? '',
      name: json['name'] as String? ?? '',
      addedAtUnixMs: (json['addedAtUnixMs'] as num?)?.toInt() ?? 0,
    );
  }
}

class UploadSessionModel {
  final String clientAddress;
  final String fileHash;
  final String fileName;
  final int transferredBytes;
  final int uploadRateBps;
  final String state;
  final int? queueRank;
  final int startedAtUnixMs;
  /// Queue entries only: eMule credit multiplier × 100 (100 = 1.0x).
  final int creditMultiplierBps;
  /// Queue entries only: eMule file-priority weight (veryhigh=18 … verylow=2).
  final int filePrioNum;
  /// Queue entries only: bad-guy/banned client — score always 0.
  final bool banned;
  /// Queue entries only: when the client joined the upload queue.
  final int enqueuedAtUnixMs;

  const UploadSessionModel({
    required this.clientAddress,
    required this.fileHash,
    required this.fileName,
    required this.transferredBytes,
    required this.uploadRateBps,
    required this.state,
    this.queueRank,
    required this.startedAtUnixMs,
    this.creditMultiplierBps = 100,
    this.filePrioNum = 7,
    this.banned = false,
    this.enqueuedAtUnixMs = 0,
  });

  /// eMule queue score: `wait_secs * credit_bps * file_prio / 1000`
  /// (0 for banned clients).
  int effectiveScore(int nowMs) {
    if (banned) return 0;
    final waitSecs = ((nowMs - enqueuedAtUnixMs) ~/ 1000).clamp(1, 1 << 62);
    final mult = creditMultiplierBps < 100 ? 100 : creditMultiplierBps;
    return waitSecs * mult * filePrioNum ~/ 1000;
  }

  factory UploadSessionModel.fromJson(Map<String, dynamic> json) {
    return UploadSessionModel(
      clientAddress:
          json['clientAddress'] as String? ??
          json['client_address'] as String? ??
          '',
      fileHash:
          json['fileHash'] as String? ?? json['file_hash'] as String? ?? '',
      fileName:
          json['fileName'] as String? ?? json['file_name'] as String? ?? '',
      transferredBytes:
          (json['transferredBytes'] as num?)?.toInt() ??
          (json['transferred_bytes'] as num?)?.toInt() ??
          0,
      uploadRateBps:
          (json['uploadRateBps'] as num?)?.toInt() ??
          (json['upload_rate_bps'] as num?)?.toInt() ??
          0,
      state: json['state'] as String? ?? 'idle',
      queueRank:
          (json['queueRank'] as num?)?.toInt() ??
          (json['queue_rank'] as num?)?.toInt(),
      startedAtUnixMs:
          (json['startedAtUnixMs'] as num?)?.toInt() ??
          (json['started_at_unix_ms'] as num?)?.toInt() ??
          0,
      creditMultiplierBps:
          (json['creditMultiplierBps'] as num?)?.toInt() ??
          (json['credit_multiplier_bps'] as num?)?.toInt() ??
          100,
      filePrioNum:
          (json['filePrioNum'] as num?)?.toInt() ??
          (json['file_prio_num'] as num?)?.toInt() ??
          7,
      banned: json['banned'] as bool? ?? false,
      enqueuedAtUnixMs:
          (json['enqueuedAtUnixMs'] as num?)?.toInt() ??
          (json['enqueued_at_unix_ms'] as num?)?.toInt() ??
          0,
    );
  }
}

class InboundListenerStatusModel {
  final bool isListening;
  final String? listenAddress;
  final int activeUploads;
  final int queuedUploads;
  final int uploadRateBps;

  const InboundListenerStatusModel({
    required this.isListening,
    this.listenAddress,
    this.activeUploads = 0,
    this.queuedUploads = 0,
    this.uploadRateBps = 0,
  });

  factory InboundListenerStatusModel.fromJson(Map<String, dynamic> json) {
    return InboundListenerStatusModel(
      isListening:
          json['isListening'] as bool? ??
          json['is_listening'] as bool? ??
          false,
      listenAddress:
          json['listenAddress'] as String? ?? json['listen_address'] as String?,
      activeUploads:
          (json['activeUploads'] as num?)?.toInt() ??
          (json['active_uploads'] as num?)?.toInt() ??
          0,
      queuedUploads:
          (json['queuedUploads'] as num?)?.toInt() ??
          (json['queued_uploads'] as num?)?.toInt() ??
          0,
      uploadRateBps:
          (json['uploadRateBps'] as num?)?.toInt() ??
          (json['upload_rate_bps'] as num?)?.toInt() ??
          0,
    );
  }

  static const idle = InboundListenerStatusModel(isListening: false);
}

class SecureIdentityStatusModel {
  final bool enabled;
  final bool hasKey;
  final String? pubkeyHash;
  final int? keyBits;

  const SecureIdentityStatusModel({
    required this.enabled,
    required this.hasKey,
    this.pubkeyHash,
    this.keyBits,
  });

  factory SecureIdentityStatusModel.fromJson(Map<String, dynamic> json) {
    return SecureIdentityStatusModel(
      enabled: json['enabled'] as bool? ?? false,
      hasKey: json['hasKey'] as bool? ?? json['has_key'] as bool? ?? false,
      pubkeyHash:
          json['pubkeyHash'] as String? ?? json['pubkey_hash'] as String?,
      keyBits:
          (json['keyBits'] as num?)?.toInt() ??
          (json['key_bits'] as num?)?.toInt(),
    );
  }

  static const unauthenticated = SecureIdentityStatusModel(
    enabled: false,
    hasKey: false,
  );
}

class PeerCreditModel {
  final String pubkeyHash;
  final String pubkeyHex;
  final int uploadedBytes;
  final int downloadedBytes;
  final double creditMultiplier;
  final int firstSeenUnixMs;
  final int lastSeenUnixMs;

  const PeerCreditModel({
    required this.pubkeyHash,
    required this.pubkeyHex,
    required this.uploadedBytes,
    required this.downloadedBytes,
    required this.creditMultiplier,
    required this.firstSeenUnixMs,
    required this.lastSeenUnixMs,
  });

  factory PeerCreditModel.fromJson(Map<String, dynamic> json) {
    return PeerCreditModel(
      pubkeyHash:
          json['pubkeyHash'] as String? ?? json['pubkey_hash'] as String? ?? '',
      pubkeyHex:
          json['pubkeyHex'] as String? ?? json['pubkey_hex'] as String? ?? '',
      uploadedBytes:
          (json['uploadedBytes'] as num?)?.toInt() ??
          (json['uploaded_bytes'] as num?)?.toInt() ??
          0,
      downloadedBytes:
          (json['downloadedBytes'] as num?)?.toInt() ??
          (json['downloaded_bytes'] as num?)?.toInt() ??
          0,
      creditMultiplier:
          (json['creditMultiplier'] as num?)?.toDouble() ??
          (json['credit_multiplier'] as num?)?.toDouble() ??
          1.0,
      firstSeenUnixMs:
          (json['firstSeenUnixMs'] as num?)?.toInt() ??
          (json['first_seen_unix_ms'] as num?)?.toInt() ??
          0,
      lastSeenUnixMs:
          (json['lastSeenUnixMs'] as num?)?.toInt() ??
          (json['last_seen_unix_ms'] as num?)?.toInt() ??
          0,
    );
  }
}

class PeerCreditStatsModel {
  final int totalPeers;
  final int totalUploadedBytes;
  final int totalDownloadedBytes;
  final int activeCreditedPeers;

  const PeerCreditStatsModel({
    required this.totalPeers,
    required this.totalUploadedBytes,
    required this.totalDownloadedBytes,
    required this.activeCreditedPeers,
  });

  factory PeerCreditStatsModel.fromJson(Map<String, dynamic> json) {
    return PeerCreditStatsModel(
      totalPeers:
          (json['totalPeers'] as num?)?.toInt() ??
          (json['total_peers'] as num?)?.toInt() ??
          0,
      totalUploadedBytes:
          (json['totalUploadedBytes'] as num?)?.toInt() ??
          (json['total_uploaded_bytes'] as num?)?.toInt() ??
          0,
      totalDownloadedBytes:
          (json['totalDownloadedBytes'] as num?)?.toInt() ??
          (json['total_downloaded_bytes'] as num?)?.toInt() ??
          0,
      activeCreditedPeers:
          (json['activeCreditedPeers'] as num?)?.toInt() ??
          (json['active_credited_peers'] as num?)?.toInt() ??
          0,
    );
  }

  static const zero = PeerCreditStatsModel(
    totalPeers: 0,
    totalUploadedBytes: 0,
    totalDownloadedBytes: 0,
    activeCreditedPeers: 0,
  );
}

class KadStatusModel {
  final String localId;
  final int totalContacts;
  final int totalReplacements;
  final int dbContactsCount;
  final int activeBuckets;
  final String networkProfile;
  final bool isUdpActive;
  final String? udpBindAddress;
  final int packetsSent;
  final int packetsReceived;
  final int activeTransactions;

  const KadStatusModel({
    required this.localId,
    required this.totalContacts,
    required this.totalReplacements,
    required this.dbContactsCount,
    required this.activeBuckets,
    required this.networkProfile,
    this.isUdpActive = false,
    this.udpBindAddress,
    this.packetsSent = 0,
    this.packetsReceived = 0,
    this.activeTransactions = 0,
  });

  factory KadStatusModel.fromJson(Map<String, dynamic> json) {
    return KadStatusModel(
      localId: json['localId'] as String? ?? json['local_id'] as String? ?? '',
      totalContacts:
          (json['totalContacts'] as num?)?.toInt() ??
          (json['total_contacts'] as num?)?.toInt() ??
          0,
      totalReplacements:
          (json['totalReplacements'] as num?)?.toInt() ??
          (json['total_replacements'] as num?)?.toInt() ??
          0,
      dbContactsCount:
          (json['dbContactsCount'] as num?)?.toInt() ??
          (json['db_contacts_count'] as num?)?.toInt() ??
          0,
      activeBuckets:
          (json['activeBuckets'] as num?)?.toInt() ??
          (json['active_buckets'] as num?)?.toInt() ??
          0,
      networkProfile:
          json['networkProfile'] as String? ??
          json['network_profile'] as String? ??
          'local-lab',
      isUdpActive:
          json['isUdpActive'] as bool? ??
          json['is_udp_active'] as bool? ??
          false,
      udpBindAddress:
          json['udpBindAddress'] as String? ??
          json['udp_bind_address'] as String?,
      packetsSent:
          (json['packetsSent'] as num?)?.toInt() ??
          (json['packets_sent'] as num?)?.toInt() ??
          0,
      packetsReceived:
          (json['packetsReceived'] as num?)?.toInt() ??
          (json['packets_received'] as num?)?.toInt() ??
          0,
      activeTransactions:
          (json['activeTransactions'] as num?)?.toInt() ??
          (json['active_transactions'] as num?)?.toInt() ??
          0,
    );
  }

  static const initial = KadStatusModel(
    localId: '',
    totalContacts: 0,
    totalReplacements: 0,
    dbContactsCount: 0,
    activeBuckets: 0,
    networkProfile: 'local-lab',
    isUdpActive: false,
    udpBindAddress: null,
    packetsSent: 0,
    packetsReceived: 0,
    activeTransactions: 0,
  );
}

class KadContactModel {
  final String kadId;
  final String ip;
  final int udpPort;
  final int? tcpPort;
  final int version;
  final bool verified;
  final int failCount;
  final int lastSeenUnixMs;

  const KadContactModel({
    required this.kadId,
    required this.ip,
    required this.udpPort,
    this.tcpPort,
    required this.version,
    required this.verified,
    required this.failCount,
    required this.lastSeenUnixMs,
  });

  factory KadContactModel.fromJson(Map<String, dynamic> json) {
    return KadContactModel(
      kadId: json['kadId'] as String? ?? json['kad_id'] as String? ?? '',
      ip: json['ip'] as String? ?? '',
      udpPort:
          (json['udpPort'] as num?)?.toInt() ??
          (json['udp_port'] as num?)?.toInt() ??
          0,
      tcpPort:
          (json['tcpPort'] as num?)?.toInt() ??
          (json['tcp_port'] as num?)?.toInt(),
      version: (json['version'] as num?)?.toInt() ?? 0,
      verified: json['verified'] as bool? ?? false,
      failCount:
          (json['failCount'] as num?)?.toInt() ??
          (json['fail_count'] as num?)?.toInt() ??
          0,
      lastSeenUnixMs:
          (json['lastSeenUnixMs'] as num?)?.toInt() ??
          (json['last_seen_unix_ms'] as num?)?.toInt() ??
          0,
    );
  }
}

class NodesDatInspectReportModel {
  final int version;
  final int declaredCount;
  final int validContactsCount;
  final int rejectedByPolicyCount;
  final int rejectedByFilterCount;
  final List<KadContactModel> sampleContacts;

  const NodesDatInspectReportModel({
    required this.version,
    required this.declaredCount,
    required this.validContactsCount,
    required this.rejectedByPolicyCount,
    required this.rejectedByFilterCount,
    required this.sampleContacts,
  });

  factory NodesDatInspectReportModel.fromJson(Map<String, dynamic> json) {
    final rawSamples =
        json['sampleContacts'] as List<dynamic>? ??
        json['sample_contacts'] as List<dynamic>? ??
        [];
    final samples = rawSamples
        .map((s) => KadContactModel.fromJson(s as Map<String, dynamic>))
        .toList();

    return NodesDatInspectReportModel(
      version: (json['version'] as num?)?.toInt() ?? 0,
      declaredCount:
          (json['declaredCount'] as num?)?.toInt() ??
          (json['declared_count'] as num?)?.toInt() ??
          0,
      validContactsCount:
          (json['validContactsCount'] as num?)?.toInt() ??
          (json['valid_contacts_count'] as num?)?.toInt() ??
          0,
      rejectedByPolicyCount:
          (json['rejectedByPolicyCount'] as num?)?.toInt() ??
          (json['rejected_by_policy_count'] as num?)?.toInt() ??
          0,
      rejectedByFilterCount:
          (json['rejectedByFilterCount'] as num?)?.toInt() ??
          (json['rejected_by_filter_count'] as num?)?.toInt() ??
          0,
      sampleContacts: samples,
    );
  }
}

class UpnpStatusModel {
  final bool enabled;
  final bool available;
  final String? gatewayAddr;
  final String? externalIp;
  final bool tcpMapped;
  final bool udpMapped;
  final int tcpPort;
  final int udpPort;
  final String? lastError;
  final int updatedAtUnixMs;

  const UpnpStatusModel({
    this.enabled = false,
    this.available = false,
    this.gatewayAddr,
    this.externalIp,
    this.tcpMapped = false,
    this.udpMapped = false,
    this.tcpPort = 39735,
    this.udpPort = 39737,
    this.lastError,
    this.updatedAtUnixMs = 0,
  });

  factory UpnpStatusModel.fromJson(Map<String, dynamic> json) {
    return UpnpStatusModel(
      enabled: json['enabled'] == true,
      available: json['available'] == true,
      gatewayAddr: json['gatewayAddr'] as String?,
      externalIp: json['externalIp'] as String?,
      tcpMapped: json['tcpMapped'] == true,
      udpMapped: json['udpMapped'] == true,
      tcpPort: (json['tcpPort'] as num?)?.toInt() ?? 39735,
      udpPort: (json['udpPort'] as num?)?.toInt() ?? 39737,
      lastError: json['lastError'] as String?,
      updatedAtUnixMs: (json['updatedAtUnixMs'] as num?)?.toInt() ?? 0,
    );
  }
}

/// Snapshot of the uploads state pushed by `upload.uploadsUpdated`:
/// active slots, waiting queue, and aggregate upload rate.
class UploadsSnapshotModel {
  final List<UploadSessionModel> active;
  final List<UploadSessionModel> queue;
  final int uploadRateBps;

  const UploadsSnapshotModel({
    this.active = const [],
    this.queue = const [],
    this.uploadRateBps = 0,
  });

  factory UploadsSnapshotModel.fromJson(Map<String, dynamic> json) {
    List<UploadSessionModel> parse(String key) =>
        ((json[key] as List?) ?? const [])
            .whereType<Map<String, dynamic>>()
            .map(UploadSessionModel.fromJson)
            .toList();
    return UploadsSnapshotModel(
      active: parse('active'),
      queue: parse('queue'),
      uploadRateBps: (json['uploadRateBps'] as num?)?.toInt() ?? 0,
    );
  }
}

/// Aggregate daemon metrics pushed by `event.statsTick` (~2 s cadence).
/// See `docs/architecture/rpc_protocol.md` §6.
class DaemonStatsModel {
  final int downloadBps;
  final int uploadBps;
  final int downloadsActive;
  final int peersConnected;
  final bool serverConnected;
  final int kadContacts;
  final bool kadUdpBound;

  const DaemonStatsModel({
    this.downloadBps = 0,
    this.uploadBps = 0,
    this.downloadsActive = 0,
    this.peersConnected = 0,
    this.serverConnected = false,
    this.kadContacts = 0,
    this.kadUdpBound = false,
  });

  factory DaemonStatsModel.fromJson(Map<String, dynamic> json) {
    int toInt(String k) => (json[k] as num?)?.toInt() ?? 0;
    return DaemonStatsModel(
      downloadBps: toInt('downloadBps'),
      uploadBps: toInt('uploadBps'),
      downloadsActive: toInt('downloadsActive'),
      peersConnected: toInt('peersConnected'),
      serverConnected: json['serverConnected'] == true,
      kadContacts: toInt('kadContacts'),
      kadUdpBound: json['kadUdpBound'] == true,
    );
  }
}
