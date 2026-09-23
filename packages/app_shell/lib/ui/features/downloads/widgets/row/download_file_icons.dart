// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';

/// Icon picked from the file extension shown on download rows.
IconData fileIconFor(String fileName) {
  final lower = fileName.toLowerCase();
  if (lower.endsWith('.iso') || lower.endsWith('.img')) {
    return Icons.album_rounded;
  }
  if (lower.endsWith('.zip') ||
      lower.endsWith('.rar') ||
      lower.endsWith('.7z') ||
      lower.endsWith('.tar.xz')) {
    return Icons.folder_zip_rounded;
  }
  if (lower.endsWith('.mp4') ||
      lower.endsWith('.mkv') ||
      lower.endsWith('.avi')) {
    return Icons.video_file_rounded;
  }
  if (lower.endsWith('.mp3') ||
      lower.endsWith('.flac') ||
      lower.endsWith('.ogg')) {
    return Icons.audio_file_rounded;
  }
  if (lower.endsWith('.blend')) {
    return Icons.view_in_ar_rounded;
  }
  return Icons.insert_drive_file_rounded;
}

/// Truncates a download id for compact display (`xxxxxxxx…`).
String abbreviateId(String id) {
  if (id.length <= 8) return id;
  return '${id.substring(0, 8)}…';
}
