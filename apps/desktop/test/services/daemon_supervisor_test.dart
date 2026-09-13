import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:desktop/data/rpc_daemon_client.dart';
import 'package:desktop/services/daemon_supervisor.dart';

void main() {
  group('DaemonSupervisor Tests', () {
    late Directory tempDir;
    late String exePath;

    setUp(() async {
      tempDir = await Directory.systemTemp.createTemp('supervisor_test_');
      exePath = Platform.isWindows
          ? '..\\..\\target\\debug\\mule-daemon.exe'
          : '../../target/debug/mule-daemon';
    });

    tearDown(() async {
      try {
        await tempDir.delete(recursive: true);
      } catch (_) {}
    });

    test('supervisor auto-starts daemon when absent and connects', () async {
      final client = RpcDaemonClient();
      final supervisor = DaemonSupervisor(
        client: client,
        customDataDir: tempDir,
        customExecutablePath: exePath,
      );

      final stateHistory = <SupervisorState>[];
      final sub = supervisor.stateStream.listen(stateHistory.add);

      await supervisor.start();

      // Should transition from discovering -> starting -> connected
      expect(supervisor.state, SupervisorState.connected);
      expect(supervisor.currentInstanceId, isNotNull);

      // Verify session and auth files were created
      expect(File('${tempDir.path}/session.json').existsSync(), isTrue);
      expect(File('${tempDir.path}/auth.token').existsSync(), isTrue);

      await sub.cancel();
      await supervisor.stop();
      supervisor.dispose();
    });

    test('supervisor connects to already running daemon without duplicate', () async {
      // 1. Start first supervisor/daemon
      final client1 = RpcDaemonClient();
      final supervisor1 = DaemonSupervisor(
        client: client1,
        customDataDir: tempDir,
        customExecutablePath: exePath,
      );
      await supervisor1.start();
      expect(supervisor1.state, SupervisorState.connected);

      // 2. Second supervisor should reuse existing session
      final client2 = RpcDaemonClient();
      final supervisor2 = DaemonSupervisor(
        client: client2,
        customDataDir: tempDir,
        customExecutablePath: exePath,
      );

      await supervisor2.start();
      expect(supervisor2.state, SupervisorState.connected);
      expect(supervisor2.currentInstanceId, supervisor1.currentInstanceId);

      await supervisor2.stop();
      supervisor2.dispose();

      await supervisor1.stop();
      supervisor1.dispose();
    });

    test('incompatible protocol version halts with protocolMismatch state', () async {
      // Manually write an incompatible session.json
      final fakeSession = {
        'protocolVersion': 99,
        'instanceId': '00000000-0000-0000-0000-000000000099',
        'pid': 12345,
        'endpoint': 'ws://127.0.0.1:4660/rpc',
        'startedAtUnixMs': 1726000000000,
      };
      File('${tempDir.path}/session.json').writeAsStringSync(jsonEncode(fakeSession));

      final client = RpcDaemonClient();
      final supervisor = DaemonSupervisor(
        client: client,
        customDataDir: tempDir,
        customExecutablePath: exePath,
      );

      await supervisor.start();

      expect(supervisor.state, SupervisorState.protocolMismatch);
      expect(supervisor.lastError, contains('Protocol version mismatch'));

      await supervisor.stop();
      supervisor.dispose();
    });

    test('invalid authentication token stops with authError state', () async {
      // Bind a mock server that always rejects token
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      final port = server.port;

      server.transform(WebSocketTransformer()).listen((WebSocket ws) {
        ws.listen((data) {
          final req = jsonDecode(data.toString());
          ws.add(jsonEncode({
            'jsonrpc': '2.0',
            'id': req['id'],
            'error': {'code': -32000, 'message': 'Authentication failed: invalid token'},
          }));
        });
      });

      // Write session file pointing to our mock server
      final session = {
        'protocolVersion': 1,
        'instanceId': '00000000-0000-0000-0000-000000000001',
        'pid': 1111,
        'endpoint': 'ws://127.0.0.1:$port/rpc',
        'startedAtUnixMs': 1726000000000,
      };
      File('${tempDir.path}/session.json').writeAsStringSync(jsonEncode(session));
      const testWrongToken = 'bad0000000000000000000000000000000000000000000000000000000000bad';
      File('${tempDir.path}/auth.token').writeAsStringSync(testWrongToken);

      final client = RpcDaemonClient();
      final supervisor = DaemonSupervisor(
        client: client,
        customDataDir: tempDir,
        customExecutablePath: exePath,
      );

      await supervisor.start();

      expect(supervisor.state, SupervisorState.authError);
      expect(supervisor.lastError, contains('rejected authentication'));
      // Guarantee that secret token is never exposed in error message
      expect(supervisor.lastError, isNot(contains(testWrongToken)));

      await supervisor.stop();
      supervisor.dispose();
      await server.close(force: true);
    });

    test('supervisor handles startup race condition: two instances starting concurrently', () async {
      final client1 = RpcDaemonClient();
      final supervisor1 = DaemonSupervisor(
        client: client1,
        customDataDir: tempDir,
        customExecutablePath: exePath,
      );

      final client2 = RpcDaemonClient();
      final supervisor2 = DaemonSupervisor(
        client: client2,
        customDataDir: tempDir,
        customExecutablePath: exePath,
      );

      // Start both concurrently
      await Future.wait([
        supervisor1.start(),
        supervisor2.start(),
      ]);

      expect(supervisor1.state, SupervisorState.connected);
      expect(supervisor2.state, SupervisorState.connected);
      expect(supervisor1.currentInstanceId, isNotNull);
      expect(supervisor2.currentInstanceId, supervisor1.currentInstanceId);

      await supervisor2.stop();
      supervisor2.dispose();

      await supervisor1.stop();
      supervisor1.dispose();
    });

    test('supervisor retries on partially written or temporary session.json', () async {
      // Mock server to accept connection
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      final port = server.port;
      server.transform(WebSocketTransformer()).listen((WebSocket ws) {
        ws.listen((data) {
          final req = jsonDecode(data.toString());
          if (req['method'] == 'system.hello') {
            ws.add(jsonEncode({
              'jsonrpc': '2.0',
              'id': req['id'],
              'result': {
                'instanceId': '00000000-0000-0000-0000-000000000002',
                'protocolVersion': 1,
                'daemonVersion': '0.1.0',
                'startedAtUnixMs': 1726000000000,
                'activeDownloads': 0,
                'completedDownloads': 0,
              },
            }));
          } else if (req['method'] == 'events.subscribe') {
            ws.add(jsonEncode({
              'jsonrpc': '2.0',
              'id': req['id'],
              'result': {'subscribed': true},
            }));
          }
        });
      });

      // Write empty session file initially
      final sessionFile = File('${tempDir.path}/session.json');
      sessionFile.writeAsStringSync('');
      File('${tempDir.path}/auth.token').writeAsStringSync('0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef');

      // Schedule proper session file write after 120ms
      Timer(const Duration(milliseconds: 120), () {
        sessionFile.writeAsStringSync(jsonEncode({
          'protocolVersion': 1,
          'instanceId': '00000000-0000-0000-0000-000000000002',
          'pid': 2222,
          'endpoint': 'ws://127.0.0.1:$port/rpc',
          'startedAtUnixMs': 1726000000000,
        }));
      });

      final client = RpcDaemonClient();
      final supervisor = DaemonSupervisor(
        client: client,
        customDataDir: tempDir,
        customExecutablePath: exePath,
      );

      await supervisor.start();
      expect(supervisor.state, SupervisorState.connected);
      expect(supervisor.currentInstanceId, '00000000-0000-0000-0000-000000000002');

      await supervisor.stop();
      supervisor.dispose();
      await server.close(force: true);
    });

    test('supervisor recovers from stale session by launching fresh daemon', () async {
      // Write stale session with dead port
      final staleSession = {
        'protocolVersion': 1,
        'instanceId': 'dead-0000-0000-0000-000000000000',
        'pid': 999999,
        'endpoint': 'ws://127.0.0.1:59998/rpc',
        'startedAtUnixMs': 1726000000000,
      };
      File('${tempDir.path}/session.json').writeAsStringSync(jsonEncode(staleSession));

      final client = RpcDaemonClient();
      final supervisor = DaemonSupervisor(
        client: client,
        customDataDir: tempDir,
        customExecutablePath: exePath,
      );

      await supervisor.start();

      // Dead endpoint should be abandoned and new daemon started
      expect(supervisor.state, SupervisorState.connected);
      expect(supervisor.currentInstanceId, isNot('dead-0000-0000-0000-000000000000'));

      await supervisor.stop();
      supervisor.dispose();
    });
  });
}
