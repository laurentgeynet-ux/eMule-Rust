// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

import '../../../../l10n/l10n_lookup.dart';

/// Sub-domain controller for the Kad DHT section.
///
/// Owns the Kad status, contact table, loading flag and the periodic status
/// refresh timer (every 3 s while the daemon is connected; contacts are only
/// re-listed while the Kad section is visible). Status and error reporting is
/// delegated to the host via [onStatus]/[onError].
class KadController extends ChangeNotifier {
  final MuleDaemonClient client;

  /// Called with a transient status message (shown in the shell banner).
  final void Function(String message)? onStatus;

  /// Called with an error message on failed RPC calls.
  final void Function(String message)? onError;

  /// Whether the Kad section is currently visible — gates the periodic
  /// contacts refresh.
  final bool Function() isSectionActive;

  KadController({
    required this.client,
    required this.isSectionActive,
    this.onStatus,
    this.onError,
  }) {
    _connSub = client.connectionState.listen((state) {
      if (state == DaemonConnectionState.connected) {
        fetchStatus();
        fetchContacts();
      }
    });
    // Event-driven refresh (`kad.statusChanged`) — replaces the former 3 s
    // polling timer.
    _kadSub = client.kadStatusEvents.listen((_) {
      fetchStatus();
      if (isSectionActive()) fetchContacts();
    });
  }

  KadStatusModel _status = KadStatusModel.initial;
  List<KadContactModel> _contacts = [];
  bool _isLoading = false;
  StreamSubscription<DaemonConnectionState>? _connSub;
  StreamSubscription<int>? _kadSub;

  KadStatusModel get status => _status;
  List<KadContactModel> get contacts => _contacts;
  bool get isLoading => _isLoading;

  Future<void> fetchStatus() async {
    try {
      _status = await client.getKadStatus();
      notifyListeners();
    } catch (e) {
      debugPrint('Failed to fetch Kad status: $e');
    }
  }

  Future<void> fetchContacts({int limit = 100}) async {
    _isLoading = true;
    notifyListeners();
    try {
      _contacts = await client.listKadContacts(limit: limit);
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrKadContacts(e));
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<NodesDatInspectReportModel?> inspectNodesDat(String filePath) async {
    try {
      return await client.inspectNodesDat(filePath);
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrNodesDatInspect(e));
      return null;
    }
  }

  Future<bool> importNodesDat(String filePath) async {
    return _runLoading(() async {
      final res = await client.importNodesDat(filePath);
      final count = res['importedCount'] ?? 0;
      onStatus?.call(L10nLookup.current.msgKadContactsImported(count));
      await fetchStatus();
      await fetchContacts();
      return true;
    },
        onErrorMessage: L10nLookup.current.msgErrNodesDatImport,
        fallback: false);
  }

  Future<void> clearContacts() async {
    try {
      final count = await client.clearKadContacts();
      onStatus?.call(L10nLookup.current.msgKadContactsCleared(count));
      await fetchStatus();
      await fetchContacts();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrKadClear(e));
    }
  }

  Future<bool> startUdp({String? bindAddress}) {
    return _runLoading(() async {
      await client.startKadUdp(bindAddress: bindAddress);
      onStatus?.call(
        L10nLookup.current
            .msgKadUdpStarted(bindAddress ?? '127.0.0.1:39737'),
      );
      await fetchStatus();
      return true;
    },
        onErrorMessage: L10nLookup.current.msgErrKadUdpStart,
        fallback: false);
  }

  Future<bool> stopUdp() {
    return _runLoading(() async {
      await client.stopKadUdp();
      onStatus?.call(L10nLookup.current.msgKadUdpStopped);
      await fetchStatus();
      return true;
    },
        onErrorMessage: L10nLookup.current.msgErrKadUdpStop,
        fallback: false);
  }

  Future<int?> pingContact({
    required String ip,
    required int port,
    String? kadId,
  }) async {
    try {
      final res = await client.pingKadContact(ip: ip, port: port, kadId: kadId);
      final latency = (res['latencyMs'] as num?)?.toInt() ?? 0;
      onStatus?.call(
        L10nLookup.current.msgKadPong('$ip:$port', latency),
      );
      await fetchStatus();
      await fetchContacts();
      return latency;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrKadPing('$ip:$port', e));
      return null;
    }
  }

  Future<bool> bootstrap({required String ip, required int port}) {
    return _runLoading(() async {
      final res = await client.bootstrapKad(ip: ip, port: port);
      final count = (res['receivedContacts'] as num?)?.toInt() ?? 0;
      onStatus?.call(L10nLookup.current.msgKadBootstrapOk(count));
      await fetchStatus();
      await fetchContacts();
      return true;
    },
        onErrorMessage: L10nLookup.current.msgErrKadBootstrap,
        fallback: false);
  }

  Future<bool> bootstrapFromUrl(String url) {
    return _runLoading(() async {
      final res = await client.bootstrapKadFromUrl(url);
      final count = (res['importedNodes'] as num?)?.toInt() ?? 0;
      onStatus?.call(L10nLookup.current.msgKadBootstrapUrlOk(count));
      await fetchStatus();
      await fetchContacts();
      return true;
    },
        onErrorMessage: L10nLookup.current.msgErrKadBootstrapUrl,
        fallback: false);
  }

  Future<List<KadContactModel>> findNode(String targetId) {
    return _runLoading(() async {
      final contacts = await client.findKadNode(targetId);
      onStatus?.call(
        L10nLookup.current.msgKadSearchFound(contacts.length),
      );
      await fetchStatus();
      await fetchContacts();
      return contacts;
    },
        onErrorMessage: L10nLookup.current.msgErrKadSearch,
        fallback: const []);
  }

  /// Searches Kad DHT for sources of a specific file hash and auto-injects them.
  Future<List<PeerSourceModel>> searchSources(
    String fileHash, {
    int? fileSize,
  }) {
    return _runLoading(() async {
      final sources = await client.searchKadSources(
        fileHash: fileHash,
        fileSize: fileSize,
      );
      onStatus?.call(
        L10nLookup.current.msgKadSourcesFound(
          sources.length,
          fileHash.substring(0, 8),
        ),
      );
      return sources;
    },
        onErrorMessage: L10nLookup.current.msgErrKadSources,
        fallback: const []);
  }

  /// Publishes this node as a source for a file hash to Kad DHT.
  Future<int> publishSource(
    String fileHash,
    int fileSize, {
    int tcpPort = 39735,
  }) {
    return _runLoading(() async {
      final count = await client.publishKadSource(
        fileHash: fileHash,
        fileSize: fileSize,
        tcpPort: tcpPort,
      );
      onStatus?.call(
        L10nLookup.current
            .msgKadPublished(count, fileHash.substring(0, 8)),
      );
      return count;
    },
        onErrorMessage: L10nLookup.current.msgErrKadPublish,
        fallback: 0);
  }

  /// Runs [action] under the loading flag; on error reports
  /// `"$onErrorMessage : $e"` via [onError] and returns [fallback].
  Future<T> _runLoading<T>(
    Future<T> Function() action, {
    required String onErrorMessage,
    T? fallback,
  }) async {
    _isLoading = true;
    notifyListeners();
    try {
      return await action();
    } catch (e) {
      onError?.call('$onErrorMessage : $e');
      notifyListeners();
      return fallback as T;
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _kadSub?.cancel();
    _connSub?.cancel();
    super.dispose();
  }
}
