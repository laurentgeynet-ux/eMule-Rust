// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:async';
import 'dart:convert';

import 'package:web_socket_channel/web_socket_channel.dart';

import '../domain/models/assistant_models.dart';

/// Client JSON-RPC/WebSocket du transport `mule-assistant`.
///
/// Protocole (miroir de `crates/mule-assistant/src/ws.rs`) :
/// - `assistant.hello {token}` — authentification par le jeton de
///   session du daemon, obligatoire avant tout autre appel ;
/// - `assistant.chat {text}` → `{accepted: true}` puis notifications
///   `assistant.event`, `assistant.confirmRequired`, `assistant.turnEnd` ;
/// - `assistant.confirm {allow}` → résout la confirmation en attente ;
/// - `assistant.cancel` → annule le tour en cours.
///
/// Une seule connexion par instance ; un seul tour à la fois (le serveur
/// rejette un second `chat` tant que le tour courant n'a pas reçu
/// `turnEnd`).
class AssistantClient {
  WebSocketChannel? _channel;
  StreamSubscription<dynamic>? _subscription;
  int _nextId = 1;
  AssistantConnState _state = AssistantConnState.disconnected;

  final Map<int, Completer<dynamic>> _pendingRequests = {};
  final StreamController<AssistantConnState> _stateController =
      StreamController<AssistantConnState>.broadcast();
  final StreamController<AssistantEvent> _eventsController =
      StreamController<AssistantEvent>.broadcast();
  final StreamController<AssistantConfirmRequest> _confirmController =
      StreamController<AssistantConfirmRequest>.broadcast();
  final StreamController<String?> _turnEndController =
      StreamController<String?>.broadcast();

  /// Injection de canal pour les tests (faux serveur en loopback).
  final WebSocketChannel Function(Uri uri)? channelFactory;

  AssistantClient({this.channelFactory});

  AssistantConnState get state => _state;
  Stream<AssistantConnState> get stateStream => _stateController.stream;
  Stream<AssistantEvent> get events => _eventsController.stream;
  Stream<AssistantConfirmRequest> get confirmRequests =>
      _confirmController.stream;

  /// Fin d'un tour : `null` si succès, message d'erreur sinon.
  Stream<String?> get turnEnd => _turnEndController.stream;

  /// Ouvre la connexion et exécute `assistant.hello`. Renvoie les infos
  /// serveur (`server`, `version`, `tools`) ou lève une erreur
  /// (jeton invalide, transport indisponible).
  Future<Map<String, dynamic>> connect(Uri url, String token) async {
    disconnect();
    _setState(AssistantConnState.connecting);
    try {
      final factory = channelFactory;
      final channel =
          factory != null ? factory(url) : WebSocketChannel.connect(url);
      _channel = channel;
      // `ready` complète aussi en erreur sur refus de connexion — la
      // fermeture est déjà gérée via le stream ; absorber le doublon
      // sinon il remonte comme erreur async non gérée.
      channel.ready.catchError((_) {});
      _subscription = channel.stream.listen(
        _onMessage,
        onError: (_) => _onClosed(),
        onDone: _onClosed,
      );
      final hello = await _request('assistant.hello', {'token': token});
      _setState(AssistantConnState.connected);
      return hello as Map<String, dynamic>;
    } catch (e) {
      _setState(AssistantConnState.error);
      rethrow;
    }
  }

  /// Lance un tour utilisateur. Les résultats arrivent en notifications.
  Future<void> chat(String text) => _request('assistant.chat', {'text': text});

  /// Répond à la confirmation d'outil en attente.
  Future<void> confirm(bool allow) =>
      _request('assistant.confirm', {'allow': allow});

  /// Annule le tour en cours.
  Future<void> cancel() => _request('assistant.cancel', const {});

  void disconnect() {
    _subscription?.cancel();
    _subscription = null;
    _channel?.sink.close();
    _channel = null;
    for (final completer in _pendingRequests.values) {
      if (!completer.isCompleted) {
        completer.completeError(
          StateError('assistant disconnected'),
        );
      }
    }
    _pendingRequests.clear();
    _setState(AssistantConnState.disconnected);
  }

  void dispose() {
    disconnect();
    _stateController.close();
    _eventsController.close();
    _confirmController.close();
    _turnEndController.close();
  }

  Future<dynamic> _request(String method, Map<String, dynamic> params) {
    final channel = _channel;
    if (channel == null) {
      return Future.error(StateError('assistant non connecté'));
    }
    final id = _nextId++;
    final completer = Completer<dynamic>();
    _pendingRequests[id] = completer;
    channel.sink.add(jsonEncode({
      'jsonrpc': '2.0',
      'id': id,
      'method': method,
      'params': params,
    }));
    return completer.future;
  }

  void _onMessage(dynamic raw) {
    final Map<String, dynamic> msg;
    try {
      msg = jsonDecode(raw as String) as Map<String, dynamic>;
    } catch (_) {
      return;
    }
    if (msg.containsKey('id')) {
      final id = msg['id'];
      if (id is! int) return;
      final completer = _pendingRequests.remove(id);
      if (completer == null) return;
      final error = msg['error'] as Map<String, dynamic>?;
      if (error != null) {
        completer.completeError(
          StateError(error['message']?.toString() ?? 'erreur assistant'),
        );
      } else {
        completer.complete(msg['result']);
      }
      return;
    }
    final params = msg['params'];
    switch (msg['method']) {
      case 'assistant.event':
        if (params is Map<String, dynamic>) {
          _eventsController.add(AssistantEvent.fromJson(params));
        }
      case 'assistant.confirmRequired':
        if (params is Map<String, dynamic>) {
          _confirmController.add(AssistantConfirmRequest.fromJson(params));
        }
      case 'assistant.turnEnd':
        final error = (params as Map<String, dynamic>?)?['error'];
        _turnEndController.add(error as String?);
    }
  }

  void _onClosed() {
    if (_state == AssistantConnState.disconnected) return;
    _subscription?.cancel();
    _subscription = null;
    _channel = null;
    for (final completer in _pendingRequests.values) {
      if (!completer.isCompleted) {
        completer.completeError(
          StateError('assistant disconnected'),
        );
      }
    }
    _pendingRequests.clear();
    _setState(AssistantConnState.error);
  }

  void _setState(AssistantConnState next) {
    if (_state == next) return;
    _state = next;
    _stateController.add(next);
  }
}
