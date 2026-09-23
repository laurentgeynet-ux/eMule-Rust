// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Optional default priority picker for the category editor.
class CategoryPriorityDropdown extends StatelessWidget {
  final String? value;
  final ValueChanged<String?> onChanged;

  const CategoryPriorityDropdown({
    super.key,
    required this.value,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: EdgeInsets.symmetric(horizontal: 12),
      decoration: BoxDecoration(
        color: AppTheme.darkSurface,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder),
      ),
      child: DropdownButtonHideUnderline(
        child: DropdownButton<String?>(
          value: value,
          isExpanded: true,
          dropdownColor: AppTheme.darkCard,
          items: [
            DropdownMenuItem(
              value: null,
              child: Text(context.l10n.normalDefault,
                style: TextStyle(fontSize: 12),
              ),
            ),
            DropdownMenuItem(
              value: 'low',
              child: Text(context.l10n.low, style: TextStyle(fontSize: 12)),
            ),
            DropdownMenuItem(
              value: 'normal',
              child: Text(context.l10n.normal, style: TextStyle(fontSize: 12)),
            ),
            DropdownMenuItem(
              value: 'high',
              child: Text(context.l10n.high, style: TextStyle(fontSize: 12)),
            ),
          ],
          onChanged: onChanged,
        ),
      ),
    );
  }
}
