// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

import '../../../../l10n/l10n_lookup.dart';

/// Sub-domain controller for the eD2K servers section.
///
/// Owns the server list, the loading flag and the live server session
/// (driven by `client.serverSessionEvents`). Status and error reporting is
/// delegated to the host via [onStatus]/[onError].
class ServersController extends ChangeNotifier {
  final MuleDaemonClient client;

  /// Called with a transient status message (shown in the shell banner).
  final void Function(String message)? onStatus;

  /// Called with an error message on failed RPC calls.
  final void Function(String message)? onError;

  ServersController({required this.client, this.onStatus, this.onError}) {
    _sessionSub = client.serverSessionEvents.listen((session) {
      _session = session;
      if (session.status == ServerConnectionStatus.connected ||
          session.status == ServerConnectionStatus.disconnected) {
        _isConnecting = false;
      } else if (session.status == ServerConnectionStatus.connecting) {
        _isConnecting = true;
      }
      notifyListeners();
    });
  }

  List<Ed2kServerModel> _servers = [];
  bool _isLoading = false;
  ServerSessionModel _session = ServerSessionModel.disconnected;
  bool _isConnecting = false;
  StreamSubscription<ServerSessionModel>? _sessionSub;

  List<Ed2kServerModel> get servers => _servers;
  bool get isLoading => _isLoading;
  ServerSessionModel get session => _session;
  bool get isConnecting => _isConnecting;

  /// Force the session back to disconnected (kill switch / full reset).
  void markDisconnected() {
    _session = ServerSessionModel.disconnected;
    notifyListeners();
  }

  Future<void> fetch() async {
    _isLoading = true;
    notifyListeners();
    try {
      _servers = await client.listServers();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrListServers(e));
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<ServerInspectReport?> inspectServerMet(String filePath) async {
    try {
      return await client.inspectServerMet(filePath);
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrInspectServerMet(e));
      notifyListeners();
      return null;
    }
  }

  Future<bool> importServerMet(String filePath) async {
    try {
      final result = await client.importServerMet(filePath);
      onStatus?.call(
        L10nLookup.current.msgServersImported(
          result.allowedCount,
          result.blockedCount,
          result.imported,
        ),
      );
      await fetch();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrImportServerMet(e));
      notifyListeners();
      return false;
    }
  }

  Future<Map<String, dynamic>?> updateFromUrl(String url) async {
    _isLoading = true;
    notifyListeners();
    try {
      final result = await client.updateServersFromUrl(url);
      final count = result['serversCount'] ?? result['imported'] ?? 0;
      onStatus?.call(L10nLookup.current.msgServersUpdated(count));
      await fetch();
      return result;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrServersUpdate(e));
      notifyListeners();
      return null;
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<void> setEnabled(String address, bool enabled) async {
    try {
      await client.setServerEnabled(address, enabled);
      await fetch();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrToggleServer(e));
      notifyListeners();
    }
  }

  Future<void> delete(String address) async {
    try {
      await client.deleteServer(address);
      await fetch();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrDeleteServer(e));
      notifyListeners();
    }
  }

  Future<void> clear() async {
    try {
      final count = await client.clearServers();
      onStatus?.call(L10nLookup.current.msgServersCleared(count));
      await fetch();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrClearServers(e));
      notifyListeners();
    }
  }

  Future<void> connect(String address) async {
    if (_isConnecting) return;
    _isConnecting = true;
    notifyListeners();
    try {
      _session = await client.connectServer(address);
      onStatus?.call(
        L10nLookup.current
            .msgConnectingToServer(_session.serverName ?? address),
      );
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrConnectServer(e));
      _session = ServerSessionModel.disconnected;
      notifyListeners();
    } finally {
      _isConnecting = false;
      notifyListeners();
    }
  }

  Future<void> disconnect() async {
    try {
      _session = await client.disconnectServer();
      onStatus?.call(L10nLookup.current.msgDisconnectedFromServer);
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrDisconnectServer(e));
      notifyListeners();
    }
  }

  Future<void> refreshSession() async {
    try {
      _session = await client.getServerStatus();
      notifyListeners();
    } catch (_) {}
  }

  @override
  void dispose() {
    _sessionSub?.cancel();
    super.dispose();
  }
}
