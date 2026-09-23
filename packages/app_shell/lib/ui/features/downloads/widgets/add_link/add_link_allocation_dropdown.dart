// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Disk allocation policy picker (sparse / preallocate / none).
class AddLinkAllocationDropdown extends StatelessWidget {
  final String value;
  final ValueChanged<String> onChanged;

  const AddLinkAllocationDropdown({
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
        child: DropdownButton<String>(
          value: value,
          isExpanded: true,
          dropdownColor: AppTheme.darkCard,
          items: [
            DropdownMenuItem(
              value: 'sparse',
              child: Text(context.l10n.sparseInstantGrowsDynamically,
                style: TextStyle(fontSize: 12),
              ),
            ),
            DropdownMenuItem(
              value: 'preallocate',
              child: Text(context.l10n.preallocateFullSizeAvoidsFragmentation,
                style: TextStyle(fontSize: 12),
              ),
            ),
            DropdownMenuItem(
              value: 'none',
              child: Text(context.l10n.noneMinimalInitialDiskUse,
                style: TextStyle(fontSize: 12),
              ),
            ),
          ],
          onChanged: (val) {
            if (val != null) onChanged(val);
          },
        ),
      ),
    );
  }
}
