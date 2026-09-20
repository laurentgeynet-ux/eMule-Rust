import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/kad_controller.dart';

/// Destructive confirmation: wipe the Kad routing table (memory + SQLite).
void showKadClearContactsDialog(
  BuildContext context,
  KadController controller,
) {
  showDialog(
    context: context,
    builder: (ctx) => AlertDialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      title: const Text('Effacer la table de routage Kad ?'),
      content: const Text(
        'Tous les contacts Kad en mémoire et dans la base SQLite locale seront supprimés.\n'
        'Aucun paquet réseau ne sera envoyé.',
        style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: const Text(
            'Annuler',
            style: TextStyle(color: AppColors.textSoft),
          ),
        ),
        FilledButton(
          style: FilledButton.styleFrom(backgroundColor: AppColors.danger),
          onPressed: () {
            Navigator.of(ctx).pop();
            controller.clearContacts();
          },
          child: const Text('Effacer'),
        ),
      ],
    ),
  );
}
