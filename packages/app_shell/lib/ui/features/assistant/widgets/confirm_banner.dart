// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

/// Bannière de confirmation d'un appel d'outil protégé : affiche le nom
/// et les arguments exacts proposés par le modèle, puis laisse
/// l'utilisateur autoriser ou refuser l'exécution.
class ConfirmBanner extends StatelessWidget {
  final AssistantConfirmRequest request;
  final ValueChanged<bool> onDecision;

  const ConfirmBanner({
    super.key,
    required this.request,
    required this.onDecision,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.lg,
        vertical: AppSpacing.sm,
      ),
      decoration: const BoxDecoration(
        color: AppColors.surface,
        border: Border(
          top: BorderSide(color: AppColors.border, width: 0.5),
        ),
      ),
      child: Row(
        children: [
          const Icon(
            Icons.gpp_maybe_outlined,
            size: 18,
            color: AppColors.accentOrange,
          ),
          const SizedBox(width: AppSpacing.sm),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(l10n.assistantConfirmTitle,
                    style: AppTextStyles.badgeLabel),
                const SizedBox(height: AppSpacing.xxs),
                SelectableText(
                  '${request.toolName} ${request.args}',
                  style: AppTextStyles.bodySmall
                      .copyWith(fontFamily: 'monospace'),
                ),
              ],
            ),
          ),
          const SizedBox(width: AppSpacing.sm),
          TextButton.icon(
            onPressed: () => onDecision(false),
            icon: const Icon(Icons.close_rounded, size: 16),
            label: Text(l10n.assistantConfirmDeny),
            style: TextButton.styleFrom(foregroundColor: AppColors.danger),
          ),
          const SizedBox(width: AppSpacing.xs),
          FilledButton.icon(
            onPressed: () => onDecision(true),
            icon: const Icon(Icons.check_rounded, size: 16),
            label: Text(l10n.assistantConfirmAllow),
          ),
        ],
      ),
    );
  }
}
