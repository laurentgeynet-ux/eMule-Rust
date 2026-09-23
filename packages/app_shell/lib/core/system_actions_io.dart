// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'dart:io';

/// Platform abstraction for OS-level file actions (open folder, reveal file).
/// IO implementation for desktop/mobile.
class SystemActions {
  const SystemActions();

  bool get isSupported =>
      Platform.isWindows || Platform.isMacOS || Platform.isLinux;

  String get pathSeparator => Platform.isWindows ? r'\' : '/';

  Future<void> openDirectory(String path) async {
    final dir = Directory(path);
    if (!dir.existsSync()) {
      try {
        dir.createSync(recursive: true);
      } catch (_) {}
    }
    if (Platform.isWindows) {
      await Process.start('explorer.exe', [dir.path]);
    } else if (Platform.isMacOS) {
      await Process.start('open', [dir.path]);
    } else if (Platform.isLinux) {
      await Process.start('xdg-open', [dir.path]);
    }
  }

  /// Opens [url] in the system default browser.
  Future<void> openUrl(String url) async {
    if (Platform.isWindows) {
      await Process.start('explorer.exe', [url]);
    } else if (Platform.isMacOS) {
      await Process.start('open', [url]);
    } else if (Platform.isLinux) {
      await Process.start('xdg-open', [url]);
    }
  }

  Future<void> revealFile(String filePath) async {
    final file = File(filePath);
    final dir = file.parent;
    if (Platform.isWindows) {
      if (file.existsSync()) {
        await Process.start('explorer.exe', ['/select,', file.path]);
      } else if (dir.existsSync()) {
        await Process.start('explorer.exe', [dir.path]);
      }
    } else if (Platform.isMacOS) {
      if (file.existsSync()) {
        await Process.start('open', ['-R', file.path]);
      } else if (dir.existsSync()) {
        await Process.start('open', [dir.path]);
      }
    } else if (Platform.isLinux) {
      if (dir.existsSync()) {
        await Process.start('xdg-open', [dir.path]);
      }
    }
  }
}

/// Shared instance — resolved per-platform via conditional export.
const systemActions = SystemActions();
