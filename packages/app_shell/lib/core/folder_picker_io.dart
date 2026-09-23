// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'dart:io';

import 'package:file_picker/file_picker.dart';

/// Opens a native folder picker and returns the selected path, or null.
///
/// Desktop/Android: delegates to `file_picker`'s native directory dialog.
/// Returns null on iOS (no directory picker) or when the user cancels.
Future<String?> pickFolder({required String description}) async {
  if (Platform.isLinux || Platform.isWindows || Platform.isMacOS ||
      Platform.isAndroid) {
    try {
      final chosen = await FilePicker.getDirectoryPath(
        dialogTitle: description,
      );
      if (chosen != null && chosen.isNotEmpty && Directory(chosen).existsSync()) {
        return chosen;
      }
    } catch (_) {}
  }
  return null;
}
