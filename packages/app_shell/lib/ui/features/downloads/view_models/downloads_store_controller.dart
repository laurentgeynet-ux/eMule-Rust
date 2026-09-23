// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'dart:async';
import '../../../../core/system_actions.dart';
import '../../../../l10n/enum_labels.dart';
import '../../../../l10n/l10n_lookup.dart';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

/// Ordre d'affichage de la liste des téléchargements : les éléments
/// terminés en bas ; les autres triés par avancement décroissant (les plus
/// complets en haut). À égalité, les ajouts les plus récents d'abord.
int compareDownloadsForList(DownloadItem a, DownloadItem b) {
  final doneCmp =
      (a.status.isCompleted ? 1 : 0) - (b.status.isCompleted ? 1 : 0);
  if (doneCmp != 0) return doneCmp;
  final progressCmp = b.progress.compareTo(a.progress);
  if (progressCmp != 0) return progressCmp;
  return b.createdAt.compareTo(a.createdAt);
}

/// Sub-domain controller for the downloads store.
///
/// Owns the download map (fed by `downloads.list` and
/// `event.downloadUpdated`), the busy flag and the selected download.
/// Status and error reporting is delegated to the host via
/// [onStatus]/[onError]; [onRefresh] triggers a full multi-domain refresh
/// after mutations that affect other stores (add, import).
class DownloadsStoreController extends ChangeNotifier {
  final MuleDaemonClient client;

  final void Function(String message)? onStatus;
  final void Function(String message)? onError;

  /// Full refresh (settings, history, categories, downloads list…).
  final Future<void> Function()? onRefresh;

  DownloadsStoreController({
    required this.client,
    this.onStatus,
    this.onError,
    this.onRefresh,
  }) {
    _eventSub = client.downloadEvents.listen((event) {
      _downloads[event.download.id] = event.download;
      notifyListeners();
    });
  }

  final Map<String, DownloadItem> _downloads = {};
  bool _isLoading = false;
  String? _selectedId;
  StreamSubscription<DownloadUpdatedEvent>? _eventSub;

  bool get isLoading => _isLoading;
  String? get selectedId => _selectedId;

  void setLoading(bool value) {
    _isLoading = value;
    notifyListeners();
  }

  List<DownloadItem> get allDownloads => _downloads.values.toList();

  /// Merge an updated item into the store (cross-domain callbacks).
  void upsert(DownloadItem item) {
    _downloads[item.id] = item;
    notifyListeners();
  }

  /// Remove an entry locally (event.downloadRemoved).
  void removeLocal(String id) {
    if (_downloads.remove(id) != null) {
      notifyListeners();
    }
  }

  /// Drop all entries (daemon restart / instance change).
  void clearAll() {
    _downloads.clear();
    notifyListeners();
  }

  DownloadItem? downloadById(String id) => _downloads[id];

  DownloadItem? getDownloadByHash(String? hash) {
    if (hash == null || hash.isEmpty) return null;
    final needle = hash.toLowerCase();
    for (final d in _downloads.values) {
      if (d.ed2kHash?.toLowerCase() == needle) return d;
    }
    return null;
  }

  int get totalDownloadSpeed => _downloads.values
      .where((d) => d.status.isActive)
      .fold<int>(0, (sum, d) => sum + d.downloadRateBps);

  int get countAll => _downloads.length;
  int get countActive =>
      _downloads.values.where((d) => d.status.isActive).length;
  int get countPaused =>
      _downloads.values.where((d) => d.status.isPaused).length;
  int get countCompleted =>
      _downloads.values.where((d) => d.status.isCompleted).length;
  int get countFailed =>
      _downloads.values.where((d) => d.status.isFailed).length;

  /// Reload the list from `downloads.list`. Throws on failure so the host
  /// can surface the error through its own error path.
  Future<void> reload() async {
    final list = await client.listDownloads();
    _downloads.clear();
    for (final item in list) {
      _downloads[item.id] = item;
    }
    notifyListeners();
  }

  void select(String? id) {
    _selectedId = id;
    if (id != null) {
      fetchDetail(id);
    }
    notifyListeners();
  }

  Future<bool> addEd2kLink(
    String link, {
    String? targetDirectory,
    String? allocationPolicy,
    String? categoryId,
  }) async {
    try {
      _isLoading = true;
      notifyListeners();

      // Check for existing download with the same eD2K hash
      final tokens = link.split('|');
      String? linkHash;
      if (tokens.length >= 5 && tokens[1].toLowerCase() == 'file') {
        linkHash = tokens[4].toLowerCase();
      }
      if (linkHash != null) {
        final existing = getDownloadByHash(linkHash);
        if (existing != null) {
          onError?.call(
            L10nLookup.current.msgDownloadDuplicate(
              existing.fileName,
              existing.status.labelL10n(L10nLookup.current),
            ),
          );
          return false;
        }
      }

      final dlId = await client.addEd2kLink(
        link,
        targetDirectory: targetDirectory,
        allocationPolicy: allocationPolicy,
        categoryId: categoryId,
      );

      // Extract human-friendly filename from eD2K link for clean user notification
      String displayName = dlId;
      if (tokens.length >= 4 && tokens[1].toLowerCase() == 'file') {
        final rawName = tokens[2];
        try {
          displayName = Uri.decodeComponent(rawName);
        } catch (_) {
          displayName = rawName;
        }
      }

      onStatus?.call(L10nLookup.current.msgDownloadAdded(displayName));
      await onRefresh?.call();
      return true;
    } catch (e) {
      final msg = e.toString().replaceAll(RegExp(r'^Exception:\s*'), '');
      onError?.call(msg);
      return false;
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<PartMetInspectionResult> inspectPartMet(
    String metPath, {
    String? partPath,
  }) =>
      client.inspectPartMet(metPath, partPath: partPath);

  Future<ImportPartMetResult?> importPartMet(
    String metPath, {
    String? partPath,
    String? targetDirectory,
    String? categoryId,
  }) async {
    try {
      _isLoading = true;
      notifyListeners();

      final res = await client.importPartMet(
        metPath,
        partPath: partPath,
        targetDirectory: targetDirectory,
        categoryId: categoryId,
      );

      _downloads[res.download.id] = res.download;
      _selectedId = res.download.id;
      onStatus?.call(
        L10nLookup.current.msgImportedFile(
          res.download.fileName,
          res.totalParts,
          res.verifiedParts,
        ),
      );
      await onRefresh?.call();
      return res;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrImportPartMet(e));
      return null;
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<void> pause(String id) async {
    try {
      final updated = await client.pauseDownload(id);
      _downloads[updated.id] = updated;
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrPause(e));
      notifyListeners();
    }
  }

  Future<void> resume(String id) async {
    try {
      final updated = await client.resumeDownload(id);
      _downloads[updated.id] = updated;
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrResume(e));
      notifyListeners();
    }
  }

  /// `downloads.rename` — renomme le fichier (en cours ou terminé ; le
  /// daemon renomme aussi le fichier disque et l'entrée `shared_files`
  /// associée). Retourne `false` si le daemon refuse (nom invalide,
  /// collision…).
  Future<bool> rename(String id, String newName) async {
    try {
      final updated = await client.renameDownload(id, newName);
      _downloads[updated.id] = updated;
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrRename(e));
      notifyListeners();
      return false;
    }
  }

  Future<void> remove(String id, {bool deleteFiles = false}) async {
    try {
      await client.removeDownload(id, deleteFiles: deleteFiles);
      _downloads.remove(id);
      if (_selectedId == id) {
        _selectedId = null;
      }
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrRemove(e));
      notifyListeners();
    }
  }

  Future<DownloadItem?> fetchDetail(String id) async {
    try {
      final detail = await client.getDownload(id);
      if (detail != null) {
        _downloads[detail.id] = detail;
        notifyListeners();
      }
      return detail;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrFetchDetails(e));
      notifyListeners();
      return null;
    }
  }

  Future<bool> changeDestination(String id, String newDirectory) async {
    try {
      _isLoading = true;
      notifyListeners();
      final updated = await client.setDownloadDestination(id, newDirectory);
      _downloads[updated.id] = updated;
      onStatus?.call(
        L10nLookup.current.msgDestinationChanged(newDirectory),
      );
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrChangeDestination(e));
      notifyListeners();
      return false;
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<bool> retryConnection(String id) async {
    try {
      final updated = await client.retryDownloadConnection(id);
      _downloads[updated.id] = updated;
      onStatus?.call(
        L10nLookup.current.msgRetryLoopback(updated.fileName),
      );
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrRetryConn(e));
      notifyListeners();
      return false;
    }
  }

  Future<void> openCompletedFile(DownloadItem item) async {
    if (!item.status.isCompleted) return;
    final targetPath = item.targetPath;
    if (targetPath == null || targetPath.isEmpty) return;
    try {
      await systemActions.revealFile(targetPath);
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrOpenFile(e));
      notifyListeners();
    }
  }

  String getRedactedDiagnostic(DownloadItem item) {
    String sanitizedTarget;
    try {
      final sep = systemActions.pathSeparator;
      sanitizedTarget = '[DOWNLOAD_DIR]$sep${item.fileName}';
    } catch (_) {
      sanitizedTarget = '[DOWNLOAD_DIR]/${item.fileName}';
    }

    final sanitizedTemp = 'data/temp/${item.id}.part';
    final totalParts = item.parts.length;
    final verifiedParts = item.parts.where((p) => p.isVerified).length;
    final completedParts = item.parts
        .where((p) => p.state == 'completed')
        .length;
    final downloadingParts = item.parts
        .where((p) => p.state == 'downloading')
        .length;
    final missingParts = item.parts.where((p) => p.state == 'missing').length;

    final buffer = StringBuffer();
    buffer.writeln('=== EMule-Rust Download Diagnostic (Sanitized) ===');
    buffer.writeln('Generated: ${DateTime.now().toUtc().toIso8601String()}');
    buffer.writeln(
      'Confinement: Loopback 127.0.0.1 (Zero WAN traffic, secrets redacted)',
    );
    buffer.writeln('Download ID: ${item.id}');
    buffer.writeln('File Name: ${item.fileName}');
    buffer.writeln('Status: ${item.status.label}');
    buffer.writeln('Size: ${item.totalBytes} bytes');
    buffer.writeln('Received: ${item.receivedBytes} bytes');
    buffer.writeln('Verified: ${item.verifiedBytes} bytes');
    buffer.writeln('Progress: ${(item.progress * 100).toStringAsFixed(1)}%');
    buffer.writeln('Current Speed: ${item.downloadRateBps} B/s');
    buffer.writeln('Average Speed: ${item.averageRateBps} B/s');
    if (item.etaSeconds != null) {
      buffer.writeln('ETA: ${item.etaSeconds}s');
    }
    buffer.writeln('Connection Attempts: ${item.connectionAttempts}');
    if (item.lastError != null) {
      buffer.writeln('Last Error: ${item.lastError}');
    }
    if (item.ed2kHash != null) {
      buffer.writeln('eD2K Hash: ${item.ed2kHash}');
    }
    if (item.ed2kLink != null) {
      buffer.writeln('Canonical Link: ${item.ed2kLink}');
    }
    buffer.writeln('Temp Path: $sanitizedTemp');
    buffer.writeln('Destination Target: $sanitizedTarget');
    buffer.writeln(
      'Part Map Summary: $totalParts parts total ($verifiedParts verified, $completedParts unverified, $downloadingParts downloading, $missingParts missing)',
    );
    buffer.writeln('Created At: ${item.createdAt.toIso8601String()}');
    buffer.writeln('Updated At: ${item.updatedAt.toIso8601String()}');
    return buffer.toString();
  }

  @override
  void dispose() {
    _eventSub?.cancel();
    super.dispose();
  }
}
