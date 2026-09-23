// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Optional category picker for the import form — selecting a category
/// with a default destination also suggests it as the target directory.
class ImportCategoryDropdown extends StatelessWidget {
  final List<CategoryItem> categories;
  final String? selectedId;
  final ValueChanged<String?> onChanged;

  const ImportCategoryDropdown({
    super.key,
    required this.categories,
    required this.selectedId,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return DropdownButtonFormField<String?>(
      key: const Key('category_select'),
      initialValue: selectedId,
      dropdownColor: AppTheme.darkSurface,
      style: const TextStyle(color: AppColors.textPrimary, fontSize: 13),
      decoration: InputDecoration(
        filled: true,
        fillColor: AppTheme.darkBackground,
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 12,
          vertical: 10,
        ),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: const BorderSide(color: AppTheme.darkBorder),
        ),
      ),
      items: [
        DropdownMenuItem<String?>(
          value: null,
          child: Text(context.l10n.noCategory),
        ),
        ...categories.map(
          (c) => DropdownMenuItem<String?>(
            value: c.id,
            child: Row(
              children: [
                Container(
                  width: 10,
                  height: 10,
                  decoration: BoxDecoration(
                    color: Color(
                      int.parse(c.color.replaceFirst('#', '0xFF')),
                    ),
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: 8),
                Text(c.name),
              ],
            ),
          ),
        ),
      ],
      onChanged: onChanged,
    );
  }
}
