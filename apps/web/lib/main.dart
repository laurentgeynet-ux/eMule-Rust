import 'package:flutter/material.dart';
import 'package:flutter_web_plugins/url_strategy.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:app_shell/app/app_controller.dart';
import 'package:app_shell/emule_sion_app.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'daemon_connect_screen.dart';

/// EMule-Rust web remote controller.
///
/// Connects to an already-running mule-daemon over WebSocket — no local
/// supervisor. The daemon URL is resolved from, in order:
/// 1. `?daemon=ws://host:port/rpc` query parameter
/// 2. `--dart-define=DAEMON_URL=…`
/// 3. persisted `daemon.url` preference
/// 4. connect form (suggested default: `ws://<current-host>:4660/rpc`)
void main() {
  WidgetsFlutterBinding.ensureInitialized();
  usePathUrlStrategy();
  runApp(const WebRemoteApp());
}

class WebRemoteApp extends StatefulWidget {
  const WebRemoteApp({super.key});

  @override
  State<WebRemoteApp> createState() => _WebRemoteAppState();
}

class _WebRemoteAppState extends State<WebRemoteApp> {
  static const _prefKey = 'daemon.url';

  AppController? _controller;
  String? _suggestedUrl;
  String? _currentUrl;
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _resolve();
  }

  Future<void> _resolve() async {
    final params = Uri.base.queryParameters;
    const defined = String.fromEnvironment('DAEMON_URL');
    final prefs = await SharedPreferences.getInstance();
    final suggested = 'ws://${Uri.base.host}:4660/rpc';

    String? url;
    if (params.containsKey('daemon')) {
      // `?daemon=` (empty) explicitly resets the saved URL → connect form.
      final q = params['daemon']!;
      if (q.isEmpty) {
        await prefs.remove(_prefKey);
      } else {
        url = q;
      }
    } else if (defined.isNotEmpty) {
      url = defined;
    } else {
      url = prefs.getString(_prefKey);
    }

    if (!mounted) return;
    setState(() {
      _suggestedUrl = suggested;
      _loading = false;
    });
    if (url != null && url.isNotEmpty) _connect(url);
  }

  Future<void> _connect(String url) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_prefKey, url);
    final client = RpcDaemonClient();
    final controller = AppController(client: client);
    controller.initialize(url: Uri.parse(url));
    if (mounted) {
      setState(() {
        _controller = controller;
        _currentUrl = url;
      });
    }
  }

  /// Disconnect from the current daemon and return to the connect form,
  /// pre-filled with the URL that was in use.
  void _disconnect() {
    _controller?.dispose();
    setState(() {
      _controller = null;
      _suggestedUrl = _currentUrl ?? _suggestedUrl;
    });
  }

  @override
  void dispose() {
    _controller?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_controller != null) {
      return eMuleRustApp(viewModel: _controller!, onChangeDaemon: _disconnect);
    }
    return MaterialApp(
      title: 'EMule-Rust',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.dark(),
      locale: const Locale('fr'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: _loading
          ? const Scaffold(
              backgroundColor: AppTheme.darkBackground,
              body: Center(child: CircularProgressIndicator()),
            )
          : DaemonConnectScreen(
              initialUrl: _suggestedUrl,
              onConnect: _connect,
            ),
    );
  }
}
