// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

/// Pastille d'état de la connexion au serveur assistant (en-tête de vue).
class AssistantStatusChip extends StatelessWidget {
  final AssistantConnState state;
  final int visibleTools;

  const AssistantStatusChip({
    super.key,
    required this.state,
    required this.visibleTools,
  });

  @override
  Widget build(BuildContext context) {
    final (label, color) = switch (state) {
      AssistantConnState.connected => (
          context.l10n.assistantStatusConnected(visibleTools),
          AppColors.positive,
        ),
      AssistantConnState.connecting => (
          context.l10n.assistantStatusConnecting,
          AppColors.accentOrange,
        ),
      AssistantConnState.error => (
          context.l10n.assistantStatusError,
          AppColors.danger,
        ),
      AssistantConnState.disconnected => (
          context.l10n.assistantStatusDisconnected,
          AppColors.textFaint,
        ),
    };
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.sm,
        vertical: AppSpacing.xxs,
      ),
      decoration: AppDecorations.chip(color: color),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.circle, size: 8, color: color),
          const SizedBox(width: AppSpacing.xs),
          Text(label, style: AppTextStyles.badgeLabel),
        ],
      ),
    );
  }
}
