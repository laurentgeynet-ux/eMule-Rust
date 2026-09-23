// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/kad_controller.dart';
import '../../../../l10n/l10n.dart';

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
      title: Text(context.l10n.effacerLaTableDeRoutage),
      content: Text(context.l10n.tousLesContactsKadEn,
        style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: Text(context.l10n.annuler,
            style: TextStyle(color: AppColors.textSoft),
          ),
        ),
        FilledButton(
          style: FilledButton.styleFrom(backgroundColor: AppColors.danger),
          onPressed: () {
            Navigator.of(ctx).pop();
            controller.clearContacts();
          },
          child: Text(context.l10n.effacer),
        ),
      ],
    ),
  );
}
