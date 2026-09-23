// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

class Formatters {
  static String formatBytes(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) {
      return '${(bytes / 1024).toStringAsFixed(1)} KB';
    }
    if (bytes < 1024 * 1024 * 1024) {
      return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(2)} GB';
  }

  static String formatSpeed(int bytesPerSec) {
    if (bytesPerSec <= 0) return '0 B/s';
    if (bytesPerSec < 1024) return '$bytesPerSec B/s';
    if (bytesPerSec < 1024 * 1024) {
      return '${(bytesPerSec / 1024).toStringAsFixed(1)} KB/s';
    }
    return '${(bytesPerSec / (1024 * 1024)).toStringAsFixed(2)} MB/s';
  }

  static String formatPercent(double progress) {
    return '${(progress.clamp(0.0, 1.0) * 100).toStringAsFixed(1)}%';
  }

  static String formatEta(int remainingBytes, int speedBps) {
    if (speedBps <= 0 || remainingBytes <= 0) return '∞';
    final seconds = remainingBytes ~/ speedBps;
    if (seconds < 60) return '${seconds}s';
    final minutes = seconds ~/ 60;
    final remSeconds = seconds % 60;
    if (minutes < 60) return '${minutes}m ${remSeconds}s';
    final hours = minutes ~/ 60;
    final remMinutes = minutes % 60;
    if (hours < 24) return '${hours}h ${remMinutes}m';
    final days = hours ~/ 24;
    return '${days}d ${hours % 24}h';
  }

  static String formatDuration(int seconds) {
    if (seconds < 60) return '${seconds}s';
    final minutes = seconds ~/ 60;
    final remSeconds = seconds % 60;
    if (minutes < 60) return '${minutes}m ${remSeconds}s';
    final hours = minutes ~/ 60;
    final remMinutes = minutes % 60;
    return '${hours}h ${remMinutes}m';
  }
}
