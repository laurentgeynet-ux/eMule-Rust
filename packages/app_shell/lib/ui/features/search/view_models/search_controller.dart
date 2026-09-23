// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

import '../../../../l10n/l10n_lookup.dart';

/// Sub-domain controller for the eD2K/Kad search section.
///
/// Owns the result list (hash-deduplicated), the searching flag, the
/// no-answer watchdog (30 s) and the `server.searchResult` event
/// subscription. Status and error reporting is delegated to the host via
/// [onStatus]/[onError].
class SearchController extends ChangeNotifier {
  final MuleDaemonClient client;

  /// Called with a transient status message (shown in the shell banner).
  final void Function(String message)? onStatus;

  /// Called with an error message on failed RPC calls.
  final void Function(String message)? onError;

  SearchController({required this.client, this.onStatus, this.onError}) {
    _resultSub = client.searchResultEvents.listen((files) {
      final existingHashes = _results.map((f) => f.fileHash).toSet();
      for (final f in files) {
        if (!existingHashes.contains(f.fileHash)) {
          _results.add(f);
          existingHashes.add(f.fileHash);
        }
      }
      _epoch++;
      _isSearching = false;
      // Results arrived — the no-answer watchdog is no longer needed.
      _timeout?.cancel();
      notifyListeners();
    });
  }

  final List<SearchResultFileModel> _results = [];
  bool _isSearching = false;
  String? _lastTerm;
  int _epoch = 0;
  Timer? _timeout;
  StreamSubscription<List<SearchResultFileModel>>? _resultSub;

  List<SearchResultFileModel> get results => _results;
  bool get isSearching => _isSearching;
  String? get lastTerm => _lastTerm;

  /// Bumped on every result-set change — lets views invalidate sort caches.
  int get epoch => _epoch;

  Future<void> searchServer({
    required String term,
    int? minSize,
    int? maxSize,
    String? extension,
    bool udpOnly = false,
  }) async {
    final trimmed = term.trim();
    if (trimmed.isEmpty) return;
    _isSearching = true;
    _results.clear();
    _epoch++;
    _lastTerm = trimmed;
    notifyListeners();

    try {
      final resp = await client.searchServer(
        term: trimmed,
        minSize: minSize,
        maxSize: maxSize,
        extension: extension,
        udpOnly: udpOnly,
      );
      final targets = resp['udpTargets'];
      var udpSummary = '';
      if (targets is List && targets.isNotEmpty) {
        var req3 = 0, req2 = 0, req1 = 0, failed = 0;
        for (final t in targets) {
          switch (t is Map ? t['request'] : null) {
            case 'REQ3':
              req3++;
            case 'REQ2':
              req2++;
            case 'REQ1':
              req1++;
            default:
              failed++;
          }
        }
        udpSummary = ' — UDP: $req3×REQ3, $req2×REQ2, $req1×REQ1'
            '${failed > 0 ? L10nLookup.current.searchUdpFailed(failed) : ''}';
      }
      onStatus?.call(
        udpOnly
            ? L10nLookup.current.msgSearchGlobalSent(trimmed, udpSummary)
            : L10nLookup.current.msgSearchSent(trimmed, udpSummary),
      );
      // eD2K servers may legitimately never answer (busy, throttled, dead):
      // without a reply the spinner would block every later search forever.
      // Auto-release after 30 s — late results still populate the list.
      _timeout?.cancel();
      _timeout = Timer(const Duration(seconds: 30), () {
        if (_isSearching) {
          _isSearching = false;
          onStatus?.call(
            L10nLookup.current.msgSearchNoResponse(trimmed),
          );
          notifyListeners();
        }
      });
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrSearch(e));
      _isSearching = false;
    } finally {
      notifyListeners();
    }
  }

  Future<void> searchKadKeywords({
    required String term,
    int? minSize,
    int? maxSize,
  }) async {
    final trimmed = term.trim();
    if (trimmed.isEmpty) return;
    _isSearching = true;
    _results.clear();
    _epoch++;
    _lastTerm = trimmed;
    notifyListeners();

    try {
      final results = await client.searchKadKeywords(
        term: trimmed,
        minSize: minSize,
        maxSize: maxSize,
      );
      final existingHashes = _results.map((f) => f.fileHash).toSet();
      for (final f in results) {
        if (!existingHashes.contains(f.fileHash)) {
          _results.add(f);
          existingHashes.add(f.fileHash);
        }
      }
      _epoch++;
      onStatus?.call(
        L10nLookup.current.msgKadSearchDone(_results.length, trimmed),
      );
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrKadSearchRun(e));
    } finally {
      _isSearching = false;
      notifyListeners();
    }
  }

  void clearResults() {
    _results.clear();
    _epoch++;
    _isSearching = false;
    _lastTerm = null;
    notifyListeners();
  }

  Future<void> querySources(String fileHash, {int? fileSize}) async {
    try {
      await client.queryServerSources(fileHash: fileHash, fileSize: fileSize);
      onStatus?.call(L10nLookup.current.msgSourceReqSent(fileHash));
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrSourceReq(e));
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _timeout?.cancel();
    _resultSub?.cancel();
    super.dispose();
  }
}
