// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'dart:io';

import 'package:file_picker/file_picker.dart';

/// Opens a native file picker and returns the selected path, or null.
///
/// Desktop/Android: delegates to `file_picker`'s native open dialog.
/// Returns null on iOS (sandboxed content URIs) or when the user cancels.
Future<String?> pickFile({
  required String description,
  List<String>? allowedExtensions,
  String? initialDirectory,
}) async {
  if (Platform.isLinux || Platform.isWindows || Platform.isMacOS ||
      Platform.isAndroid) {
    try {
      final file = await FilePicker.pickFile(
        dialogTitle: description,
        initialDirectory: initialDirectory,
        type: allowedExtensions == null ? FileType.any : FileType.custom,
        allowedExtensions: allowedExtensions,
      );
      final path = file?.path;
      if (path != null && path.isNotEmpty && File(path).existsSync()) {
        return path;
      }
    } catch (_) {}
  }
  return null;
}
