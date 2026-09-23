// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/servers_controller.dart';
import '../../../../l10n/l10n.dart';

/// Destructive confirmation: wipe the local server list.
Future<void> showServersClearDialog(
  BuildContext context,
  ServersController controller,
) async {
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (ctx) => AlertDialog(
      title: Text(context.l10n.effacerTousLesServeurs),
      content: Text(context.l10n.cetteOperationSupprimeLaListe,
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(false),
          child: Text(context.l10n.annuler),
        ),
        ElevatedButton(
          style: ElevatedButton.styleFrom(backgroundColor: AppColors.danger),
          onPressed: () => Navigator.of(ctx).pop(true),
          child: Text(context.l10n.effacer),
        ),
      ],
    ),
  );

  if (confirmed == true) {
    await controller.clear();
  }
}
