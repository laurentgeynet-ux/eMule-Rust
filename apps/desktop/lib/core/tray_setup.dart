// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'dart:io';

import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

/// System tray icon with Show / Quit entries.
///
/// Desktop-only (Windows / Linux / macOS). The icon file is bundled as a
/// Flutter asset and resolved from `data/flutter_assets` next to the
/// executable.
class TraySetup {
  static Future<void> init() async {
    if (!_isDesktop) return;
    try {
      final iconPath = _assetPath();
      await trayManager.setIcon(iconPath);
      await trayManager.setToolTip('EMule-Rust');
      await trayManager.setContextMenu(
        Menu(items: [
          MenuItem(key: 'show', label: 'Show EMule-Rust'),
          MenuItem.separator(),
          MenuItem(key: 'quit', label: 'Quit'),
        ]),
      );
      trayManager.addListener(_TrayListener());
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

class _TrayListener extends TrayListener {
  @override
  void onTrayIconMouseDown() async {
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
        await windowManager.destroy();
    }
  }
}
