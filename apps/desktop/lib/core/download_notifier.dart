// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:local_notifier/local_notifier.dart';
import 'package:mule_client/mule_client.dart';

/// Native OS notifications for download lifecycle events (desktop only).
///
/// Subscribes to [MuleDaemonClient.downloadLifecycleEvents] and shows a
/// toast when a download completes or fails. No-op on non-desktop
/// platforms and when the OS notifier cannot initialize.
class DownloadNotifier {
  /// Windows AppUserModelID used by the toast notifier. WinToast writes a
  /// `<appName>.lnk` shortcut in the Start Menu under this name.
  static const _windowsAppName = 'eMuleRust';

  final MuleDaemonClient _client;
  StreamSubscription<DownloadLifecycleEvent>? _sub;
  bool _ready = false;

  DownloadNotifier(this._client);

  Future<void> init() async {
    if (!(Platform.isWindows || Platform.isLinux || Platform.isMacOS)) {
      return;
    }
    try {
      if (Platform.isWindows) await _healAumidShortcut();
      await localNotifier.setup(appName: _windowsAppName);
      _ready = true;
    } catch (_) {
      return; // notifier unavailable (e.g. missing dbus on Linux)
    }
    _sub = _client.downloadLifecycleEvents.listen(_onEvent);
  }

  /// Deletes the WinToast shortcut when it does not point at the running
  /// executable, so `setup()` recreates it with the correct target.
  ///
  /// WinToast only validates the stored AppUserModelID, never the target:
  /// after the bundle is moved, the dead shortcut makes the taskbar resolve
  /// our AUMID to a non-existent path and show the generic icon.
  Future<void> _healAumidShortcut() async {
    try {
      final appData = Platform.environment['APPDATA'];
      if (appData == null || appData.isEmpty) return;
      final lnk = File(
        '$appData\\Microsoft\\Windows\\Start Menu\\Programs\\$_windowsAppName.lnk',
      );
      if (lnk.existsSync() && !await _pointsAtCurrentExe(lnk)) {
        await lnk.delete();
      }
    } catch (_) {}
  }

  /// A `.lnk` embeds its target path as raw ASCII/UTF-16 text; if the running
  /// exe path is found inside, the shortcut is already correct.
  Future<bool> _pointsAtCurrentExe(File lnk) async {
    try {
      final bytes = await lnk.readAsBytes();
      final exe = Platform.resolvedExecutable;
      final isAscii = exe.codeUnits.every((u) => u < 128);
      return (isAscii && _containsBytes(bytes, ascii.encode(exe))) ||
          _containsBytes(bytes, _utf16le(exe));
    } catch (_) {
      return false;
    }
  }

  static List<int> _utf16le(String s) {
    final out = <int>[];
    for (final unit in s.codeUnits) {
      out.add(unit & 0xFF);
      out.add(unit >> 8);
    }
    return out;
  }

  static bool _containsBytes(List<int> haystack, List<int> needle) {
    if (needle.isEmpty || needle.length > haystack.length) return false;
    outer:
    for (var i = 0; i <= haystack.length - needle.length; i++) {
      for (var j = 0; j < needle.length; j++) {
        if (haystack[i + j] != needle[j]) continue outer;
      }
      return true;
    }
    return false;
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
