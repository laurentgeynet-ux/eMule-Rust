// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:mule_client/mule_client.dart';

import '../core/daemon_lifecycle.dart';
import 'generated/app_localizations.dart';

/// Libellés localisés des enums définis dans `mule_client` — le package
/// client ne dépend pas des localisations, donc les getters `.label`
/// restent en anglais technique (logs, rapports de diagnostic). Ces
/// extensions exposent la version affichable dans l'UI.
extension DaemonConnectionStateL10n on DaemonConnectionState {
  String labelL10n(AppLocalizations l10n) {
    switch (this) {
      case DaemonConnectionState.disconnected:
        return l10n.connDisconnected;
      case DaemonConnectionState.connecting:
        return l10n.connConnecting;
      case DaemonConnectionState.connected:
        return l10n.connConnected;
      case DaemonConnectionState.error:
        return l10n.connError;
    }
  }
}

extension ServerConnectionStatusL10n on ServerConnectionStatus {
  String labelL10n(AppLocalizations l10n) {
    switch (this) {
      case ServerConnectionStatus.connecting:
        return l10n.connConnectingEllipsis;
      case ServerConnectionStatus.connected:
        return l10n.connConnected;
      case ServerConnectionStatus.disconnecting:
        return l10n.connDisconnecting;
      case ServerConnectionStatus.disconnected:
        return l10n.connDisconnected;
    }
  }
}

extension PeerConnectionStatusL10n on PeerConnectionStatus {
  String labelL10n(AppLocalizations l10n) {
    switch (this) {
      case PeerConnectionStatus.disconnected:
        return l10n.connDisconnected;
      case PeerConnectionStatus.connecting:
        return l10n.connConnectingEllipsis;
      case PeerConnectionStatus.handshaking:
        return l10n.peerHandshaking;
      case PeerConnectionStatus.queued:
        return l10n.peerQueued;
      case PeerConnectionStatus.transferring:
        return l10n.peerTransferring;
      case PeerConnectionStatus.disconnecting:
        return l10n.connDisconnecting;
    }
  }
}

extension DownloadStatusL10n on DownloadStatus {
  String labelL10n(AppLocalizations l10n) {
    switch (this) {
      case DownloadStatus.queued:
        return l10n.dlStatusQueued;
      case DownloadStatus.waitingForSources:
        return l10n.dlStatusWaitingForSources;
      case DownloadStatus.allocating:
        return l10n.dlStatusAllocating;
      case DownloadStatus.downloading:
        return l10n.dlStatusDownloading;
      case DownloadStatus.paused:
        return l10n.dlStatusPaused;
      case DownloadStatus.verifying:
        return l10n.dlStatusVerifying;
      case DownloadStatus.completing:
        return l10n.dlStatusCompleting;
      case DownloadStatus.completed:
        return l10n.dlStatusCompleted;
      case DownloadStatus.failed:
        return l10n.dlStatusFailed;
    }
  }
}

extension DownloadPriorityL10n on DownloadPriority {
  String labelL10n(AppLocalizations l10n) {
    switch (this) {
      case DownloadPriority.low:
        return l10n.low;
      case DownloadPriority.normal:
        return l10n.normal;
      case DownloadPriority.high:
        return l10n.high;
      case DownloadPriority.auto:
        return 'Auto';
    }
  }
}

extension NetworkProfileL10n on NetworkProfile {
  String labelL10n(AppLocalizations l10n) {
    switch (this) {
      case NetworkProfile.localLab:
        return l10n.profileLocalLab;
      case NetworkProfile.publicEd2kPreview:
        return l10n.profilePublicEd2k;
      case NetworkProfile.publicEd2kKadPreview:
        return l10n.profilePublicEd2kKad;
    }
  }
}

extension NetworkSubsystemStateL10n on NetworkSubsystemState {
  String labelL10n(AppLocalizations l10n) {
    switch (this) {
      case NetworkSubsystemState.disabled:
        return l10n.subsystemDisabled;
      case NetworkSubsystemState.previewActive:
        return l10n.subsystemReady;
      case NetworkSubsystemState.connected:
        return l10n.connConnected;
      case NetworkSubsystemState.shuttingDown:
        return l10n.subsystemShuttingDown;
      case NetworkSubsystemState.error:
        return l10n.connError;
    }
  }
}

/// États du superviseur de daemon (`app_shell` — pas dans mule_client).
extension SupervisorStateL10n on SupervisorState {
  String labelL10n(AppLocalizations l10n) {
    switch (this) {
      case SupervisorState.idle:
        return l10n.daemonIdle;
      case SupervisorState.discovering:
        return l10n.daemonDiscovering;
      case SupervisorState.starting:
        return l10n.daemonStarting;
      case SupervisorState.connected:
        return l10n.daemonConnected;
      case SupervisorState.reconnecting:
        return l10n.daemonReconnecting;
      case SupervisorState.protocolMismatch:
        return l10n.daemonProtocolMismatch;
      case SupervisorState.authError:
        return l10n.daemonAuthError;
      case SupervisorState.error:
        return l10n.daemonError;
    }
  }
}
