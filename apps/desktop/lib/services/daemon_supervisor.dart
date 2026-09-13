import 'dart:async';
import 'dart:convert';
import 'dart:io';

import '../data/rpc_daemon_client.dart';
import '../domain/models/daemon_info.dart';

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
  Stream<SupervisorState> get stateStream => _stateController.stream;
  Stream<void> get instanceChangedStream => _instanceChangedController.stream;

  Directory resolveDataDir() {
    if (customDataDir != null) return customDataDir!;

    final envData = Platform.environment['MULE_DATA_DIR'];
    if (envData != null && envData.trim().isNotEmpty) {
      return Directory(envData.trim());
    }

    if (Platform.isWindows) {
      final appData = Platform.environment['APPDATA'];
      if (appData != null && appData.isNotEmpty) {
        return Directory('$appData\\EmuleSion');
      }
    } else {
      final home = Platform.environment['HOME'];
      if (home != null && home.isNotEmpty) {
        return Directory('$home/.emule_sion');
      }
    }

    return Directory('.mule_data');
  }

  void _setState(SupervisorState newState, [String? error]) {
    _state = newState;
    _lastError = error;
    if (!_stateController.isClosed) {
      _stateController.add(newState);
    }
  }

  Future<void> start() async {
    _reconnectAttempts = 0;
    _reconnectTimer?.cancel();
    await _connectOrLaunch();
  }

  Future<void> _connectOrLaunch() async {
    if (_isDisposed) return;
    _setState(SupervisorState.discovering);

    final dataDir = resolveDataDir();
    DaemonSessionFile? session = _readSessionFile(dataDir);
    String? token = _readTokenFile(dataDir);

    // 1. Try to connect to existing daemon if session file exists
    if (session != null) {
      final connected = await _tryConnectToSession(session, token);
      if (connected) return;
    }

    // 2. If daemon is absent or unreachable, attempt auto-launch
    _setState(SupervisorState.starting);
    final launched = await _launchDaemon(dataDir);
    if (!launched) {
      _setState(SupervisorState.error, _lastError ?? 'Failed to locate or start mule-daemon executable.');
      return;
    }

    // 3. Wait for session file to be written by the launched process
    session = await _waitForSessionFile(dataDir, timeout: const Duration(seconds: 10));
    if (session == null) {
      _setState(SupervisorState.error, 'Timeout waiting for mule-daemon session.json initialization.');
      return;
    }

    token = _readTokenFile(dataDir);
    final connected = await _tryConnectToSession(session, token);
    if (!connected) {
      _scheduleReconnect();
    }
  }

  DaemonSessionFile? _readSessionFile(Directory dataDir) {
    try {
      final file = File('${dataDir.path}/session.json');
      if (file.existsSync()) {
        final content = file.readAsStringSync();
        final json = jsonDecode(content) as Map<String, dynamic>;
        return DaemonSessionFile.fromJson(json);
      }
    } catch (_) {}
    return null;
  }

  String? _readTokenFile(Directory dataDir) {
    try {
      final file = File('${dataDir.path}/auth.token');
      if (file.existsSync()) {
        final content = file.readAsStringSync().trim();
        if (content.isNotEmpty) return content;
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
      _managedProcess = await Process.start(exePath, [
        '--workdir',
        dataDir.path,
      ]);

      _managedProcess!.exitCode.then((exitCode) {
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

    // 1. Check relative to current process executable (bundle layout)
    final appDir = File(Platform.resolvedExecutable).parent;
    final inAppDir = File('${appDir.path}/$exeName');
    if (inAppDir.existsSync()) return inAppDir.path;

    final inBinDir = File('${appDir.path}/bin/$exeName');
    if (inBinDir.existsSync()) return inBinDir.path;

    // 2. Check development build locations
    final devRelease = File('../../target/release/$exeName');
    if (devRelease.existsSync()) return devRelease.path;

    final devDebug = File('../../target/debug/$exeName');
    if (devDebug.existsSync()) return devDebug.path;

    return null;
  }

  Future<DaemonSessionFile?> _waitForSessionFile(Directory dataDir, {required Duration timeout}) async {
    final stopwatch = Stopwatch()..start();
    while (stopwatch.elapsed < timeout) {
      final session = _readSessionFile(dataDir);
      if (session != null) return session;
      await Future.delayed(const Duration(milliseconds: 150));
    }
    return null;
  }

  Future<void> stop() async {
    _isDisposed = true;
    _reconnectTimer?.cancel();
    _reconnectTimer = null;

    await client.disconnect();

    if (_managedProcess != null) {
      _managedProcess!.kill();
      try {
        await _managedProcess!.exitCode.timeout(const Duration(seconds: 2));
      } catch (_) {}
      _managedProcess = null;
    }

    _setState(SupervisorState.idle);
  }

  void dispose() {
    stop();
    _stateController.close();
    _instanceChangedController.close();
  }
}
