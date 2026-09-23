// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../core/daemon_lifecycle.dart';

/// Status dot color for the raw daemon connection state.
Color connectionStateColor(DaemonConnectionState state) {
  switch (state) {
    case DaemonConnectionState.connected:
      return AppTheme.statusCompleted;
    case DaemonConnectionState.connecting:
      return AppTheme.statusPaused;
    case DaemonConnectionState.error:
      return AppTheme.statusFailed;
    case DaemonConnectionState.disconnected:
      return AppTheme.statusQueued;
  }
}

/// Status dot color for the daemon supervisor lifecycle state.
Color supervisorStateColor(SupervisorState state) {
  switch (state) {
    case SupervisorState.connected:
      return AppTheme.statusCompleted;
    case SupervisorState.starting:
    case SupervisorState.discovering:
    case SupervisorState.reconnecting:
      return AppTheme.statusPaused;
    case SupervisorState.protocolMismatch:
    case SupervisorState.authError:
    case SupervisorState.error:
      return AppTheme.statusFailed;
    case SupervisorState.idle:
      return AppTheme.statusQueued;
  }
}
