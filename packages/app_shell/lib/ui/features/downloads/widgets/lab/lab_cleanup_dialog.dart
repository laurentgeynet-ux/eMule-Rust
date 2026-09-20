import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

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
      title: Text(context.l10n.confirmCleanup),
      content: Text(context.l10n.thisWillRemoveFixtureFile('$fixtureCount')),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(false),
          child: Text(context.l10n.cancel),
        ),
        FilledButton(
          style: FilledButton.styleFrom(
            backgroundColor: AppTheme.statusFailed,
          ),
          onPressed: () => Navigator.of(ctx).pop(true),
          child: Text(context.l10n.deleteTestData),
        ),
      ],
    ),
  );
  return confirmed == true;
}
