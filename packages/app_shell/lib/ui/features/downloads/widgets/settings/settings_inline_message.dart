// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Inline feedback strip for settings save/reset outcomes.
class SettingsInlineMessage extends StatelessWidget {
  final String message;
  final bool isError;

  const SettingsInlineMessage({
    super.key,
    required this.message,
    required this.isError,
  });

  @override
  Widget build(BuildContext context) {
    final color = isError ? AppTheme.statusFailed : AppColors.lab;
    final textColor =
        isError ? AppTheme.statusFailed : AppColors.accentTeal;
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Container(
        padding: const EdgeInsets.all(10),
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.15),
          borderRadius: BorderRadius.circular(6),
          border: Border.all(color: color.withValues(alpha: 0.3)),
        ),
        child: Row(
          children: [
            Icon(
              isError ? Icons.error_outline : Icons.check_circle_outline,
              color: textColor,
              size: 16,
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                message,
                style: TextStyle(fontSize: 12, color: textColor),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
