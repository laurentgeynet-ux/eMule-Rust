// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

/// Statistics dashboard: four KPI cards (totals, activity windows,
/// resumes/retries, integrity health) laid out in a responsive wrap.
class HistoryStatsDashboard extends StatelessWidget {
  final HistoryStats stats;

  const HistoryStatsDashboard({super.key, required this.stats});

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final cardWidth = ((constraints.maxWidth - 36) / 4).clamp(200.0, 350.0);
        return Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            HistoryStatCard(
              width: cardWidth,
              title: context.l10n.totalCompleted,
              mainValue: context.l10n.histFilesCount(stats.totalCompletedCount),
              subtitle: Formatters.formatBytes(stats.totalCompletedBytes),
              icon: Icons.check_circle_outline_rounded,
              color: AppTheme.statusCompleted,
            ),
            HistoryStatCard(
              width: cardWidth,
              title: context.l10n.activityWindows,
              mainValue: context.l10n.histSessionCount(
                stats.sessionCompletedCount,
                Formatters.formatBytes(stats.sessionCompletedBytes),
              ),
              subtitle: context.l10n.twentyFourHours(
                  '${stats.last24hCompletedCount}',
                  '${stats.last7dCompletedCount}'),
              icon: Icons.access_time_rounded,
              color: AppColors.accentCyan,
            ),
            HistoryStatCard(
              width: cardWidth,
              title: context.l10n.resumesRetries,
              mainValue:
                  context.l10n.histResumesCount(stats.totalResumesCount),
              subtitle: context.l10n.failuresLogged('${stats.totalFailedCount}'),
              icon: Icons.replay_rounded,
              color: AppTheme.statusDownloading,
            ),
            HistoryStatCard(
              width: cardWidth,
              title: context.l10n.integrityHealth,
              mainValue: context.l10n
                  .histHashMismatchCount(stats.totalIntegrityFailures),
              subtitle: stats.totalIntegrityFailures == 0
                  ? context.l10n.histHashVerified
                  : context.l10n.histIntegrityErrors,
              icon: Icons.verified_user_outlined,
              color: stats.totalIntegrityFailures == 0
                  ? AppColors.accentTeal
                  : AppTheme.statusFailed,
            ),
          ],
        );
      },
    );
  }
}

/// Single KPI card of [HistoryStatsDashboard].
class HistoryStatCard extends StatelessWidget {
  final double width;
  final String title;
  final String mainValue;
  final String subtitle;
  final IconData icon;
  final Color color;

  const HistoryStatCard({
    super.key,
    required this.width,
    required this.title,
    required this.mainValue,
    required this.subtitle,
    required this.icon,
    required this.color,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      width: width,
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            padding: const EdgeInsets.all(8),
            decoration: BoxDecoration(
              color: color.withValues(alpha: 0.15),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Icon(icon, color: color, size: 22),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: const TextStyle(
                    fontSize: 12,
                    color: AppColors.textMuted,
                    fontWeight: FontWeight.w500,
                  ),
                ),
                const SizedBox(height: 4),
                Text(
                  mainValue,
                  style: const TextStyle(
                    fontSize: 15,
                    fontWeight: FontWeight.bold,
                    color: AppColors.textPrimary,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  style: const TextStyle(
                    fontSize: 11,
                    color: AppColors.textFaint,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
