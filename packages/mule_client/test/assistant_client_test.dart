// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';

/// Faux serveur `mule-assistant` en loopback : répond au protocole
/// JSON-RPC de `crates/mule-assistant/src/ws.rs` et rejoue des scénarios
/// scriptés (tokens, outils, confirmation, annulation).
class FakeAssistantServer {
  late HttpServer server;
  WebSocket? socket;
  final requests = <Map<String, dynamic>>[];

  /// File de scénarios à rejouer à la réception de `assistant.chat`.
  final _chatScripts = <Future<void> Function(WebSocket)>[];

  Future<Uri> start() async {
    server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    server.transform(WebSocketTransformer()).listen((ws) {
      socket = ws;
      ws.listen((data) async {
        final msg = jsonDecode(data.toString()) as Map<String, dynamic>;
        requests.add(msg);
        final id = msg['id'];
        switch (msg['method']) {
          case 'assistant.hello':
            final token = (msg['params'] as Map)['token'];
            if (token == 'good-token') {
              _safeSend(ws, {
                'id': id,
                'result': {'server': 'mule-assistant', 'version': '0.1.0', 'tools': 10},
              });
            } else {
              _safeSend(ws, {
                'id': id,
                'error': {'code': -32001, 'message': 'jeton invalide'},
              });
            }
          case 'assistant.chat':
            _safeSend(ws, {'id': id, 'result': {'accepted': true}});
            if (_chatScripts.isNotEmpty) {
              unawaited(_chatScripts.removeAt(0)(ws));
            }
          case 'assistant.confirm':
            final allow = (msg['params'] as Map)['allow'] == true;
            _safeSend(ws, {'id': id, 'result': {'confirmed': allow}});
          case 'assistant.cancel':
            _safeSend(ws, {'id': id, 'result': {'cancelled': true}});
            unawaited(Future(() {
              final s = socket;
              if (s != null) {
                _safeSend(s, {
                  'method': 'assistant.turnEnd',
                  'params': {'error': null},
                });
              }
            }));
        }
      });
    });
    return Uri.parse('ws://127.0.0.1:${server.port}/assistant');
  }

  /// Programme le scénario exécuté à la prochaine réception de `chat`.
  void onChat(Future<void> Function(WebSocket) script) =>
      _chatScripts.add(script);

  /// Écriture tolérante : la socket peut être déjà fermée quand un
  /// scénario ou une réponse tardive s'exécute.
  void _safeSend(WebSocket ws, Map<String, dynamic> msg) {
    try {
      ws.add(jsonEncode(msg));
    } catch (_) {}
  }

  Future<void> stop() async {
    await socket?.close();
    await server.close(force: true);
  }
}

void main() {
  late FakeAssistantServer fake;
  late AssistantClient client;
  late Uri endpoint;

  Future<void> connectOk() async {
    await client.connect(endpoint, 'good-token');
    expect(client.state, AssistantConnState.connected);
  }

  setUp(() async {
    fake = FakeAssistantServer();
    endpoint = await fake.start();
    client = AssistantClient();
  });

  tearDown(() async {
    client.dispose();
    await fake.stop();
  });

  test('hello accepte le bon jeton et expose les infos serveur', () async {
    final info = await client.connect(endpoint, 'good-token');
    expect(info['server'], 'mule-assistant');
    expect(info['tools'], 10);
    expect(client.state, AssistantConnState.connected);
  });

  test('hello refuse un mauvais jeton', () async {
    await expectLater(
      client.connect(endpoint, 'bad-token'),
      throwsA(isA<StateError>()),
    );
    expect(client.state, AssistantConnState.error);
  });

  test('chat : réponse finale simple (tokens + final + turnEnd)', () async {
    fake.onChat((ws) async {
      fake._safeSend(ws, {
        'method': 'assistant.event',
        'params': {'type': 'token', 'content': 'Bon'},
      });
      fake._safeSend(ws, {
        'method': 'assistant.event',
        'params': {'type': 'token', 'content': 'jour'},
      });
      fake._safeSend(ws, {
        'method': 'assistant.event',
        'params': {'type': 'final', 'content': 'Bonjour'},
      });
      fake._safeSend(ws, {
        'method': 'assistant.turnEnd',
        'params': {'error': null},
      });
    });
    await connectOk();
    final events = <AssistantEvent>[];
    final ends = <String?>[];
    final sub1 = client.events.listen(events.add);
    final sub2 = client.turnEnd.listen(ends.add);

    await client.chat('salut');
    await Future.delayed(const Duration(milliseconds: 100));

    expect(events.map((e) => e.type), [
      AssistantEventType.token,
      AssistantEventType.token,
      AssistantEventType.finalText,
    ]);
    expect(events[0].content, 'Bon');
    expect(events[2].content, 'Bonjour');
    expect(ends, [null]);
    await sub1.cancel();
    await sub2.cancel();
  });

  test('events outils : toolStart puis toolResult avec is_error', () async {
    fake.onChat((ws) async {
      fake._safeSend(ws, {
        'method': 'assistant.event',
        'params': {
          'type': 'toolStart',
          'name': 'emule_status',
          'args': {},
        },
      });
      fake._safeSend(ws, {
        'method': 'assistant.event',
        'params': {
          'type': 'toolResult',
          'name': 'emule_status',
          'is_error': false,
          'content': '{"ok":true}',
        },
      });
      fake._safeSend(ws, {
        'method': 'assistant.turnEnd',
        'params': {'error': null},
      });
    });
    await connectOk();
    final events = <AssistantEvent>[];
    final sub = client.events.listen(events.add);
    await client.chat('status ?');
    await Future.delayed(const Duration(milliseconds: 100));

    expect(events[0].type, AssistantEventType.toolStart);
    expect(events[0].toolName, 'emule_status');
    expect(events[1].type, AssistantEventType.toolResult);
    expect(events[1].isError, isFalse);
    expect(events[1].content, contains('ok'));
    await sub.cancel();
  });

  test('confirmRequired → confirm(true) résout la confirmation', () async {
    fake.onChat((ws) async {
      fake._safeSend(ws, {
        'method': 'assistant.confirmRequired',
        'params': {
          'name': 'emule_delete_server',
          'args': {'address': 'ed2k://1.2.3.4:4661'},
        },
      });
      // Le serveur attend la confirmation puis termine le tour.
      await Future.delayed(const Duration(milliseconds: 150));
      fake._safeSend(ws, {
        'method': 'assistant.turnEnd',
        'params': {'error': null},
      });
    });
    await connectOk();
    final confirms = <AssistantConfirmRequest>[];
    final sub = client.confirmRequests.listen(confirms.add);

    await client.chat('supprime le serveur');
    await Future.delayed(const Duration(milliseconds: 60));
    expect(confirms, hasLength(1));
    expect(confirms.single.toolName, 'emule_delete_server');

    await client.confirm(true);
    await Future.delayed(const Duration(milliseconds: 200));
    final confirmReq = fake.requests.lastWhere(
      (m) => m['method'] == 'assistant.confirm',
    );
    expect((confirmReq['params'] as Map)['allow'], isTrue);
    await sub.cancel();
  });

  test('cancel envoie assistant.cancel et reçoit turnEnd', () async {
    fake.onChat((ws) async {
      // Tour long sans événements — le client annule avant la fin.
    });
    await connectOk();
    final ends = <String?>[];
    final sub = client.turnEnd.listen(ends.add);
    await client.chat('tâche longue');
    await client.cancel();
    await Future.delayed(const Duration(milliseconds: 100));

    expect(fake.requests.map((m) => m['method']),
        containsAll(['assistant.chat', 'assistant.cancel']));
    expect(ends, [null]);
    await sub.cancel();
  });

  test('fermeture serveur : état error et requêtes pendantes échouent',
      () async {
    await connectOk();
    final chatFuture = client.chat('ping');
    // Attacher l'expectation avant la fermeture : l'erreur arrive vite.
    final expectation = expectLater(chatFuture, throwsA(anything));
    await fake.socket?.close();
    await expectation;
    expect(client.state, AssistantConnState.error);
  });
}
