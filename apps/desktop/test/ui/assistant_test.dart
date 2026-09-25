// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:app_shell/l10n/generated/app_localizations.dart';
import 'package:app_shell/ui/features/assistant/view_models/assistant_controller.dart';
import 'package:app_shell/ui/features/assistant/views/assistant_view.dart';
import 'package:app_shell/app/providers.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';

/// Faux serveur `mule-assistant` loopback rejouant des scénarios
/// scriptés — même protocole que `ws.rs`.
class FakeAssistantServer {
  late HttpServer server;
  WebSocket? socket;
  final requests = <Map<String, dynamic>>[];
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
              send(ws, {
                'id': id,
                'result': {
                  'server': 'mule-assistant',
                  'version': '0.1.0',
                  'tools': 10,
                },
              });
            } else {
              send(ws, {
                'id': id,
                'error': {'code': -32001, 'message': 'jeton invalide'},
              });
            }
          case 'assistant.chat':
            send(ws, {'id': id, 'result': {'accepted': true}});
            if (_chatScripts.isNotEmpty) {
              unawaited(_chatScripts.removeAt(0)(ws));
            }
          case 'assistant.confirm':
            send(ws, {'id': id, 'result': {'confirmed': true}});
          case 'assistant.cancel':
            send(ws, {'id': id, 'result': {'cancelled': true}});
            send(ws, {
              'method': 'assistant.turnEnd',
              'params': {'error': null},
            });
        }
      });
    });
    return Uri.parse('ws://127.0.0.1:${server.port}/assistant');
  }

  void onChat(Future<void> Function(WebSocket) script) =>
      _chatScripts.add(script);

  /// Écriture tolérante : la socket peut être fermée par un cancel.
  void send(WebSocket ws, Map<String, dynamic> msg) {
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
  late Uri endpoint;
  late AssistantController ctl;

  AssistantController makeCtl({String? token = 'good-token'}) =>
      AssistantController(
        tokenResolver: () async => token,
        endpoint: endpoint,
      );

  setUp(() async {
    fake = FakeAssistantServer();
    endpoint = await fake.start();
    ctl = makeCtl();
  });

  tearDown(() async {
    ctl.dispose();
    await fake.stop();
  });

  test('jeton absent → lastError renseigné, pas de connexion', () async {
    final offline = makeCtl(token: null);
    await offline.connect();
    expect(offline.state, AssistantConnState.disconnected);
    expect(offline.lastError, isNotNull);
    offline.dispose();
  });

  test('connect échoue sur endpoint fermé → lastError + état error',
      () async {
    final closed = AssistantController(
      tokenResolver: () async => 'good-token',
      endpoint: Uri.parse('ws://127.0.0.1:1/assistant'),
    );
    await closed.connect();
    expect(closed.state, AssistantConnState.error);
    expect(closed.lastError, isNotNull);
    closed.dispose();
  });

  test('tour simple : entrée user + tokens + réponse finale', () async {
    fake.onChat((ws) async {
      fake.send(ws, {
        'method': 'assistant.event',
        'params': {'type': 'token', 'content': 'Bon'},
      });
      fake.send(ws, {
        'method': 'assistant.event',
        'params': {'type': 'final', 'content': 'Bonjour'},
      });
      fake.send(ws, {
        'method': 'assistant.turnEnd',
        'params': {'error': null},
      });
    });
    await ctl.connect();
    expect(ctl.isConnected, isTrue);
    expect(ctl.visibleTools, 10);

    await ctl.send('salut');
    expect(ctl.entries.single.kind, ChatEntryKind.user);

    await Future.delayed(const Duration(milliseconds: 150));
    expect(ctl.turnActive, isFalse);
    expect(ctl.entries.last.kind, ChatEntryKind.assistant);
    expect(ctl.entries.last.text, 'Bonjour');
  });

  test('events outils → entrées toolCall puis toolResult', () async {
    fake.onChat((ws) async {
      fake.send(ws, {
        'method': 'assistant.event',
        'params': {
          'type': 'toolStart',
          'name': 'emule_status',
          'args': <String, dynamic>{},
        },
      });
      fake.send(ws, {
        'method': 'assistant.event',
        'params': {
          'type': 'toolResult',
          'name': 'emule_status',
          'is_error': false,
          'content': '{"ok":true}',
        },
      });
      fake.send(ws, {
        'method': 'assistant.event',
        'params': {'type': 'final', 'content': 'Tout va bien.'},
      });
      fake.send(ws, {
        'method': 'assistant.turnEnd',
        'params': {'error': null},
      });
    });
    await ctl.connect();
    await ctl.send('status ?');
    await Future.delayed(const Duration(milliseconds: 150));

    expect(ctl.entries.map((e) => e.kind), [
      ChatEntryKind.user,
      ChatEntryKind.toolCall,
      ChatEntryKind.toolResult,
      ChatEntryKind.assistant,
    ]);
    expect(ctl.entries[1].toolName, 'emule_status');
    expect(ctl.entries[2].isError, isFalse);
  });

  test('confirmRequired → pendingConfirm puis confirm(true)', () async {
    fake.onChat((ws) async {
      fake.send(ws, {
        'method': 'assistant.confirmRequired',
        'params': {
          'name': 'emule_delete_server',
          'args': {'address': 'ed2k://1.2.3.4:4661'},
        },
      });
      await Future.delayed(const Duration(milliseconds: 120));
      fake.send(ws, {
        'method': 'assistant.turnEnd',
        'params': {'error': null},
      });
    });
    await ctl.connect();
    await ctl.send('supprime le serveur');
    await Future.delayed(const Duration(milliseconds: 60));

    expect(ctl.pendingConfirm, isNotNull);
    expect(ctl.pendingConfirm!.toolName, 'emule_delete_server');

    await ctl.confirm(true);
    expect(ctl.pendingConfirm, isNull);
    await Future.delayed(const Duration(milliseconds: 150));
    expect(
      fake.requests.map((m) => m['method']),
      contains('assistant.confirm'),
    );
  });

  test('cancel : turnActive retombe au turnEnd', () async {
    fake.onChat((ws) async {
      // Tour sans fin — seul le cancel libère.
    });
    await ctl.connect();
    await ctl.send('tâche longue');
    expect(ctl.turnActive, isTrue);

    await ctl.cancel();
    await Future.delayed(const Duration(milliseconds: 100));
    expect(ctl.turnActive, isFalse);
    expect(
      fake.requests.map((m) => m['method']),
      contains('assistant.cancel'),
    );
  });

  testWidgets('AssistantView : affiche le chat et envoie un message',
      (tester) async {
    fake.onChat((ws) async {
      fake.send(ws, {
        'method': 'assistant.event',
        'params': {'type': 'final', 'content': 'Réponse test'},
      });
      fake.send(ws, {
        'method': 'assistant.turnEnd',
        'params': {'error': null},
      });
    });
    // WS réel : la connexion doit sortir de la zone FakeAsync — les
    // timers internes du canal y deviendraient des timers factices
    // périodiques et bloqueraient la fin du test.
    await tester.runAsync(() => ctl.connect());

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          assistantControllerProvider
              .overrideWith((ref) => ctl, disposeNotifier: false),
        ],
        child: MaterialApp(
          locale: const Locale('fr'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: const Scaffold(body: AssistantView()),
        ),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));

    expect(find.text('Assistant local'), findsOneWidget);
    expect(find.textContaining('Connecté'), findsOneWidget);

    await tester.enterText(find.byType(TextField), 'bonjour');
    await tester.tap(find.byIcon(Icons.send_rounded));
    await tester.pump();
    // Attente réelle : les événements arrivent via la socket réelle.
    await tester
        .runAsync(() => Future.delayed(const Duration(milliseconds: 150)));
    await tester.pump();

    expect(find.text('bonjour'), findsOneWidget);
    expect(find.text('Réponse test'), findsOneWidget);
  });

  testWidgets('AssistantView hors-ligne : état indisponible + retry',
      (tester) async {
    final offline = makeCtl(token: null);
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          assistantControllerProvider
              .overrideWith((ref) => offline, disposeNotifier: false),
        ],
        child: MaterialApp(
          locale: const Locale('fr'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: const Scaffold(body: AssistantView()),
        ),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));

    expect(find.text('Assistant indisponible'), findsOneWidget);
    expect(find.text('Réessayer'), findsOneWidget);
    offline.dispose();
  });
}
