// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

/// État « assistant injoignable » : icône, explication (serveur non
/// lancé ou jeton introuvable) et bouton de nouvelle tentative.
class AssistantOffline extends StatelessWidget {
  final String? detail;
  final VoidCallback onRetry;

  const AssistantOffline({super.key, this.detail, required this.onRetry});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 480),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.smart_toy_outlined,
                size: 48, color: AppColors.iconFaint),
            const SizedBox(height: AppSpacing.lg),
            Text(l10n.assistantUnavailableTitle,
                style: AppTextStyles.sectionTitle),
            const SizedBox(height: AppSpacing.sm),
            Text(
              l10n.assistantUnavailableHint,
              style: AppTextStyles.bodySmall,
              textAlign: TextAlign.center,
            ),
            if (detail != null) ...[
              const SizedBox(height: AppSpacing.sm),
              Text(
                detail!,
                style: AppTextStyles.metricLabel
                    .copyWith(color: AppColors.danger),
                textAlign: TextAlign.center,
              ),
            ],
            const SizedBox(height: AppSpacing.lg),
            FilledButton.icon(
              onPressed: onRetry,
              icon: const Icon(Icons.refresh_rounded, size: 16),
              label: Text(l10n.assistantRetry),
            ),
          ],
        ),
      ),
    );
  }
}
