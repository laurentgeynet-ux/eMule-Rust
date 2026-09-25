// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n_lookup.dart';

/// Endpoint WebSocket par défaut de `mule-assistant serve`
/// (miroir de `DEFAULT_WS_PORT` côté Rust — loopback uniquement).
final Uri defaultAssistantEndpoint =
    Uri.parse('ws://127.0.0.1:4680/assistant');

/// Sub-domain controller for the local-AI assistant section.
///
/// Owns the connection to `mule-assistant` (WebSocket JSON-RPC), the
/// visible conversation ([entries]), the in-flight streamed text
/// ([streamingText]) and the pending tool confirmation. The assistant is
/// a desktop feature: the session token is read from the daemon workdir
/// via [tokenResolver] (`auth.token`), the same secret as the RPC layer.
class AssistantController extends ChangeNotifier {
  /// Resolves the daemon session token (`null` → feature unavailable).
  final Future<String?> Function() tokenResolver;

  /// Assistant WS endpoint — injectable for tests
  /// (`null` → [defaultAssistantEndpoint]).
  final Uri? endpoint;

  /// Client factory — injectable for tests.
  final AssistantClient Function()? clientFactory;

  /// Called with a transient status message (shown in the shell banner).
  final void Function(String message)? onStatus;

  /// Called with an error message on failed operations.
  final void Function(String message)? onError;

  AssistantController({
    required this.tokenResolver,
    this.endpoint,
    this.clientFactory,
    this.onStatus,
    this.onError,
  }) : assert(
          endpoint == null ||
              endpoint.host == '127.0.0.1' ||
              endpoint.host == '::1',
          'assistant endpoint must be loopback',
        );

  Uri get _endpoint => endpoint ?? defaultAssistantEndpoint;

  AssistantClient? _client;
  StreamSubscription<AssistantEvent>? _eventsSub;
  StreamSubscription<AssistantConfirmRequest>? _confirmSub;
  StreamSubscription<String?>? _turnEndSub;
  StreamSubscription<AssistantConnState>? _stateSub;

  final List<ChatEntry> _entries = [];
  String _streamingText = '';
  AssistantConnState _state = AssistantConnState.disconnected;
  bool _turnActive = false;
  AssistantConfirmRequest? _pendingConfirm;
  int _visibleTools = 0;
  String? _lastError;

  List<ChatEntry> get entries => List.unmodifiable(_entries);
  String get streamingText => _streamingText;
  AssistantConnState get state => _state;
  bool get turnActive => _turnActive;
  AssistantConfirmRequest? get pendingConfirm => _pendingConfirm;
  int get visibleTools => _visibleTools;
  String? get lastError => _lastError;
  bool get isConnected => _state == AssistantConnState.connected;

  /// Connecte au serveur assistant (idempotent). Le jeton vient du
  /// workdir du daemon — `null` signifie fonction indisponible (web ou
  /// daemon externe sans accès fichier).
  Future<void> connect() async {
    if (isConnected || _state == AssistantConnState.connecting) return;
    final token = await tokenResolver();
    if (token == null) {
      _lastError = L10nLookup.current.assistantTokenUnavailable;
      notifyListeners();
      return;
    }
    final client = (clientFactory ?? AssistantClient.new)();
    _client = client;
    _wireSubscriptions(client);
    try {
      final info = await client.connect(_endpoint, token);
      _visibleTools = info['tools'] as int? ?? 0;
      _lastError = null;
    } catch (e) {
      _lastError = L10nLookup.current.assistantConnectError(e.toString());
      AppLogger.warn('assistant', 'Connection failed', error: e);
    }
    // Le stateStream (broadcast) livre ses événements en microtâches :
    // synchroniser l'état visible avant de notifier, sinon l'appelant
    // observe encore `connecting` au retour de `connect()`.
    _state = client.state;
    notifyListeners();
  }

  /// Envoie un message utilisateur — les réponses arrivent par events.
  Future<void> send(String text) async {
    final trimmed = text.trim();
    if (trimmed.isEmpty || _turnActive) return;
    final client = _client;
    if (client == null || !isConnected) {
      await connect();
      if (!isConnected) return;
    }
    _entries.add(ChatEntry(kind: ChatEntryKind.user, text: trimmed));
    _streamingText = '';
    _turnActive = true;
    _lastError = null;
    notifyListeners();
    try {
      await client!.chat(trimmed);
    } catch (e) {
      _turnActive = false;
      _entries.add(ChatEntry(
        kind: ChatEntryKind.error,
        text: e.toString(),
        isError: true,
      ));
      notifyListeners();
    }
  }

  /// Répond à la demande de confirmation d'outil en attente.
  Future<void> confirm(bool allow) async {
    if (_pendingConfirm == null) return;
    _pendingConfirm = null;
    notifyListeners();
    try {
      await _client?.confirm(allow);
    } catch (e) {
      AppLogger.warn('assistant', 'confirm failed', error: e);
    }
  }

  /// Annule le tour en cours.
  Future<void> cancel() async {
    try {
      await _client?.cancel();
    } catch (e) {
      AppLogger.warn('assistant', 'cancel failed', error: e);
    }
  }

  void _wireSubscriptions(AssistantClient client) {
    _eventsSub?.cancel();
    _confirmSub?.cancel();
    _turnEndSub?.cancel();
    _stateSub?.cancel();
    _eventsSub = client.events.listen(_onEvent);
    _confirmSub = client.confirmRequests.listen((req) {
      _pendingConfirm = req;
      notifyListeners();
    });
    _turnEndSub = client.turnEnd.listen(_onTurnEnd);
    _stateSub = client.stateStream.listen((s) {
      _state = s;
      if (s == AssistantConnState.error || s == AssistantConnState.disconnected) {
        _turnActive = false;
        _pendingConfirm = null;
      }
      notifyListeners();
    });
  }

  void _onEvent(AssistantEvent ev) {
    switch (ev.type) {
      case AssistantEventType.token:
        _streamingText += ev.content ?? '';
      case AssistantEventType.toolStart:
        _flushStreaming();
        _entries.add(ChatEntry(
          kind: ChatEntryKind.toolCall,
          text: _formatArgs(ev.args),
          toolName: ev.toolName,
        ));
      case AssistantEventType.toolResult:
        _entries.add(ChatEntry(
          kind: ChatEntryKind.toolResult,
          text: ev.content ?? '',
          toolName: ev.toolName,
          isError: ev.isError,
        ));
      case AssistantEventType.finalText:
        _entries.add(ChatEntry(
          kind: ChatEntryKind.assistant,
          text: ev.content ?? '',
        ));
        _streamingText = '';
      case AssistantEventType.notice:
        _entries.add(ChatEntry(
          kind: ChatEntryKind.notice,
          text: ev.content ?? '',
        ));
      case AssistantEventType.error:
        _entries.add(ChatEntry(
          kind: ChatEntryKind.error,
          text: ev.content ?? 'erreur',
          isError: true,
        ));
    }
    notifyListeners();
  }

  void _onTurnEnd(String? error) {
    _turnActive = false;
    _pendingConfirm = null;
    _flushStreaming();
    if (error != null) {
      _entries.add(ChatEntry(
        kind: ChatEntryKind.error,
        text: error,
        isError: true,
      ));
    }
    notifyListeners();
  }

  /// Les tokens restants deviennent une entrée si le tour s'achève
  /// sans `final` (annulation, erreur backend).
  void _flushStreaming() {
    if (_streamingText.isEmpty) return;
    _entries.add(ChatEntry(
      kind: ChatEntryKind.assistant,
      text: _streamingText,
    ));
    _streamingText = '';
  }

  String _formatArgs(Map<String, dynamic>? args) =>
      args == null || args.isEmpty ? '{}' : args.toString();

  @override
  void dispose() {
    _eventsSub?.cancel();
    _confirmSub?.cancel();
    _turnEndSub?.cancel();
    _stateSub?.cancel();
    _client?.dispose();
    super.dispose();
  }
}
