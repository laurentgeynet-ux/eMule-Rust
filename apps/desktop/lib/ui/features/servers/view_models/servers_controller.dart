import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

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
      onError?.call('Failed to list servers: $e');
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<ServerInspectReport?> inspectServerMet(String filePath) async {
    try {
      return await client.inspectServerMet(filePath);
    } catch (e) {
      onError?.call('Failed to inspect server.met: $e');
      notifyListeners();
      return null;
    }
  }

  Future<bool> importServerMet(String filePath) async {
    try {
      final result = await client.importServerMet(filePath);
      onStatus?.call(
        'Imported ${result.imported} servers (${result.allowedCount} allowed, '
        '${result.blockedCount} blocked by filter)',
      );
      await fetch();
      return true;
    } catch (e) {
      onError?.call('Failed to import server.met: $e');
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
      onStatus?.call(
        'Mise à jour réussie : $count serveurs traités depuis l\'URL',
      );
      await fetch();
      return result;
    } catch (e) {
      onError?.call('Échec de la mise à jour des serveurs depuis l\'URL : $e');
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
      onError?.call('Failed to toggle server state: $e');
      notifyListeners();
    }
  }

  Future<void> delete(String address) async {
    try {
      await client.deleteServer(address);
      await fetch();
    } catch (e) {
      onError?.call('Failed to delete server: $e');
      notifyListeners();
    }
  }

  Future<void> clear() async {
    try {
      final count = await client.clearServers();
      onStatus?.call('Cleared $count servers');
      await fetch();
    } catch (e) {
      onError?.call('Failed to clear servers: $e');
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
        'Connexion au serveur ${_session.serverName ?? address}...',
      );
      notifyListeners();
    } catch (e) {
      onError?.call('Connexion impossible : $e');
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
      onStatus?.call('Déconnecté du serveur eD2K');
      notifyListeners();
    } catch (e) {
      onError?.call('Erreur de déconnexion : $e');
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
