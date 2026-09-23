// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

/// Sub-domain controller for the Uploads section.
///
/// Subscribes to `upload.uploadsUpdated` events ([MuleDaemonClient
/// .uploadsEvents]) and keeps the latest snapshot: active upload sessions,
/// the waiting queue and the aggregate upload rate. An initial `uploads.list`
/// fetch seeds the view before the first event arrives.
class UploadsController extends ChangeNotifier {
  final MuleDaemonClient client;

  UploadsController({required this.client});

  UploadsSnapshotModel _snapshot = const UploadsSnapshotModel();
  StreamSubscription<UploadsSnapshotModel>? _sub;

  UploadsSnapshotModel get snapshot => _snapshot;
  List<UploadSessionModel> get active => _snapshot.active;
  List<UploadSessionModel> get queue => _snapshot.queue;
  int get totalUploadRateBps => _snapshot.uploadRateBps;

  /// Subscribes to daemon upload events. Idempotent.
  void attach() {
    _sub ??= client.uploadsEvents.listen(_onSnapshot);
  }

  void _onSnapshot(UploadsSnapshotModel s) {
    _snapshot = s;
    notifyListeners();
  }

  /// Seeds the snapshot from `uploads.list` (RPC fallback used before the
  /// first event, e.g. right after connecting).
  Future<void> fetch() async {
    try {
      final raw = await client.listUploads();
      List<UploadSessionModel> parse(String key) =>
          ((raw[key] as List?) ?? const [])
              .whereType<Map<String, dynamic>>()
              .map(UploadSessionModel.fromJson)
              .toList();
      _snapshot = UploadsSnapshotModel(
        active: parse('active'),
        queue: parse('queued'),
        uploadRateBps: _snapshot.uploadRateBps,
      );
      notifyListeners();
    } catch (_) {
      // Non-fatal: the next event will refresh the snapshot.
    }
  }

  /// Cancels the upload session for [clientAddress] (`uploads.cancel`).
  Future<void> cancelUpload(String clientAddress) async {
    try {
      await client.cancelUpload(clientAddress);
      await fetch();
    } catch (_) {
      // Non-fatal: the next event will refresh the snapshot.
    }
  }

  @override
  void dispose() {
    _sub?.cancel();
    super.dispose();
  }
}
