import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Footer bar of the settings dialog (Close button).
class SettingsDialogFooter extends StatelessWidget {
  const SettingsDialogFooter({super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
      decoration: const BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.vertical(bottom: Radius.circular(12)),
        border: Border(
          top: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.end,
        children: [
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: AppColors.fillLight,
              foregroundColor: AppColors.textPrimary,
            ),
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }
}
