// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/foundation.dart';

enum DaemonConnectionState {
  disconnected,
  connecting,
  connected,
  error;

  String get label {
    switch (this) {
      case DaemonConnectionState.disconnected:
        return 'Disconnected';
      case DaemonConnectionState.connecting:
        return 'Connecting';
      case DaemonConnectionState.connected:
        return 'Connected';
      case DaemonConnectionState.error:
        return 'Error';
    }
  }

  bool get isConnected => this == DaemonConnectionState.connected;
}

@immutable
class DaemonInfo {
  final String version;
  final int protocolVersion;
  final String instanceId;
  final String sessionId;
  final List<String> capabilities;
  final String? workdirMode;
  final String? workdirPath;
  final String? defaultDownloadDir;

  const DaemonInfo({
    required this.version,
    required this.protocolVersion,
    required this.instanceId,
    required this.sessionId,
    required this.capabilities,
    this.workdirMode,
    this.workdirPath,
    this.defaultDownloadDir,
  });

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is DaemonInfo &&
          runtimeType == other.runtimeType &&
          instanceId == other.instanceId &&
          sessionId == other.sessionId;

  @override
  int get hashCode => instanceId.hashCode ^ sessionId.hashCode;
}
