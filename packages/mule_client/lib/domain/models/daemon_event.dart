// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/foundation.dart';
import 'download_item.dart';

@immutable
class DownloadUpdatedEvent {
  final int sequence;
  final DownloadItem download;

  const DownloadUpdatedEvent({
    required this.sequence,
    required this.download,
  });

  @override
  String toString() => 'DownloadUpdatedEvent(seq: $sequence, dl: ${download.id}, progress: ${download.progress})';
}

/// Lifecycle transitions of a download (`event.downloadAdded/Removed/
/// Completed/Failed`). Distinct from [DownloadUpdatedEvent] (progression) —
/// lets the UI react to queue changes without diffing `downloads.list`.
enum DownloadLifecycleKind { added, removed, completed, failed }

@immutable
class DownloadLifecycleEvent {
  final int sequence;
  final DownloadLifecycleKind kind;

  /// Snapshot of the download (null for [DownloadLifecycleKind.removed]).
  final DownloadItem? download;

  /// Download id (set for [DownloadLifecycleKind.removed]).
  final String? id;

  /// Failure reason when [kind] is `failed`.
  final String? reason;

  const DownloadLifecycleEvent({
    required this.sequence,
    required this.kind,
    this.download,
    this.id,
    this.reason,
  });
}

/// `settings.changed` notification: lists the setting keys touched so
/// clients can decide whether to refetch.
@immutable
class SettingsChangedEvent {
  final int sequence;
  final List<String> keys;

  const SettingsChangedEvent({required this.sequence, this.keys = const []});
}
