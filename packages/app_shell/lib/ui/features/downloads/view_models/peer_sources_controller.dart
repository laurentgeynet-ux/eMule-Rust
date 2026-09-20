import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

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
        '${rawSources.length} source(s) trouvée(s) pour $hash',
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

  Future<void> loadSourcesForHash(String fileHash) async {
    _isLoading = true;
    notifyListeners();
    try {
      _sources[fileHash] = await client.listSources(fileHash);
    } catch (e) {
      onError?.call('Échec du chargement des sources : $e');
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
      onStatus?.call('Source $address ajoutée');
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call('Échec de l\'ajout de la source : $e');
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
      onStatus?.call('Connexion à $address initialisée');
      await loadSourcesForHash(fileHash);
      return true;
    } catch (e) {
      onError?.call('Échec de la connexion à la source : $e');
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
            ? 'Peer $address déconnecté'
            : 'Session(s) peer déconnectée(s)',
      );
      notifyListeners();
    } catch (e) {
      onError?.call('Échec de la déconnexion peer : $e');
      notifyListeners();
    }
  }

  Future<void> clearSourcesForHash(String fileHash) async {
    try {
      await client.clearSources(fileHash);
      _sources.remove(fileHash);
      onStatus?.call('Sources effacées');
      notifyListeners();
    } catch (e) {
      onError?.call('Échec de l\'effacement des sources : $e');
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
