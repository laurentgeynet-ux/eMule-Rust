// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

/// Tuile compacte d'un événement outil : appel validé (nom + arguments)
/// ou résultat (contenu tronqué à l'affichage, données complètes au
/// survol via tooltip).
class ToolEventTile extends StatelessWidget {
  final ChatEntry entry;

  const ToolEventTile({super.key, required this.entry});

  /// Longueur maximale d'un résultat affiché inline — au-delà, la tuile
  /// tronque avec ellipse (le LLM reçoit de toute façon une version
  /// résumée ; l'UI n'a pas à tout dérouler).
  static const int resultPreviewMaxChars = 400;

  @override
  Widget build(BuildContext context) {
    final isCall = entry.kind == ChatEntryKind.toolCall;
    final icon = isCall
        ? Icons.build_circle_outlined
        : entry.isError
            ? Icons.error_outline_rounded
            : Icons.check_circle_outline_rounded;
    final iconColor = isCall
        ? AppColors.accentCyan
        : entry.isError
            ? AppColors.danger
            : AppColors.positive;
    final title = isCall
        ? context.l10n.assistantToolCall
        : context.l10n.assistantToolResult;
    final preview = entry.text.length > resultPreviewMaxChars
        ? '${entry.text.substring(0, resultPreviewMaxChars)}…'
        : entry.text;

    return Container(
      margin: const EdgeInsets.symmetric(vertical: AppSpacing.xxs),
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.md,
        vertical: AppSpacing.sm,
      ),
      decoration: BoxDecoration(
        color: AppColors.fillSubtle,
        borderRadius: BorderRadius.circular(AppRadius.md),
        border: Border.all(color: AppColors.border, width: 0.5),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(icon, size: 14, color: iconColor),
              const SizedBox(width: AppSpacing.xs),
              Text(
                '$title · ${entry.toolName ?? ''}',
                style: AppTextStyles.badgeLabel,
              ),
            ],
          ),
          if (preview.isNotEmpty) ...[
            const SizedBox(height: AppSpacing.xs),
            SelectableText(
              preview,
              style: AppTextStyles.bodySmall.copyWith(fontFamily: 'monospace'),
            ),
          ],
        ],
      ),
    );
  }
}
