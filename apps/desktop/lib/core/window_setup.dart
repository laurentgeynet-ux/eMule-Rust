// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:async';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:window_manager/window_manager.dart';

/// Desktop window bootstrap: restores persisted bounds, enforces a minimum
/// size and saves the geometry when the window moves/resizes/closes.
///
/// No-op on non-desktop platforms.
class WindowSetup {
  static const _keyW = 'window.width';
  static const _keyH = 'window.height';
  static const _keyX = 'window.x';
  static const _keyY = 'window.y';

  static const Size defaultSize = Size(1280, 800);
  static const Size minSize = Size(760, 500);

  static Future<void> init() async {
    if (!_isDesktop) return;
    await windowManager.ensureInitialized();

    final prefs = await SharedPreferences.getInstance();
    final w = prefs.getDouble(_keyW);
    final h = prefs.getDouble(_keyH);
    final x = prefs.getDouble(_keyX);
    final y = prefs.getDouble(_keyY);
    final hasBounds = w != null && h != null;

    await windowManager.waitUntilReadyToShow(
      WindowOptions(
        size: hasBounds ? Size(w, h) : defaultSize,
        minimumSize: minSize,
        center: !hasBounds || x == null || y == null,
        title: 'eMuleRust',
        titleBarStyle: TitleBarStyle.normal,
      ),
      () async {
        if (hasBounds && x != null && y != null) {
          await windowManager.setPosition(Offset(x, y));
        }
        await windowManager.show();
        await windowManager.focus();
      },
    );

    windowManager.addListener(_WindowStateSaver());
  }

  static bool get _isDesktop =>
      Platform.isWindows || Platform.isLinux || Platform.isMacOS;
}

/// Persists window bounds (debounced) so the next launch restores them.
class _WindowStateSaver extends WindowListener {
  Timer? _debounce;

  void _scheduleSave() {
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 500), _save);
  }

  Future<void> _save() async {
    final prefs = await SharedPreferences.getInstance();
    final bounds = await windowManager.getBounds();
    await prefs.setDouble(WindowSetup._keyW, bounds.size.width);
    await prefs.setDouble(WindowSetup._keyH, bounds.size.height);
    await prefs.setDouble(WindowSetup._keyX, bounds.topLeft.dx);
    await prefs.setDouble(WindowSetup._keyY, bounds.topLeft.dy);
  }

  @override
  void onWindowResized() => _scheduleSave();

  @override
  void onWindowMoved() => _scheduleSave();

  @override
  void onWindowClose() {
    _debounce?.cancel();
    _save();
  }
}
