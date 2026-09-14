import 'dart:async';
import 'dart:convert';
import 'dart:io';

import '../data/rpc_daemon_client.dart';
import '../domain/models/daemon_info.dart';
import 'work_directory_resolver.dart';

enum SupervisorState {
  idle,
  discovering,
  starting,
  connected,
  reconnecting,
  protocolMismatch,
  authError,
  error;

  String get label {
    switch (this) {
      case SupervisorState.idle:
        return 'Idle';
      case SupervisorState.discovering:
        return 'Discovering Daemon...';
      case SupervisorState.starting:
        return 'Starting Daemon...';
      case SupervisorState.connected:
        return 'Daemon Connected';
      case SupervisorState.reconnecting:
        return 'Reconnecting...';
      case SupervisorState.protocolMismatch:
        return 'Protocol Version Incompatible';
      case SupervisorState.authError:
        return 'Authentication Failed';
      case SupervisorState.error:
        return 'Daemon Error';
    }
  }

  bool get isConnected => this == SupervisorState.connected;
}

class DaemonSessionFile {
  final int protocolVersion;
  final String instanceId;
  final int pid;
  final String endpoint;
  final int startedAtUnixMs;

  const DaemonSessionFile({
    required this.protocolVersion,
    required this.instanceId,
    required this.pid,
    required this.endpoint,
    required this.startedAtUnixMs,
  });

  factory DaemonSessionFile.fromJson(Map<String, dynamic> json) {
    return DaemonSessionFile(
      protocolVersion: (json['protocolVersion'] as num?)?.toInt() ?? 0,
      instanceId: json['instanceId'] as String? ?? '',
      pid: (json['pid'] as num?)?.toInt() ?? 0,
      endpoint: json['endpoint'] as String? ?? '',
      startedAtUnixMs: (json['startedAtUnixMs'] as num?)?.toInt() ?? 0,
    );
  }
}

class DaemonSupervisor {
  final RpcDaemonClient client;
  final Directory? customDataDir;
  final String? customExecutablePath;

  SupervisorState _state = SupervisorState.idle;
  String? _lastError;
  String? _currentInstanceId;
  int? _activePid;
  Process? _managedProcess;
  Timer? _reconnectTimer;
  int _reconnectAttempts = 0;
  bool _isDisposed = false;

  final StreamController<SupervisorState> _stateController =
      StreamController<SupervisorState>.broadcast();
  final StreamController<void> _instanceChangedController =
      StreamController<void>.broadcast();

  DaemonSupervisor({
    required this.client,
    this.customDataDir,
    this.customExecutablePath,
  });

  SupervisorState get state => _state;
  String? get lastError => _lastError;
  String? get currentInstanceId => _currentInstanceId;
  int? get activePid => _activePid ?? _managedProcess?.pid;
  Stream<SupervisorState> get stateStream => _stateController.stream;
  Stream<void> get instanceChangedStream => _instanceChangedController.stream;

  Directory resolveDataDir() {
    return WorkDirectoryResolver.resolve(
      customDir: customDataDir,
      executableFile: customExecutablePath != null ? File(customExecutablePath!) : null,
    ).directory;
  }

  void _setState(SupervisorState newState, [String? error]) {
    _state = newState;
    _lastError = error;
    if (!_stateController.isClosed) {
      _stateController.add(newState);
    }
  }

  final StringBuffer _stderrBuffer = StringBuffer();
  int? _lastExitCode;

  Future<void> start() async {
    _reconnectAttempts = 0;
    _reconnectTimer?.cancel();
    await _connectOrLaunch();
  }

  Future<void> _connectOrLaunch() async {
    if (_isDisposed) return;
    _setState(SupervisorState.discovering);

    final dataDir = resolveDataDir();
    DaemonSessionFile? session = await _readSessionFileAsync(dataDir);
    String? token = _readTokenFile(dataDir);

    final DaemonSessionFile? initialSession = session;
    final String? staleInstanceId = session?.instanceId;
    // 1. Try to connect to existing daemon if session file exists
    if (session != null) {
      final connected = await _tryConnectToSession(session, token);
      if (connected) return;
      if (_state == SupervisorState.protocolMismatch || _state == SupervisorState.authError) {
        return;
      }

      // Existing session was unreachable.
      // Anti-race guard: Re-read session.json to ensure it hasn't changed before deleting it.
      final currentSession = await _readSessionFileAsync(dataDir, retries: 1);
      if (currentSession != null) {
        final isSame = currentSession.instanceId == initialSession!.instanceId &&
            currentSession.pid == initialSession.pid &&
            currentSession.endpoint == initialSession.endpoint &&
            currentSession.startedAtUnixMs == initialSession.startedAtUnixMs;

        if (isSame) {
          try {
            final f = File('${dataDir.path}/session.json');
            if (f.existsSync()) f.deleteSync();
          } catch (_) {}
        } else {
          // A new session was published in the meantime! Restart discovery instead of deleting.
          _setState(SupervisorState.discovering);
          await _connectOrLaunch();
          return;
        }
      }
    }

    // 2. If daemon is absent or unreachable, attempt auto-launch
    _setState(SupervisorState.starting);
    final launched = await _launchDaemon(dataDir);
    if (!launched) {
      _setState(SupervisorState.error, _lastError ?? 'Failed to locate or start mule-daemon executable.');
      return;
    }

    // 3. Wait for session file to be written by the launched process or a concurrent daemon
    session = await _waitForSessionFile(
      dataDir,
      ignoreInstanceId: staleInstanceId,
      timeout: const Duration(seconds: 10),
    );
    if (session == null) {
      final errText = _stderrBuffer.toString().trim();
      if (_lastExitCode != null) {
        _setState(
          SupervisorState.error,
          'mule-daemon exited prematurely with code $_lastExitCode${errText.isNotEmpty ? ": $errText" : ""}',
        );
      } else {
        _setState(SupervisorState.error, 'Timeout waiting for mule-daemon session.json initialization.');
      }
      return;
    }

    // If our launched process exited because another daemon won the lock race, clear handle
    if (_lastExitCode != null) {
      _managedProcess = null;
    }

    token = _readTokenFile(dataDir);
    final connected = await _tryConnectToSession(session, token);
    if (!connected) {
      _scheduleReconnect();
    }
  }

  Future<DaemonSessionFile?> _readSessionFileAsync(
    Directory dataDir, {
    int retries = 5,
    Duration retryDelay = const Duration(milliseconds: 60),
  }) async {
    final sessionFile = File('${dataDir.path}/session.json');
    final tmpFile = File('${dataDir.path}/session.json.tmp');

    for (int i = 0; i < retries; i++) {
      if (!sessionFile.existsSync()) {
        // If .tmp file is present, a write is actively in progress
        if (tmpFile.existsSync() && i < retries - 1) {
          await Future.delayed(retryDelay);
          continue;
        }
        return null;
      }

      try {
        final content = (await sessionFile.readAsString()).trim();
        if (content.isEmpty) {
          if (i < retries - 1) {
            await Future.delayed(retryDelay);
            continue;
          }
          return null;
        }
        final json = jsonDecode(content) as Map<String, dynamic>;
        if (json.containsKey('protocolVersion') &&
            json.containsKey('instanceId') &&
            json.containsKey('endpoint')) {
          return DaemonSessionFile.fromJson(json);
        }
      } catch (_) {
        // Sharing violation, file locked by atomic rename, or JSON parse error during write
        if (i < retries - 1) {
          await Future.delayed(retryDelay);
          continue;
        }
      }
    }
    return null;
  }

  String? _readTokenFile(Directory dataDir) {
    try {
      final file = File('${dataDir.path}/auth.token');
      if (file.existsSync()) {
        final content = file.readAsStringSync().trim();
        if (content.length == 64 && RegExp(r'^[0-9a-fA-F]{64}$').hasMatch(content)) {
          return content;
        }
      }
    } catch (_) {}
    return null;
  }

  Future<bool> _tryConnectToSession(DaemonSessionFile session, String? token) async {
    // Check protocol version before connecting
    if (session.protocolVersion != 1) {
      _setState(
        SupervisorState.protocolMismatch,
        'Protocol version mismatch: daemon requires version ${session.protocolVersion}, client supports version 1.',
      );
      return false;
    }

    try {
      final uri = Uri.parse(session.endpoint);
      final info = await client.connect(url: uri, token: token);

      final isFirstConnect = _currentInstanceId == null;
      final instanceChanged = !isFirstConnect && _currentInstanceId != info.instanceId;
      _currentInstanceId = info.instanceId;
      _activePid = session.pid;

      _setState(SupervisorState.connected);
      _reconnectAttempts = 0;

      if (instanceChanged && !_instanceChangedController.isClosed) {
        _instanceChangedController.add(null);
      }

      // Monitor connection loss to initiate backoff reconnect
      _monitorConnection();
      return true;
    } catch (e) {
      final str = e.toString();
      if (str.contains('Authentication failed') || str.contains('-32000')) {
        _setState(SupervisorState.authError, 'Daemon rejected authentication token.');
        return false;
      }
      _lastError = 'Connection failed to ${session.endpoint}';
      return false;
    }
  }

  void _monitorConnection() {
    late StreamSubscription sub;
    sub = client.connectionState.listen((connState) {
      if (connState == DaemonConnectionState.disconnected ||
          connState == DaemonConnectionState.error) {
        sub.cancel();
        if (!_isDisposed && _state == SupervisorState.connected) {
          _scheduleReconnect();
        }
      }
    });
  }

  void _scheduleReconnect() {
    if (_isDisposed) return;
    if (_state == SupervisorState.protocolMismatch || _state == SupervisorState.authError) {
      return; // Do not enter infinite reconnect loop on permanent fatal errors
    }

    _setState(SupervisorState.reconnecting);
    _reconnectAttempts++;

    // Exponential backoff: 500ms, 1s, 2s, 4s, max 10s
    final delayMs = (_reconnectAttempts <= 4)
        ? (500 * (1 << (_reconnectAttempts - 1)))
        : 10000;

    _reconnectTimer?.cancel();
    _reconnectTimer = Timer(Duration(milliseconds: delayMs), () async {
      await _connectOrLaunch();
    });
  }

  Future<bool> _launchDaemon(Directory dataDir) async {
    final exePath = _findExecutable();
    if (exePath == null) {
      _lastError = 'mule-daemon executable not found. Please compile or install mule-daemon.';
      return false;
    }

    try {
      dataDir.createSync(recursive: true);
      _stderrBuffer.clear();
      _lastExitCode = null;

      final process = await Process.start(exePath, [
        '--workdir',
        dataDir.path,
        '--parent-pid',
        pid.toString(),
      ]);
      _managedProcess = process;
      _activePid = process.pid;

      process.stderr.transform(utf8.decoder).listen((data) {
        _stderrBuffer.write(data);
      });

      process.exitCode.then((exitCode) {
        _lastExitCode = exitCode;
        if (!_isDisposed && _state == SupervisorState.connected) {
          _scheduleReconnect();
        }
      });

      return true;
    } catch (e) {
      _lastError = 'Failed to launch $exePath: $e';
      return false;
    }
  }

  String? _findExecutable() {
    if (customExecutablePath != null && File(customExecutablePath!).existsSync()) {
      return customExecutablePath!;
    }

    final exeName = Platform.isWindows ? 'mule-daemon.exe' : 'mule-daemon';

    // 1. Check relative to current process executable (packaged bundle layout)
    final appDir = File(Platform.resolvedExecutable).parent;
    final candidatePaths = [
      '${appDir.path}/$exeName',
      '${appDir.path}/bin/$exeName',
      // 2. Check development build locations
      '../../target/release/$exeName',
      '../../target/debug/$exeName',
      '../target/release/$exeName',
      '../target/debug/$exeName',
      'target/release/$exeName',
      'target/debug/$exeName',
    ];

    for (final candidate in candidatePaths) {
      final f = File(candidate);
      if (f.existsSync()) return f.path;
    }

    return null;
  }

  Future<DaemonSessionFile?> _waitForSessionFile(
    Directory dataDir, {
    String? ignoreInstanceId,
    required Duration timeout,
  }) async {
    final stopwatch = Stopwatch()..start();
    while (stopwatch.elapsed < timeout) {
      final session = await _readSessionFileAsync(dataDir, retries: 2, retryDelay: const Duration(milliseconds: 50));
      if (session != null && (ignoreInstanceId == null || session.instanceId != ignoreInstanceId)) {
        return session;
      }
      await Future.delayed(const Duration(milliseconds: 150));
    }
    return null;
  }

  Future<void> stop() async {
    _isDisposed = true;
    _reconnectTimer?.cancel();
    _reconnectTimer = null;

    // 1. Send graceful shutdown RPC if connected
    if (client.currentConnectionState == DaemonConnectionState.connected) {
      try {
        await client.stopDaemon().timeout(const Duration(milliseconds: 800));
      } catch (_) {}
    }

    // 2. Wait for managed process to exit or kill it
    if (_managedProcess != null) {
      try {
        await _managedProcess!.exitCode.timeout(const Duration(milliseconds: 1000));
      } catch (_) {
        _managedProcess!.kill();
        try {
          await _managedProcess!.exitCode.timeout(const Duration(milliseconds: 500));
        } catch (_) {}
      }
      _managedProcess = null;
    }
    _activePid = null;

    // 3. Disconnect client channel
    await client.disconnect();

    _setState(SupervisorState.idle);
  }

  void dispose() {
    stop();
    _stateController.close();
    _instanceChangedController.close();
  }
}
