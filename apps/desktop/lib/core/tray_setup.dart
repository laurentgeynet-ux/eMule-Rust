// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:io';

import 'package:app_shell/core/app_quit.dart';
import 'package:local_notifier/local_notifier.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

/// System tray icon with Show / Quit entries.
///
/// Desktop-only (Windows / Linux / macOS). The icon file is bundled as a
/// Flutter asset and resolved from `data/flutter_assets` next to the
/// executable.
///
/// When the tray is active, closing the main window hides it to the tray
/// instead of quitting — the classic P2P client behaviour. Quit is only
/// possible through the tray menu.
class TraySetup {
  /// SharedPreferences flag: the "still running in the tray" toast has been
  /// shown once already.
  static const _notifiedKey = 'tray.closeToTrayNotified';

  static Future<void> init() async {
    if (!_isDesktop) return;
    try {
      final iconPath = _assetPath();
      await trayManager.setIcon(iconPath);
      await trayManager.setToolTip('eMuleRust');
      await trayManager.setContextMenu(
        Menu(items: [
          MenuItem(key: 'show', label: _Labels.showItem),
          MenuItem.separator(),
          MenuItem(key: 'quit', label: _Labels.quitItem),
        ]),
      );
      trayManager.addListener(_TrayListener());
      // Intercept close only once the tray is guaranteed to exist —
      // otherwise the window could not be closed at all.
      await windowManager.setPreventClose(true);
      windowManager.addListener(_MinimizeToTray());
      AppQuit.onHideToTray = _MinimizeToTray.hideToTray;
      AppQuit.closeToTray = true;
    } catch (_) {
      // Tray is best-effort (e.g. missing AppIndicator on some Linux DEs)
    }
  }

  static String _assetPath() {
    final exeDir = File(Platform.resolvedExecutable).parent.path;
    final sep = Platform.isWindows ? r'\' : '/';
    return '$exeDir${sep}data${sep}flutter_assets${sep}assets${sep}app_icon.ico';
  }

  static bool get _isDesktop =>
      Platform.isWindows || Platform.isLinux || Platform.isMacOS;
}

/// Localized strings for the tray menu and the first-hide toast.
class _Labels {
  static bool get _fr => Platform.localeName.toLowerCase().startsWith('fr');

  static String get showItem => _fr ? 'Afficher eMuleRust' : 'Show eMuleRust';
  static String get quitItem => _fr ? 'Quitter' : 'Quit';
  static String get toastTitle =>
      _fr ? 'eMuleRust continue en arrière-plan' : 'eMuleRust keeps running';
  static String get toastBody => _fr
      ? 'L\'application est réduite dans la zone de notification. '
          'Clic gauche pour l\'afficher, clic droit pour quitter.'
      : 'The app was minimized to the system tray. '
          'Left-click the icon to show it, right-click to quit.';
}

/// Hides the app into the tray on window close.
///
/// Registered both as a `window_manager` listener (fires wherever the OS
/// close event reaches the plugin) and as `AppQuit.onHideToTray` (fired by
/// the Flutter exit-request handler — the only path on Windows, where the
/// engine consumes `WM_CLOSE`). Both may fire for one close: `hide()` is
/// idempotent and the hint toast is deduplicated.
class _MinimizeToTray extends WindowListener {
  static bool _hintPending = false;

  static Future<void> hideToTray() async {
    // On a real quit the window is being destroyed — no hide needed.
    if (AppQuit.quitting) return;
    await windowManager.hide();
    await _notifyFirstHide();
  }

  @override
  void onWindowClose() => hideToTray();

  /// One-time toast explaining that the app is still running in the tray.
  static Future<void> _notifyFirstHide() async {
    if (_hintPending) return;
    _hintPending = true;
    try {
      final prefs = await SharedPreferences.getInstance();
      if (prefs.getBool(TraySetup._notifiedKey) ?? false) return;
      await LocalNotification(
        title: _Labels.toastTitle,
        body: _Labels.toastBody,
      ).show();
      await prefs.setBool(TraySetup._notifiedKey, true);
    } catch (_) {
    } finally {
      _hintPending = false;
    }
  }
}

class _TrayListener extends TrayListener {
  @override
  void onTrayIconMouseDown() async {
    // Single left click restores the window; a double click fires this
    // handler twice and has the same effect.
    await windowManager.show();
    await windowManager.focus();
  }

  @override
  void onTrayIconRightMouseDown() {
    trayManager.popUpContextMenu();
  }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) async {
    switch (menuItem.key) {
      case 'show':
        await windowManager.show();
        await windowManager.focus();
      case 'quit':
        // Real quit: flag it, lift the close prevention, then retrigger a
        // normal close. The exit handler sees `quitting`, stops the daemon
        // gracefully and lets the process exit — no hide to tray.
        AppQuit.quitting = true;
        await windowManager.setPreventClose(false);
        await windowManager.close();
    }
  }
}
