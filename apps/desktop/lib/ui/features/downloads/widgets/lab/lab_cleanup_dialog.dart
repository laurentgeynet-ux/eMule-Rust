import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Destructive confirmation for purging lab fixtures and temp parts.
/// Returns `true` when the user confirms.
Future<bool> showLabCleanupDialog(
  BuildContext context, {
  required int fixtureCount,
}) async {
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (ctx) => AlertDialog(
      backgroundColor: AppTheme.darkSurface,
      title: const Text('Confirm Cleanup'),
      content: Text(
        'This will remove $fixtureCount fixture file(s) in <workdir>/fixtures/ '
        'and clean temporary download part files. Incoming and finished downloads will never be touched.\n\n'
        'Are you sure you want to proceed?',
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(false),
          child: const Text('Cancel'),
        ),
        FilledButton(
          style: FilledButton.styleFrom(
            backgroundColor: AppTheme.statusFailed,
          ),
          onPressed: () => Navigator.of(ctx).pop(true),
          child: const Text('Delete Test Data'),
        ),
      ],
    ),
  );
  return confirmed == true;
}
