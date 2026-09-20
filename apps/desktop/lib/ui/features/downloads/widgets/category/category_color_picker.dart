import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Parses a `#RRGGBB` hex string, falling back to the primary accent.
Color parseCategoryColor(String hex) {
  final clean = hex.replaceAll('#', '').trim();
  if (clean.length == 6) {
    final val = int.tryParse('FF$clean', radix: 16);
    if (val != null) return Color(val);
  }
  return AppTheme.primary;
}

/// Preset color swatch palette + hex input used by the category editor.
class CategoryColorPicker extends StatelessWidget {
  final TextEditingController colorController;
  final VoidCallback onChanged;

  const CategoryColorPicker({
    super.key,
    required this.colorController,
    required this.onChanged,
  });

  static const presetColors = [
    '#2196F3', // Blue
    '#4CAF50', // Green
    '#9C27B0', // Purple
    '#FF9800', // Orange
    '#E91E63', // Pink
    '#00BCD4', // Cyan
    '#009688', // Teal
    '#FF5722', // Deep Orange
    '#3F51B5', // Indigo
    '#607D8B', // Blue Grey
  ];

  @override
  Widget build(BuildContext context) {
    final currentColor = parseCategoryColor(colorController.text);
    final selected =
        colorController.text.trim().toUpperCase();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: presetColors.map((hex) {
            final c = parseCategoryColor(hex);
            final isSelected = selected == hex.toUpperCase();
            return InkWell(
              borderRadius: BorderRadius.circular(16),
              onTap: () {
                colorController.text = hex;
                onChanged();
              },
              child: Container(
                width: 28,
                height: 28,
                decoration: BoxDecoration(
                  color: c,
                  shape: BoxShape.circle,
                  border: Border.all(
                    color: isSelected
                        ? AppColors.textPrimary
                        : AppColors.fillMedium,
                    width: isSelected ? 2.5 : 1.0,
                  ),
                  boxShadow: isSelected
                      ? [
                          BoxShadow(
                            color: c.withValues(alpha: 0.5),
                            blurRadius: 6,
                            spreadRadius: 1,
                          ),
                        ]
                      : null,
                ),
                child: isSelected
                    ? const Icon(
                        Icons.check,
                        size: 16,
                        color: AppColors.textPrimary,
                      )
                    : null,
              ),
            );
          }).toList(),
        ),
        const SizedBox(height: 10),
        Row(
          children: [
            Container(
              width: 20,
              height: 20,
              decoration: BoxDecoration(
                color: currentColor,
                borderRadius: BorderRadius.circular(4),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: TextField(
                controller: colorController,
                style: const TextStyle(
                  fontSize: 12,
                  fontFamily: 'monospace',
                ),
                decoration: const InputDecoration(
                  hintText: '#RRGGBB',
                  isDense: true,
                  contentPadding: EdgeInsets.symmetric(
                    horizontal: 10,
                    vertical: 8,
                  ),
                ),
                onChanged: (_) => onChanged(),
              ),
            ),
          ],
        ),
      ],
    );
  }
}
