import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Category picker for the add-link form — shows each category's default
/// destination when set.
class AddLinkCategoryDropdown extends StatelessWidget {
  final List<CategoryItem> categories;
  final String? selectedId;
  final ValueChanged<String?> onChanged;

  const AddLinkCategoryDropdown({
    super.key,
    required this.categories,
    required this.selectedId,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12),
      decoration: BoxDecoration(
        color: AppTheme.darkSurface,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder),
      ),
      child: DropdownButtonHideUnderline(
        child: DropdownButton<String?>(
          value: selectedId,
          isExpanded: true,
          dropdownColor: AppTheme.darkCard,
          items: [
            const DropdownMenuItem<String?>(
              value: null,
              child: Text(
                'Sans catégorie (None)',
                style: TextStyle(
                  fontSize: 13,
                  color: AppColors.textSecondary,
                ),
              ),
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
                        color: c.parsedColor,
                        shape: BoxShape.circle,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Text(
                      c.name,
                      style: const TextStyle(
                        fontSize: 13,
                        color: AppColors.textPrimary,
                      ),
                    ),
                    if (c.defaultDestination != null &&
                        c.defaultDestination!.isNotEmpty)
                      Text(
                        ' (${c.defaultDestination})',
                        style: const TextStyle(
                          fontSize: 11,
                          color: AppColors.textFaint,
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ],
          onChanged: onChanged,
        ),
      ),
    );
  }
}
