import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Dialog: change the final destination directory of a download. The
/// .part data stays confined in data/temp/ until verification completes.
void showChangeDestinationDialog(
  BuildContext context,
  DownloadItem item, {
  required Future<bool> Function(String newDirectory) onSave,
}) {
  final target = item.targetPath ?? 'downloads';
  final controller = TextEditingController(
    text: target.contains('/') || target.contains(r'\')
        ? (target.replaceAll(r'\', '/').split('/')..removeLast()).join('/')
        : target,
  );

  showDialog(
    context: context,
    builder: (ctx) => AlertDialog(
      title: const Text('Change Target Destination'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Select target destination folder for "${item.fileName}":'),
          const SizedBox(height: 12),
          TextField(
            controller: controller,
            decoration: const InputDecoration(
              labelText: 'Target Directory Path',
              hintText: 'e.g. downloads or C:\\TargetDir',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 8),
          const Text(
            'The folder must exist and be writable. Active .part data remains in data/temp/ until completed.',
            style: TextStyle(fontSize: 11, color: AppColors.textMuted),
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(
          style: FilledButton.styleFrom(backgroundColor: AppTheme.primary),
          onPressed: () async {
            final newDir = controller.text.trim();
            if (newDir.isNotEmpty) {
              final success = await onSave(newDir);
              if (ctx.mounted && success) {
                Navigator.of(ctx).pop();
              }
            }
          },
          child: const Text('Save Destination'),
        ),
      ],
    ),
  );
}
