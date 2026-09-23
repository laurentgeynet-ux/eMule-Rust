// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';

class AppTheme {
  // Brand colors
  static const Color primary = Color(0xFF0284C7); // Sky 600
  static const Color primaryDark = Color(0xFF0369A1); // Sky 700
  static const Color secondary = Color(0xFF0D9488); // Teal 600
  static const Color darkBackground = Color(0xFF0F172A); // Slate 900
  static const Color darkSurface = Color(0xFF1E293B); // Slate 800
  static const Color darkCard = Color(0xFF334155); // Slate 700
  static const Color darkBorder = Color(0xFF475569); // Slate 600

  // Status colors
  static const Color statusDownloading = Color(0xFF38BDF8); // Sky 400
  static const Color statusPaused = Color(0xFFFBBF24); // Amber 400
  static const Color statusCompleted = Color(0xFF4ADE80); // Green 400
  static const Color statusFailed = Color(0xFFF87171); // Red 400
  static const Color statusQueued = Color(0xFF94A3B8); // Slate 400
  static const Color statusVerifying = Color(0xFFA78BFA); // Purple 400

  static Color getStatusColor(DownloadStatus status) {
    switch (status) {
      case DownloadStatus.downloading:
      case DownloadStatus.allocating:
        return statusDownloading;
      case DownloadStatus.paused:
        return statusPaused;
      case DownloadStatus.completed:
        return statusCompleted;
      case DownloadStatus.failed:
        return statusFailed;
      case DownloadStatus.verifying:
      case DownloadStatus.completing:
        return statusVerifying;
      case DownloadStatus.queued:
      case DownloadStatus.waitingForSources:
        return statusQueued;
    }
  }

  static ThemeData dark() {
    return ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      scaffoldBackgroundColor: darkBackground,
      colorScheme: const ColorScheme.dark(
        primary: primary,
        secondary: secondary,
        surface: darkSurface,
        surfaceContainerHighest: darkCard,
        onSurface: Colors.white,
      ),
      cardTheme: CardThemeData(
        color: darkSurface,
        elevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(10),
          side: const BorderSide(color: darkBorder, width: 0.5),
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: darkSurface,
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: const BorderSide(color: darkBorder),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: const BorderSide(color: darkBorder),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: const BorderSide(color: primary, width: 1.5),
        ),
      ),
      dividerTheme: const DividerThemeData(
        color: darkBorder,
        thickness: 0.5,
        space: 1,
      ),
    );
  }
}
