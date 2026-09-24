// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Second tab of the IP filter dialog: token-bucket bandwidth quotas.
class RateLimitsTab extends StatelessWidget {
  final TextEditingController downloadLimitController;
  final TextEditingController uploadLimitController;
  final TextEditingController connLimitController;
  final VoidCallback onSave;

  /// Limites courantes — utilisées pour afficher la valeur *effective*
  /// en helper quand elle diffère de la configurée (ratio eD2K / USS).
  final RateLimitsModel? limits;

  /// Appelé sur chaque saisie utilisateur (dirty tracking — les champs
  /// modifiés ne sont pas écrasés par une resync externe).
  final ValueChanged<TextEditingController>? onChanged;

  const RateLimitsTab({
    super.key,
    required this.downloadLimitController,
    required this.uploadLimitController,
    required this.connLimitController,
    required this.onSave,
    this.limits,
    this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final l = limits;

    // Helper sous le champ : valeur effective quand le bucket diffère de
    // la limite configurée (ratio eD2K sur le download, USS sur l'upload).
    String? dlHelper;
    String? ulHelper;
    if (l != null) {
      final effDl = l.downloadBytesPerSec ~/ 1024;
      if (l.downloadRatioApplied && effDl != l.downloadConfiguredBytesPerSec ~/ 1024) {
        dlHelper = l10n.limitsEffectiveRatio(effDl);
      }
      final effUl = l.ussEnabled && l.ussCurrentLimitBytesPerSec > 0
          ? l.ussCurrentLimitBytesPerSec ~/ 1024
          : l.uploadBytesPerSec ~/ 1024;
      if (l.ussEnabled) {
        ulHelper = l10n.limitsUssDriven(effUl);
      } else if (effUl != l.uploadConfiguredBytesPerSec ~/ 1024) {
        ulHelper = l10n.limitsEffective(effUl);
      }
    }

    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(l10n.bandwidthTokenBucketQuotas,
            style: TextStyle(
              fontWeight: FontWeight.bold,
              color: AppColors.textPrimary,
              fontSize: 13,
            ),
          ),
          const SizedBox(height: 4),
          Text(l10n.limitsAreEnforcedAsynchronouslyUsing,
            style: TextStyle(fontSize: 11, color: AppColors.textSoft),
          ),
          const SizedBox(height: 4),
          Text(l10n.rateLimitsCapacityNote,
            style: TextStyle(fontSize: 10, color: AppColors.textFaint),
          ),
          if (l != null && l.ussEnabled) ...[
            const SizedBox(height: 8),
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Icon(Icons.speed_rounded,
                    size: 13, color: AppColors.accentTeal),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(
                    l10n.rateLimitsUssActiveNote,
                    style: const TextStyle(
                        fontSize: 10, color: AppColors.accentTeal),
                  ),
                ),
              ],
            ),
          ],
          const SizedBox(height: 16),
          _LimitField(
            controller: downloadLimitController,
            label: l10n.downloadLimitKbS0,
            helper: dlHelper,
            onChanged: onChanged,
            onSubmit: onSave,
          ),
          const SizedBox(height: 12),
          _LimitField(
            controller: uploadLimitController,
            label: l10n.uploadLimitKbS0,
            helper: ulHelper,
            onChanged: onChanged,
            onSubmit: onSave,
          ),
          const SizedBox(height: 12),
          _LimitField(
            controller: connLimitController,
            label: l10n.maxNewConnectionsMinute,
            onChanged: onChanged,
            onSubmit: onSave,
          ),
          const SizedBox(height: 20),
          Align(
            alignment: Alignment.centerRight,
            child: ElevatedButton.icon(
              onPressed: onSave,
              style: ElevatedButton.styleFrom(
                backgroundColor: AppColors.warningShade700,
                foregroundColor: AppColors.onAccent,
              ),
              icon: const Icon(Icons.save, size: 16),
              label: Text(l10n.saveLimits,
                style: TextStyle(fontWeight: FontWeight.bold),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _LimitField extends StatelessWidget {
  final TextEditingController controller;
  final String label;
  final String? helper;
  final ValueChanged<TextEditingController>? onChanged;
  final VoidCallback onSubmit;

  const _LimitField({
    required this.controller,
    required this.label,
    required this.onSubmit,
    this.helper,
    this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: controller,
      keyboardType: TextInputType.number,
      style: const TextStyle(color: AppColors.textPrimary, fontSize: 12),
      decoration: InputDecoration(
        labelText: label,
        labelStyle: const TextStyle(
          color: AppColors.textSecondary,
          fontSize: 12,
        ),
        helperText: helper,
        helperStyle: const TextStyle(
          color: AppColors.textMuted,
          fontSize: 10,
        ),
        helperMaxLines: 2,
        filled: true,
        fillColor: AppTheme.darkBackground,
      ),
      onChanged: (_) => onChanged?.call(controller),
      onSubmitted: (_) => onSubmit(),
    );
  }
}
