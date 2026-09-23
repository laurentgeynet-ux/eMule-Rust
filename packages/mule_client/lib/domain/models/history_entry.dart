// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

/// Historical record of a download transition event in EMule-Rust.
class HistoryEntry {
  final int id;
  final String downloadId;
  final String fileName;
  final int totalBytes;
  final int transferredBytes;
  final String ed2kHash;
  final String eventType; // 'completed', 'failed', 'removed', 'verified_pending_promotion', 'resumed'
  final String status;
  final int durationSecs;
  final int connectionAttempts;
  final int resumeCount;
  final String verificationResult; // 'verified_ok', 'hash_mismatch', 'unverified', 'skipped'
  final String? errorMessage;
  final DateTime timestamp;

  const HistoryEntry({
    required this.id,
    required this.downloadId,
    required this.fileName,
    required this.totalBytes,
    required this.transferredBytes,
    required this.ed2kHash,
    required this.eventType,
    required this.status,
    required this.durationSecs,
    required this.connectionAttempts,
    required this.resumeCount,
    required this.verificationResult,
    this.errorMessage,
    required this.timestamp,
  });
}
