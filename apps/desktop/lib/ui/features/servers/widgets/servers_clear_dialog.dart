import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/servers_controller.dart';

/// Destructive confirmation: wipe the local server list.
Future<void> showServersClearDialog(
  BuildContext context,
  ServersController controller,
) async {
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (ctx) => AlertDialog(
      title: const Text('Effacer tous les serveurs ?'),
      content: const Text(
        'Cette operation supprime la liste des serveurs enregistres dans votre base locale.',
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(false),
          child: const Text('Annuler'),
        ),
        ElevatedButton(
          style: ElevatedButton.styleFrom(backgroundColor: AppColors.danger),
          onPressed: () => Navigator.of(ctx).pop(true),
          child: const Text('Effacer'),
        ),
      ],
    ),
  );

  if (confirmed == true) {
    await controller.clear();
  }
}
