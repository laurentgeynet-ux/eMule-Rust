// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:ui_kit/ui_kit.dart';

import 'diagnostic_format.dart';
import 'diagnostic_info_row.dart';

/// Info row showing an obfuscated selectable path plus a copy button that
/// puts the *full* path on the clipboard.
class DiagnosticCopyRow extends StatelessWidget {
  final String label;
  final String path;
  final String copyTooltip;
  final String copiedMessage;

  const DiagnosticCopyRow({
    super.key,
    required this.label,
    required this.path,
    required this.copyTooltip,
    required this.copiedMessage,
  });

  @override
  Widget build(BuildContext context) {
    return DiagnosticInfoRow(
      label: label,
      content: Row(
        children: [
          Expanded(
            child: Tooltip(
              message: path,
              child: SelectableText(
                formatPathForDisplay(path),
                style: const TextStyle(
                  fontSize: 12,
                  fontFamily: 'monospace',
                  color: AppColors.textPrimary,
                ),
              ),
            ),
          ),
          const SizedBox(width: 8),
          IconButton(
            icon: const Icon(
              Icons.copy,
              size: 16,
              color: AppColors.textSecondary,
            ),
            tooltip: copyTooltip,
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
            onPressed: () {
              Clipboard.setData(ClipboardData(text: path));
            },
          ),
        ],
      ),
    );
  }
}
