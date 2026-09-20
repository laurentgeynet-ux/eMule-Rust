import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';

void main() {
  group('RpcDaemonClient Tests with local WebSocket server', () {
    late HttpServer server;
    late int port;
    late StreamController<dynamic> serverMessages;
    WebSocket? serverWs;

    setUp(() async {
      serverMessages = StreamController<dynamic>.broadcast();
      server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      port = server.port;

      server.transform(WebSocketTransformer()).listen((WebSocket ws) {
        serverWs = ws;
        ws.listen((data) {
          final decoded = jsonDecode(data.toString()) as Map<String, dynamic>;
          serverMessages.add(decoded);

          final id = decoded['id'];
          final method = decoded['method'];
          final params = decoded['params'];

          // Echo or reply based on method
          if (method == 'system.hello') {
            final token = params['token'];
            if (token == 'invalid') {
              ws.add(jsonEncode({
                'jsonrpc': '2.0',
                'id': id,
                'error': {'code': -32000, 'message': 'Auth failed'},
              }));
            } else {
              ws.add(jsonEncode({
                'jsonrpc': '2.0',
                'id': id,
                'result': {
                  'daemonVersion': '0.1.3',
                  'protocolVersion': 1,
                  'instanceId': 'mock-inst',
                  'sessionId': 'mock-sess',
                  'capabilities': ['downloads', 'download-events'],
                },
              }));
            }
          } else if (method == 'events.subscribe') {
            ws.add(jsonEncode({
              'jsonrpc': '2.0',
              'id': id,
              'result': {'subscribed': true},
            }));
          } else if (method == 'downloads.list') {
            ws.add(jsonEncode({
              'jsonrpc': '2.0',
              'id': id,
              'result': {
                'sequence': 5,
                'downloads': [
                  {
                    'id': 'dl-100',
                    'fileName': 'movie.mkv',
                    'totalBytes': 1000000,
                    'receivedBytes': 200000,
                    'verifiedBytes': 200000,
                    'progress': 0.2,
                    'status': 'downloading',
                    'priority': 'normal',
                    'sourceCount': 6,
                    'downloadRateBps': 50000,
                    'uploadedBytes': 0,
                    'createdAtUnixMs': 1726000000000,
                    'updatedAtUnixMs': 1726000001000,
                  }
                ],
              },
            }));
          } else if (method == 'downloads.pause') {
            ws.add(jsonEncode({
              'jsonrpc': '2.0',
              'id': id,
              'result': {
                'id': params['id'],
                'fileName': 'movie.mkv',
                'totalBytes': 1000000,
                'receivedBytes': 200000,
                'verifiedBytes': 200000,
                'progress': 0.2,
                'status': 'paused',
                'priority': 'normal',
                'sourceCount': 6,
                'downloadRateBps': 0,
                'uploadedBytes': 0,
                'createdAtUnixMs': 1726000000000,
                'updatedAtUnixMs': 1726000002000,
              },
            }));
          }
        });
      });
    });

    tearDown(() async {
      await serverWs?.close();
      await server.close(force: true);
      await serverMessages.close();
    });

    test('successful handshake and subscription', () async {
      final client = RpcDaemonClient();
      final info = await client.connect(
        url: Uri.parse('ws://127.0.0.1:$port/rpc'),
        token: 'valid_token',
      );

      expect(client.currentConnectionState, DaemonConnectionState.connected);
      expect(info.version, '0.1.3');
      expect(info.instanceId, 'mock-inst');
      expect(client.lastSequence, 0);

      client.dispose();
    });

    test('failed handshake triggers error state and RpcException', () async {
      final client = RpcDaemonClient();

      await expectLater(
        client.connect(
          url: Uri.parse('ws://127.0.0.1:$port/rpc'),
          token: 'invalid',
        ),
        throwsA(isA<RpcException>()),
      );

      expect(client.currentConnectionState, DaemonConnectionState.disconnected);

      client.dispose();
    });

    test('listDownloads updates lastSequence and parses download items', () async {
      final client = RpcDaemonClient();
      await client.connect(
        url: Uri.parse('ws://127.0.0.1:$port/rpc'),
        token: 'valid',
      );

      final list = await client.listDownloads();
      expect(client.lastSequence, 5);
      expect(list.length, 1);
      expect(list[0].id, 'dl-100');
      expect(list[0].fileName, 'movie.mkv');
      expect(list[0].status, DownloadStatus.downloading);

      client.dispose();
    });

    test('event notifications are streamed and stale sequence numbers ignored', () async {
      final client = RpcDaemonClient();
      await client.connect(
        url: Uri.parse('ws://127.0.0.1:$port/rpc'),
        token: 'valid',
      );

      await client.listDownloads(); // sequence becomes 5

      final List<int> receivedSequences = [];
      final sub = client.downloadEvents.listen((event) {
        receivedSequences.add(event.sequence);
      });

      // 1. Send stale event (seq 4 <= 5) -> Should be ignored
      serverWs!.add(jsonEncode({
        'jsonrpc': '2.0',
        'method': 'event.downloadUpdated',
        'params': {
          'sequence': 4,
          'download': {
            'id': 'dl-100',
            'fileName': 'movie.mkv',
            'totalBytes': 1000000,
            'receivedBytes': 250000,
            'verifiedBytes': 250000,
            'progress': 0.25,
            'status': 'downloading',
            'priority': 'normal',
            'sourceCount': 6,
            'downloadRateBps': 50000,
            'uploadedBytes': 0,
            'createdAtUnixMs': 1726000000000,
            'updatedAtUnixMs': 1726000003000,
          },
        },
      }));

      // 2. Send fresh event (seq 6 > 5) -> Should be processed
      serverWs!.add(jsonEncode({
        'jsonrpc': '2.0',
        'method': 'event.downloadUpdated',
        'params': {
          'sequence': 6,
          'download': {
            'id': 'dl-100',
            'fileName': 'movie.mkv',
            'totalBytes': 1000000,
            'receivedBytes': 300000,
            'verifiedBytes': 300000,
            'progress': 0.3,
            'status': 'downloading',
            'priority': 'normal',
            'sourceCount': 6,
            'downloadRateBps': 60000,
            'uploadedBytes': 0,
            'createdAtUnixMs': 1726000000000,
            'updatedAtUnixMs': 1726000004000,
          },
        },
      }));

      // Allow event processing
      await Future.delayed(const Duration(milliseconds: 100));

      expect(receivedSequences, equals([6]));
      expect(client.lastSequence, 6);

      await sub.cancel();
      client.dispose();
    });

    test('pauseDownload calls RPC method and returns updated item', () async {
      final client = RpcDaemonClient();
      await client.connect(
        url: Uri.parse('ws://127.0.0.1:$port/rpc'),
        token: 'valid',
      );

      final updated = await client.pauseDownload('dl-100');
      expect(updated.status, DownloadStatus.paused);
      expect(updated.downloadRateBps, 0);

      client.dispose();
    });
  });
}
