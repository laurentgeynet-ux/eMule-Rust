// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'work_dirs.dart';

/// Lifecycle state of the daemon supervisor.
enum SupervisorState {
  idle,
  discovering,
  starting,
  connected,
  reconnecting,
  protocolMismatch,
  authError,
  error;

  String get label {
    switch (this) {
      case SupervisorState.idle:
        return 'Idle';
      case SupervisorState.discovering:
        return 'Discovering Daemon...';
      case SupervisorState.starting:
        return 'Starting Daemon...';
      case SupervisorState.connected:
        return 'Daemon Connected';
      case SupervisorState.reconnecting:
        return 'Reconnecting...';
      case SupervisorState.protocolMismatch:
        return 'Protocol Version Incompatible';
      case SupervisorState.authError:
        return 'Authentication Failed';
      case SupervisorState.error:
        return 'Daemon Error';
    }
  }

  bool get isConnected => this == SupervisorState.connected;
}

/// Pure-Dart view of an active daemon session.
abstract class DaemonSessionInfo {
  int get protocolVersion;
  String get instanceId;
  int get pid;
  String get endpoint;
  int get startedAtUnixMs;
}

/// Abstraction over the daemon process supervisor. The desktop app provides
/// `DaemonSupervisor` (spawns/manages the local mule-daemon process); remote
/// clients (web) pass null and connect to an already-running daemon.
abstract class DaemonLifecycle {
  SupervisorState get state;
  String? get lastError;
  int? get activePid;
  DaemonSessionInfo? get activeSession;
  Stream<SupervisorState> get stateStream;
  Stream<void> get instanceChangedStream;

  /// Resolves the daemon working/download directories.
  WorkDirInfo resolveWorkDirInfo();

  Future<void> start();
  Future<void> stop();
  void dispose();
}
