// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Label + selectable value row with a copy-to-clipboard affordance.
class DetailCopyRow extends StatelessWidget {
  final String label;
  final String value;
  final bool monospace;
  final int maxLines;

  const DetailCopyRow({
    super.key,
    required this.label,
    required this.value,
    this.monospace = false,
    this.maxLines = 1,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: 170,
          child: Text(
            label,
            style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
          ),
        ),
        Expanded(
          child: SelectableText(
            value,
            maxLines: maxLines,
            style: TextStyle(
              fontSize: 12,
              color: AppColors.textPrimary,
              fontFamily: monospace ? 'monospace' : null,
            ),
          ),
        ),
        const SizedBox(width: 8),
        InkWell(
          borderRadius: BorderRadius.circular(4),
          onTap: () {
            Clipboard.setData(ClipboardData(text: value));
          },
          child: const Padding(
            padding: EdgeInsets.all(4.0),
            child: Icon(
              Icons.copy_rounded,
              size: 14,
              color: AppColors.textMuted,
            ),
          ),
        ),
      ],
    );
  }
}
