enum NetworkProfile {
  localLab,
  publicEd2kPreview,
  publicEd2kKadPreview;

  static NetworkProfile fromString(String value) {
    switch (value.trim().toLowerCase()) {
      case 'public-ed2k-preview':
      case 'public_ed2k_preview':
        return NetworkProfile.publicEd2kPreview;
      case 'public-ed2k-kad-preview':
      case 'public_ed2k_kad_preview':
        return NetworkProfile.publicEd2kKadPreview;
      case 'local-lab':
      case 'local_lab':
      default:
        return NetworkProfile.localLab;
    }
  }

  String toKebabCase() {
    switch (this) {
      case NetworkProfile.localLab:
        return 'local-lab';
      case NetworkProfile.publicEd2kPreview:
        return 'public-ed2k-preview';
      case NetworkProfile.publicEd2kKadPreview:
        return 'public-ed2k-kad-preview';
    }
  }

  String get label {
    switch (this) {
      case NetworkProfile.localLab:
        return 'Local Lab (Loopback Only)';
      case NetworkProfile.publicEd2kPreview:
        return 'Public eD2K Preview';
      case NetworkProfile.publicEd2kKadPreview:
        return 'Public eD2K + Kad Preview';
    }
  }
}

enum NetworkSubsystemState {
  disabled,
  previewActive,
  connected,
  shuttingDown,
  error;

  static NetworkSubsystemState fromString(String value) {
    switch (value.trim().toLowerCase()) {
      case 'disabled':
        return NetworkSubsystemState.disabled;
      case 'connected':
        return NetworkSubsystemState.connected;
      case 'shutting-down':
      case 'shutting_down':
        return NetworkSubsystemState.shuttingDown;
      case 'error':
        return NetworkSubsystemState.error;
      case 'preview-active':
      case 'preview_active':
      default:
        return NetworkSubsystemState.previewActive;
    }
  }

  String get label {
    switch (this) {
      case NetworkSubsystemState.disabled:
        return 'Disabled (Kill Switch)';
      case NetworkSubsystemState.previewActive:
        return 'Ready (Preview)';
      case NetworkSubsystemState.connected:
        return 'Connected';
      case NetworkSubsystemState.shuttingDown:
        return 'Shutting Down';
      case NetworkSubsystemState.error:
        return 'Error';
    }
  }
}

class ResourceBudgetsModel {
  final int maxActivePeerConnections;
  final int maxActiveServerConnections;
  final int maxActiveListeners;
  final int connectTimeoutMs;
  final int handshakeTimeoutMs;
  final int maxFrameSizeBytes;
  final int maxTotalBufferBytes;
  final int rateLimitPerMin;

  const ResourceBudgetsModel({
    required this.maxActivePeerConnections,
    required this.maxActiveServerConnections,
    required this.maxActiveListeners,
    required this.connectTimeoutMs,
    required this.handshakeTimeoutMs,
    required this.maxFrameSizeBytes,
    required this.maxTotalBufferBytes,
    required this.rateLimitPerMin,
  });

  factory ResourceBudgetsModel.fromJson(Map<String, dynamic> json) {
    return ResourceBudgetsModel(
      maxActivePeerConnections: json['max_active_peer_connections'] as int? ?? 20,
      maxActiveServerConnections: json['max_active_server_connections'] as int? ?? 2,
      maxActiveListeners: json['max_active_listeners'] as int? ?? 2,
      connectTimeoutMs: json['connect_timeout_ms'] as int? ?? 10000,
      handshakeTimeoutMs: json['handshake_timeout_ms'] as int? ?? 15000,
      maxFrameSizeBytes: json['max_frame_size_bytes'] as int? ?? 10485760,
      maxTotalBufferBytes: json['max_total_buffer_bytes'] as int? ?? 33554432,
      rateLimitPerMin: json['rate_limit_per_min'] as int? ?? 60,
    );
  }

  Map<String, dynamic> toJson() => {
    'max_active_peer_connections': maxActivePeerConnections,
    'max_active_server_connections': maxActiveServerConnections,
    'max_active_listeners': maxActiveListeners,
    'connect_timeout_ms': connectTimeoutMs,
    'handshake_timeout_ms': handshakeTimeoutMs,
    'max_frame_size_bytes': maxFrameSizeBytes,
    'max_total_buffer_bytes': maxTotalBufferBytes,
    'rate_limit_per_min': rateLimitPerMin,
  };
}

class NetworkActiveConnections {
  final int peers;
  final int servers;
  final int listeners;

  const NetworkActiveConnections({
    this.peers = 0,
    this.servers = 0,
    this.listeners = 0,
  });

  factory NetworkActiveConnections.fromJson(Map<String, dynamic> json) {
    return NetworkActiveConnections(
      peers: json['peers'] as int? ?? 0,
      servers: json['servers'] as int? ?? 0,
      listeners: json['listeners'] as int? ?? 0,
    );
  }
}

class NetworkPolicySnapshot {
  final NetworkProfile profile;
  final NetworkSubsystemState state;
  final ResourceBudgetsModel budgets;
  final NetworkActiveConnections activeConnections;

  const NetworkPolicySnapshot({
    required this.profile,
    required this.state,
    required this.budgets,
    required this.activeConnections,
  });

  factory NetworkPolicySnapshot.fromJson(Map<String, dynamic> json) {
    return NetworkPolicySnapshot(
      profile: NetworkProfile.fromString(json['profile'] as String? ?? 'local-lab'),
      state: NetworkSubsystemState.fromString(json['state'] as String? ?? 'preview-active'),
      budgets: ResourceBudgetsModel.fromJson((json['budgets'] as Map<String, dynamic>?) ?? {}),
      activeConnections: NetworkActiveConnections.fromJson(
        (json['activeConnections'] as Map<String, dynamic>?) ?? {},
      ),
    );
  }
}

class NetworkAuditEntryItem {
  final int id;
  final int timestampEpochMs;
  final String eventType;
  final String? redactedTarget;
  final String? pseudonymizedTarget;
  final NetworkProfile profile;
  final String decision;
  final String? reason;

  const NetworkAuditEntryItem({
    required this.id,
    required this.timestampEpochMs,
    required this.eventType,
    this.redactedTarget,
    this.pseudonymizedTarget,
    required this.profile,
    required this.decision,
    this.reason,
  });

  factory NetworkAuditEntryItem.fromJson(Map<String, dynamic> json) {
    return NetworkAuditEntryItem(
      id: json['id'] as int? ?? 0,
      timestampEpochMs: json['timestamp_epoch_ms'] as int? ?? 0,
      eventType: json['event_type'] as String? ?? '',
      redactedTarget: json['redacted_target'] as String?,
      pseudonymizedTarget: json['pseudonymized_target'] as String?,
      profile: NetworkProfile.fromString(json['profile'] as String? ?? 'local-lab'),
      decision: json['decision'] as String? ?? '',
      reason: json['reason'] as String?,
    );
  }
}
