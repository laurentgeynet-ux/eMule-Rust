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
      File('${tempDir.path}/auth.token').writeAsStringSync('wrong_token');

      final client = RpcDaemonClient();
      final supervisor = DaemonSupervisor(
        client: client,
        customDataDir: tempDir,
        customExecutablePath: exePath,
      );

      await supervisor.start();

      expect(supervisor.state, SupervisorState.authError);
      expect(supervisor.lastError, contains('rejected authentication'));

      await supervisor.stop();
      supervisor.dispose();
      await server.close(force: true);
    });
  });
}
