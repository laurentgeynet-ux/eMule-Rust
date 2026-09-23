// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

/// Sub-domain controller for the Shares section (`/shares`).
///
/// Keeps the latest `shares.list` snapshot — persisted shared files plus
/// active downloads published as partial shares (eMule `CPartFile`
/// semantics). Download lifecycle events refresh the list immediately;
/// progress events (which grow `completedParts` on partial shares) are
/// debounced. An initial `fetch()` seeds the view before the first event.
class SharesController extends ChangeNotifier {
  final MuleDaemonClient client;

  SharesController({required this.client});

  List<SharedFileModel> _files = const [];
  bool _loading = true;
  bool _disposed = false;
  StreamSubscription<DownloadLifecycleEvent>? _lifecycleSub;
  StreamSubscription<DownloadUpdatedEvent>? _updateSub;
  StreamSubscription<int>? _sharesSub;
  StreamSubscription<UploadsSnapshotModel>? _uploadsSub;
  Timer? _updateDebounce;
  Timer? _uploadsDebounce;

  List<SharedFileModel> get files => _files;
  int get count => _files.length;

  /// `true` until the first `shares.list` answer arrives.
  bool get loading => _loading;

  /// Subscribes to daemon download + `shares.changed` events. Idempotent.
  void attach() {
    _lifecycleSub ??= client.downloadLifecycleEvents.listen((_) => fetch());
    // Share mutations outside download events (shares.add/remove/rescan,
    // MCP tools) — keeps the sidebar badge live.
    _sharesSub ??= client.sharesEvents.listen((_) => fetch());
    _updateSub ??= client.downloadEvents.listen((_) {
      _updateDebounce ??= Timer(const Duration(seconds: 3), () {
        _updateDebounce = null;
        fetch();
      });
    });
    // `upload.uploadsUpdated` (~2 s pendant l'activité + une dernière
    // émission à la libération du slot) pilote le rafraîchissement des
    // colonnes débit/total d'upload — le débit retombe à 0 sur la dernière
    // émission quand le dernier slot se ferme.
    _uploadsSub ??= client.uploadsEvents.listen((_) {
      _uploadsDebounce ??= Timer(const Duration(seconds: 1), () {
        _uploadsDebounce = null;
        fetch();
      });
    });
  }

  /// Reloads the shared-files list (`shares.list` — includes partial
  /// shares for in-progress downloads).
  Future<void> fetch() async {
    try {
      final files = await client.listSharedFiles();
      if (_disposed) return;
      _files = files;
      _loading = false;
      notifyListeners();
    } catch (_) {
      // Non-fatal: the next event will refresh the list.
      _loading = false;
      if (!_disposed) notifyListeners();
    }
  }

  /// Re-scans the Incoming directory on the daemon (`shares.rescan`),
  /// then refreshes the list. Returns the number of newly shared files.
  Future<int> rescan() async {
    final added = await client.rescanSharedFiles();
    await fetch();
    return added;
  }

  /// `shares.setEnabled` — enable/suspend a shared file (rejected by the
  /// daemon for partial shares).
  Future<void> setEnabled(String fileHash, bool enabled) async {
    await client.setSharedFileEnabled(fileHash, enabled);
    await fetch();
  }

  /// `shares.setUploadPriority` — eMule upload priority (rejected by the
  /// daemon for partial shares).
  Future<void> setUploadPriority(String fileHash, String priority) async {
    await client.setSharedFileUploadPriority(fileHash, priority);
    await fetch();
  }

  /// `shares.setCategory` — associe (ou retire avec `null`) une catégorie
  /// au fichier partagé ; pour un partage synthétisé depuis un download,
  /// c'est la catégorie du download qui est mise à jour.
  Future<void> setSharedFileCategory(
    String fileHash,
    String? categoryId,
  ) async {
    await client.setSharedFileCategory(fileHash, categoryId);
    await fetch();
  }

  /// `shares.remove` — retire le partage et supprime le fichier du disque
  /// (le daemon refuse pour les partages partiels de downloads en cours).
  Future<void> removeSharedFile(String fileHash) async {
    await client.removeSharedFile(fileHash, deleteFile: true);
    await fetch();
  }

  /// `shares.rename` — renomme le fichier partagé sur disque et en base
  /// (le download complété associé suit s'il existe).
  Future<void> renameSharedFile(String fileHash, String newName) async {
    await client.renameSharedFile(fileHash, newName);
    await fetch();
  }

  @override
  void dispose() {
    _disposed = true;
    _updateDebounce?.cancel();
    _uploadsDebounce?.cancel();
    _lifecycleSub?.cancel();
    _updateSub?.cancel();
    _sharesSub?.cancel();
    _uploadsSub?.cancel();
    super.dispose();
  }
}
