// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

import '../../../../l10n/l10n_lookup.dart';

/// Sub-domain controller for peer sources and the peer session.
///
/// Owns the per-hash source lists, the loading flag and the live peer
/// session (driven by `client.peerSessionEvents`), plus the
/// `server.foundSources` event subscription which auto-refreshes source
/// lists. Status and error reporting is delegated to the host via
/// [onStatus]/[onError].
class PeerSourcesController extends ChangeNotifier {
  final MuleDaemonClient client;

  /// Called with a transient status message (shown in the shell banner).
  final void Function(String message)? onStatus;

  /// Called with an error message on failed RPC calls.
  final void Function(String message)? onError;

  PeerSourcesController({required this.client, this.onStatus, this.onError}) {
    _foundSourcesSub = client.foundSourcesEvents.listen((params) {
      final rawSources = params['sources'] as List<dynamic>? ?? [];
      final hash = params['file_hash'] as String? ?? '';
      onStatus?.call(
        L10nLookup.current.msgSourcesFound(hash, rawSources.length),
      );
      if (hash.isNotEmpty) {
        loadSourcesForHash(hash);
      }
      notifyListeners();
    });

    _sessionSub = client.peerSessionEvents.listen((session) {
      _session = session;
      _isConnecting = session.status == PeerConnectionStatus.connecting ||
          session.status == PeerConnectionStatus.handshaking;
      notifyListeners();
    });
  }

  final Map<String, List<PeerSourceModel>> _sources = {};
  final Map<String, SwarmStatusModel> _swarms = {};
  final Map<String, DownloadItem> _details = {};
  bool _isLoading = false;
  PeerSessionModel _session = PeerSessionModel.disconnected;
  bool _isConnecting = false;
  StreamSubscription<Map<String, dynamic>>? _foundSourcesSub;
  StreamSubscription<PeerSessionModel>? _sessionSub;

  bool get isLoading => _isLoading;
  PeerSessionModel get session => _session;
  bool get isConnecting => _isConnecting;

  List<PeerSourceModel> getSourcesForHash(String fileHash) {
    return _sources[fileHash] ?? [];
  }

  /// Sources visibles façon eMule « srclist » : candidates, en attente de
  /// re-demande, en connexion ou connectées. Les entrées deadlist
  /// (échec, rejeté, filtré IP) restent persistées pour le backoff mais
  /// ne sont pas affichées — comme la `DeadSourceList` d'eMule.
  ///
  /// Les pairs déjà rendus dans la section des sessions actives
  /// (swarm ou session courante) ne sont pas répétés dans la liste.
  List<PeerSourceModel> getVisibleSourcesForHash(String fileHash) {
    final shown = <String>{
      for (final p in getSwarmForHash(fileHash).peers)
        if (p.peerAddress != null) p.peerAddress!,
    };
    final s = _session;
    if (s.fileHash == fileHash &&
        s.status != PeerConnectionStatus.disconnected &&
        s.peerAddress != null) {
      shown.add(s.peerAddress!);
    }
    return getSourcesForHash(fileHash)
        .where(
          (src) =>
              src.status.isListedInSourceList && !shown.contains(src.address),
        )
        .toList();
  }

  /// Live swarm sessions (connected peers, rates, queue ranks) for a file.
  SwarmStatusModel getSwarmForHash(String fileHash) {
    return _swarms[fileHash] ?? SwarmStatusModel.empty;
  }

  /// Full download detail (includes the per-part progress map) for a row id.
  DownloadItem? getDetailForId(String downloadId) => _details[downloadId];

  Future<void> loadSwarmForHash(String fileHash) async {
    try {
      _swarms[fileHash] = await client.getSwarmStatus(fileHash: fileHash);
      notifyListeners();
    } catch (_) {
      // Swarm status is best-effort live telemetry — ignore failures.
    }
  }

  Future<void> loadDetailForId(String downloadId) async {
    try {
      final detail = await client.getDownload(downloadId);
      if (detail != null) {
        _details[downloadId] = detail;
        notifyListeners();
      }
    } catch (_) {
      // Detail (part map) is best-effort — ignore failures.
    }
  }

  Future<void> loadSourcesForHash(String fileHash) async {
    _isLoading = true;
    notifyListeners();
    try {
      _sources[fileHash] = await client.listSources(fileHash);
      unawaited(loadSwarmForHash(fileHash));
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrLoadSources(e));
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<bool> addManualSource(String fileHash, String address) async {
    try {
      final src = await client.addManualSource(fileHash, address);
      final list = _sources.putIfAbsent(fileHash, () => []);
      list.removeWhere((s) => s.address == address);
      list.insert(0, src);
      onStatus?.call(L10nLookup.current.msgSourceAdded(address));
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrAddSource(e));
      notifyListeners();
      return false;
    }
  }

  Future<bool> connect(
    String fileHash,
    String address, {
    bool force = false,
  }) async {
    _isConnecting = true;
    notifyListeners();
    try {
      _session = await client.connectPeerSource(
        fileHash,
        address,
        force: force,
      );
      onStatus?.call(L10nLookup.current.msgConnInitiated(address));
      await loadSourcesForHash(fileHash);
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrConnectSource(e));
      await loadSourcesForHash(fileHash);
      return false;
    } finally {
      _isConnecting = false;
      notifyListeners();
    }
  }

  Future<void> disconnect({String? address}) async {
    try {
      _session = await client.disconnectPeerSource(address: address);
      onStatus?.call(
        address != null
            ? L10nLookup.current.msgPeerDisconnected(address)
            : L10nLookup.current.msgSessionsDisconnected,
      );
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrDisconnectPeer(e));
      notifyListeners();
    }
  }

  /// Changes the eMule-style file priority ("low" | "normal" | "high").
  /// The daemon broadcasts the updated snapshot via `downloadEvents`.
  Future<void> setDownloadPriority(String downloadId, String priority) async {
    try {
      await client.setDownloadPriority(downloadId, priority);
      onStatus?.call(L10nLookup.current.msgPriorityChanged(priority));
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrPriority(e));
    }
  }

  /// Per-file source cap (eMule FT_MAXSOURCES); `null` restores global.
  Future<void> setDownloadMaxSources(String downloadId, int? maxSources) async {
    try {
      await client.setDownloadMaxSources(downloadId, maxSources);
      onStatus?.call(
        maxSources == null
            ? L10nLookup.current.msgSrcLimitReset
            : L10nLookup.current.msgSrcLimitSet(maxSources),
      );
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrSrcLimit(e));
    }
  }

  Future<void> clearSourcesForHash(String fileHash) async {
    try {
      await client.clearSources(fileHash);
      _sources.remove(fileHash);
      onStatus?.call(L10nLookup.current.msgSourcesCleared);
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrClearSources(e));
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _foundSourcesSub?.cancel();
    _sessionSub?.cancel();
    super.dispose();
  }
}
