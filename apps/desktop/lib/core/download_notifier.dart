import 'dart:async';
import 'dart:io';

import 'package:local_notifier/local_notifier.dart';
import 'package:mule_client/mule_client.dart';

/// Native OS notifications for download lifecycle events (desktop only).
///
/// Subscribes to [MuleDaemonClient.downloadLifecycleEvents] and shows a
/// toast when a download completes or fails. No-op on non-desktop
/// platforms and when the OS notifier cannot initialize.
class DownloadNotifier {
  final MuleDaemonClient _client;
  StreamSubscription<DownloadLifecycleEvent>? _sub;
  bool _ready = false;

  DownloadNotifier(this._client);

  Future<void> init() async {
    if (!(Platform.isWindows || Platform.isLinux || Platform.isMacOS)) {
      return;
    }
    try {
      await localNotifier.setup(appName: 'EMule-Rust');
      _ready = true;
    } catch (_) {
      return; // notifier unavailable (e.g. missing dbus on Linux)
    }
    _sub = _client.downloadLifecycleEvents.listen(_onEvent);
  }

  void _onEvent(DownloadLifecycleEvent e) {
    if (!_ready) return;
    final name = e.download?.fileName;
    if (name == null) return;
    final (title, body) = switch (e.kind) {
      DownloadLifecycleKind.completed => ('Téléchargement terminé', name),
      DownloadLifecycleKind.failed => (
          'Téléchargement échoué',
          e.reason != null ? '$name — ${e.reason}' : name,
        ),
      _ => (null, null),
    };
    if (title == null) return;
    LocalNotification(title: title, body: body!).show();
  }

  Future<void> dispose() => _sub?.cancel() ?? Future.value();
}
