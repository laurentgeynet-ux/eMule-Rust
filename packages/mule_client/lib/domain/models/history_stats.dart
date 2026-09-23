// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

/// Aggregated historical and session statistics for downloads and integrity.
class HistoryStats {
  final int totalCompletedCount;
  final int totalCompletedBytes;
  final int totalFailedCount;
  final int totalResumesCount;
  final int totalIntegrityFailures;

  final int sessionCompletedBytes;
  final int sessionCompletedCount;
  final int last24hCompletedBytes;
  final int last24hCompletedCount;
  final int last7dCompletedBytes;
  final int last7dCompletedCount;

  const HistoryStats({
    this.totalCompletedCount = 0,
    this.totalCompletedBytes = 0,
    this.totalFailedCount = 0,
    this.totalResumesCount = 0,
    this.totalIntegrityFailures = 0,
    this.sessionCompletedBytes = 0,
    this.sessionCompletedCount = 0,
    this.last24hCompletedBytes = 0,
    this.last24hCompletedCount = 0,
    this.last7dCompletedBytes = 0,
    this.last7dCompletedCount = 0,
  });
}
