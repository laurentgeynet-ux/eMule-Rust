import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Section 1: default download directory — path field + save, reset to
/// portable default and open folder shortcuts.
class SettingsDirectorySection extends StatelessWidget {
  final TextEditingController dirController;
  final bool isCustom;
  final bool isSaving;
  final VoidCallback onSave;
  final VoidCallback onReset;
  final VoidCallback onOpenFolder;

  const SettingsDirectorySection({
    super.key,
    required this.dirController,
    required this.isCustom,
    required this.isSaving,
    required this.onSave,
    required this.onReset,
    required this.onOpenFolder,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text(
          'Global Default Download Directory',
          style: TextStyle(
            fontSize: 14,
            fontWeight: FontWeight.w600,
            color: AppColors.textPrimary,
          ),
        ),
        const SizedBox(height: 6),
        const Text(
          'This path is used as the default target destination when adding new downloads. '
          'Temporary .part files remain strictly confined in data/temp/ until completed.',
          style: TextStyle(
            fontSize: 12,
            color: AppColors.textSoft,
            height: 1.4,
          ),
        ),
        const SizedBox(height: 14),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: TextField(
                controller: dirController,
                style: const TextStyle(fontSize: 13),
                decoration: InputDecoration(
                  isDense: true,
                  labelText: 'Destination Directory Path',
                  hintText: 'e.g. C:\\Downloads or downloads',
                  filled: true,
                  fillColor: AppTheme.darkCard,
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(8),
                    borderSide:
                        const BorderSide(color: AppTheme.darkBorder),
                  ),
                  helperText: isCustom
                      ? 'Using customized directory (stored in database)'
                      : 'Using default portable directory (<bundleRoot>/downloads)',
                  helperStyle: TextStyle(
                    color: isCustom
                        ? AppColors.accentTeal
                        : AppColors.textFaint,
                    fontSize: 11,
                  ),
                ),
              ),
            ),
            const SizedBox(width: 10),
            FilledButton(
              style: FilledButton.styleFrom(
                backgroundColor: AppTheme.primary,
                padding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 14,
                ),
              ),
              onPressed: isSaving ? null : onSave,
              child: isSaving
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(
                        strokeWidth: 2,
                        color: AppColors.textPrimary,
                      ),
                    )
                  : const Text('Save'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        Row(
          children: [
            OutlinedButton.icon(
              icon: const Icon(Icons.restart_alt_rounded, size: 16),
              label: const Text('Reset to Portable Default'),
              style: OutlinedButton.styleFrom(
                foregroundColor: AppColors.textSecondary,
                side: const BorderSide(color: AppTheme.darkBorder),
              ),
              onPressed: isSaving ? null : onReset,
            ),
            const SizedBox(width: 12),
            TextButton.icon(
              icon: const Icon(Icons.folder_open_rounded, size: 16),
              label: const Text('Open Folder'),
              onPressed: onOpenFolder,
            ),
          ],
        ),
      ],
    );
  }
}
