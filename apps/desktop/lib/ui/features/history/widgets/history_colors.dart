import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Accent color per history event type.
Color historyEventColor(String eventType) {
  switch (eventType.toLowerCase()) {
    case 'completed':
      return AppTheme.statusCompleted;
    case 'failed':
      return AppTheme.statusFailed;
    case 'resumed':
      return AppTheme.statusDownloading;
    case 'verified_pending_promotion':
      return AppColors.accentPurple;
    case 'removed':
      return AppColors.accentOrange;
    default:
      return AppColors.textSecondary;
  }
}

/// Color for the verification-result badge.
Color historyVerificationColor(String verif) {
  switch (verif.toLowerCase()) {
    case 'verified_ok':
      return AppColors.accentTeal;
    case 'hash_mismatch':
      return AppTheme.statusFailed;
    default:
      return AppColors.textMuted;
  }
}

/// `YYYY-MM-DD HH:mm`
String formatHistoryTimestamp(DateTime dt) {
  final y = dt.year.toString().padLeft(4, '0');
  final m = dt.month.toString().padLeft(2, '0');
  final d = dt.day.toString().padLeft(2, '0');
  final h = dt.hour.toString().padLeft(2, '0');
  final min = dt.minute.toString().padLeft(2, '0');
  return '$y-$m-$d $h:$min';
}
