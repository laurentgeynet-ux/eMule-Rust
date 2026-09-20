import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

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
      onError?.call('Échec de la récupération des contacts Kad : $e');
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<NodesDatInspectReportModel?> inspectNodesDat(String filePath) async {
    try {
      return await client.inspectNodesDat(filePath);
    } catch (e) {
      onError?.call('Échec de l\'inspection de nodes.dat : $e');
      return null;
    }
  }

  Future<bool> importNodesDat(String filePath) async {
    return _runLoading(() async {
      final res = await client.importNodesDat(filePath);
      final count = res['importedCount'] ?? 0;
      onStatus?.call('Importé $count contacts Kad dans la table K-buckets');
      await fetchStatus();
      await fetchContacts();
      return true;
    }, onErrorMessage: 'Échec de l\'importation de nodes.dat', fallback: false);
  }

  Future<void> clearContacts() async {
    try {
      final count = await client.clearKadContacts();
      onStatus?.call('Effacé $count contacts Kad');
      await fetchStatus();
      await fetchContacts();
    } catch (e) {
      onError?.call('Échec de l\'effacement des contacts Kad : $e');
    }
  }

  Future<bool> startUdp({String? bindAddress}) {
    return _runLoading(() async {
      await client.startKadUdp(bindAddress: bindAddress);
      onStatus?.call(
        'Socket UDP Kad démarré sur ${bindAddress ?? "127.0.0.1:39737"}',
      );
      await fetchStatus();
      return true;
    }, onErrorMessage: 'Échec du démarrage UDP Kad', fallback: false);
  }

  Future<bool> stopUdp() {
    return _runLoading(() async {
      await client.stopKadUdp();
      onStatus?.call('Socket UDP Kad arrêté');
      await fetchStatus();
      return true;
    }, onErrorMessage: 'Échec de l\'arrêt UDP Kad', fallback: false);
  }

  Future<int?> pingContact({
    required String ip,
    required int port,
    String? kadId,
  }) async {
    try {
      final res = await client.pingKadContact(ip: ip, port: port, kadId: kadId);
      final latency = (res['latencyMs'] as num?)?.toInt() ?? 0;
      onStatus?.call('Pong reçu de $ip:$port en ${latency}ms');
      await fetchStatus();
      await fetchContacts();
      return latency;
    } catch (e) {
      onError?.call('Échec du ping Kad ($ip:$port) : $e');
      return null;
    }
  }

  Future<bool> bootstrap({required String ip, required int port}) {
    return _runLoading(() async {
      final res = await client.bootstrapKad(ip: ip, port: port);
      final count = (res['receivedContacts'] as num?)?.toInt() ?? 0;
      onStatus?.call('Amorçage Kad réussi ($count contacts reçus)');
      await fetchStatus();
      await fetchContacts();
      return true;
    }, onErrorMessage: 'Échec de l\'amorçage Kad', fallback: false);
  }

  Future<bool> bootstrapFromUrl(String url) {
    return _runLoading(() async {
      final res = await client.bootstrapKadFromUrl(url);
      final count = (res['importedNodes'] as num?)?.toInt() ?? 0;
      onStatus?.call(
        'Amorçage Kad depuis URL réussi ($count nœuds importés)',
      );
      await fetchStatus();
      await fetchContacts();
      return true;
    }, onErrorMessage: 'Échec de l\'amorçage Kad depuis URL', fallback: false);
  }

  Future<List<KadContactModel>> findNode(String targetId) {
    return _runLoading(() async {
      final contacts = await client.findKadNode(targetId);
      onStatus?.call(
        'Recherche Kad : ${contacts.length} nœuds les plus proches trouvés',
      );
      await fetchStatus();
      await fetchContacts();
      return contacts;
    }, onErrorMessage: 'Échec de la recherche Kad', fallback: const []);
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
        'Kad : ${sources.length} sources trouvées pour '
        '${fileHash.substring(0, 8)}…',
      );
      return sources;
    }, onErrorMessage: 'Échec de la recherche de sources Kad', fallback: const []);
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
        'Kad : publié sur $count nœuds pour ${fileHash.substring(0, 8)}…',
      );
      return count;
    }, onErrorMessage: 'Échec de la publication Kad', fallback: 0);
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
