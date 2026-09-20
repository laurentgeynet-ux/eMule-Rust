import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import 'lab_step_card.dart';
import '../../../../../l10n/l10n.dart';

/// Step 5: destructive cleanup of generated fixtures and partial temp files
/// (restricted to portable mode to protect user files).
class LabStepCleanup extends StatelessWidget {
  final bool isPortable;
  final bool isCleaning;
  final VoidCallback onCleanup;

  const LabStepCleanup({
    super.key,
    required this.isPortable,
    required this.isCleaning,
    required this.onCleanup,
  });

  @override
  Widget build(BuildContext context) {
    return LabStepCard(
      stepNumber: 5,
      title: context.l10n.cleanTestDataFixturesIncomplete,
      description:
          'Purges generated files in <workdir>/fixtures and partial files in <workdir>/temp. Completed downloads in downloads/ are preserved and never touched.',
      content: Row(
        children: [
          OutlinedButton.icon(
            style: OutlinedButton.styleFrom(
              foregroundColor: AppTheme.statusFailed,
              side: const BorderSide(
                color: AppTheme.statusFailed,
                width: 0.75,
              ),
            ),
            icon: isCleaning
                ? const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: AppTheme.statusFailed,
                    ),
                  )
                : const Icon(Icons.delete_sweep_outlined, size: 16),
            label: Text(context.l10n.cleanupTestData),
            onPressed: isPortable && !isCleaning ? onCleanup : null,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              isPortable
                  ? 'Safe to run in Portable mode.'
                  : 'Cleanup is restricted in installed mode to protect user files.',
              style: TextStyle(
                fontSize: 11,
                color:
                    isPortable ? AppColors.textMuted : AppColors.accentAmber,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
