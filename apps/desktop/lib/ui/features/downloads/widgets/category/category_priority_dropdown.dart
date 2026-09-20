import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

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
      padding: const EdgeInsets.symmetric(horizontal: 12),
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
          items: const [
            DropdownMenuItem(
              value: null,
              child: Text(
                'Normal (Default)',
                style: TextStyle(fontSize: 12),
              ),
            ),
            DropdownMenuItem(
              value: 'low',
              child: Text('Low', style: TextStyle(fontSize: 12)),
            ),
            DropdownMenuItem(
              value: 'normal',
              child: Text('Normal', style: TextStyle(fontSize: 12)),
            ),
            DropdownMenuItem(
              value: 'high',
              child: Text('High', style: TextStyle(fontSize: 12)),
            ),
          ],
          onChanged: onChanged,
        ),
      ),
    );
  }
}
