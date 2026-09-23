// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Inline error strip for inspection/import failures in the import dialog.
class ImportInlineError extends StatelessWidget {
  final String message;
  final bool withIcon;

  const ImportInlineError({
    super.key,
    required this.message,
    this.withIcon = true,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: AppTheme.statusFailed.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(6),
        border: Border.all(
          color: AppTheme.statusFailed.withValues(alpha: 0.4),
        ),
      ),
      child: withIcon
          ? Row(
              children: [
                const Icon(
                  Icons.error_outline,
                  color: AppTheme.statusFailed,
                  size: 16,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    message,
                    style: const TextStyle(
                      color: AppTheme.statusFailed,
                      fontSize: 12,
                    ),
                  ),
                ),
              ],
            )
          : Text(
              message,
              style: const TextStyle(
                color: AppTheme.statusFailed,
                fontSize: 12,
              ),
            ),
    );
  }
}
