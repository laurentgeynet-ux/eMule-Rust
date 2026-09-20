import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

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
              title: 'Total Completed',
              mainValue: '${stats.totalCompletedCount} files',
              subtitle: Formatters.formatBytes(stats.totalCompletedBytes),
              icon: Icons.check_circle_outline_rounded,
              color: AppTheme.statusCompleted,
            ),
            HistoryStatCard(
              width: cardWidth,
              title: 'Activity Windows',
              mainValue:
                  '${stats.sessionCompletedCount} session '
                  '(${Formatters.formatBytes(stats.sessionCompletedBytes)})',
              subtitle:
                  '24h: ${stats.last24hCompletedCount} | '
                  '7d: ${stats.last7dCompletedCount}',
              icon: Icons.access_time_rounded,
              color: AppColors.accentCyan,
            ),
            HistoryStatCard(
              width: cardWidth,
              title: 'Resumes & Retries',
              mainValue: '${stats.totalResumesCount} resumes',
              subtitle: '${stats.totalFailedCount} failures logged',
              icon: Icons.replay_rounded,
              color: AppTheme.statusDownloading,
            ),
            HistoryStatCard(
              width: cardWidth,
              title: 'Integrity Health',
              mainValue: '${stats.totalIntegrityFailures} hash mismatch',
              subtitle: stats.totalIntegrityFailures == 0
                  ? '100% hash verified'
                  : 'Integrity errors detected',
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
