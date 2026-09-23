// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'detail_metric_tile.dart';
import 'detail_section_card.dart';
import '../../../../../l10n/l10n.dart';

/// Transfer metrics card: progress bar, received/remaining/verified/rates/
/// ETA/sources/attempts tiles and the last-error banner.
class DownloadMetricsCard extends StatelessWidget {
  final DownloadItem item;
  final Color statusColor;

  const DownloadMetricsCard({
    super.key,
    required this.item,
    required this.statusColor,
  });

  @override
  Widget build(BuildContext context) {
    return DetailSectionCard(
      title: context.l10n.transferMetricsSpeed,
      icon: Icons.speed_rounded,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ClipRRect(
            borderRadius: BorderRadius.circular(6),
            child: LinearProgressIndicator(
              value: item.progress,
              minHeight: 10,
              backgroundColor: AppColors.fillLight,
              valueColor: AlwaysStoppedAnimation<Color>(statusColor),
            ),
          ),
          const SizedBox(height: 14),
          Wrap(
            spacing: 24,
            runSpacing: 12,
            children: [
              DetailMetricTile(
                label: context.l10n.received,
                value: Formatters.formatBytes(item.receivedBytes),
              ),
              DetailMetricTile(
                label: context.l10n.remaining,
                value: Formatters.formatBytes(item.remainingBytes),
              ),
              DetailMetricTile(
                label: context.l10n.verified,
                value: Formatters.formatBytes(item.verifiedBytes),
              ),
              DetailMetricTile(
                label: context.l10n.currentRate,
                value: Formatters.formatSpeed(item.downloadRateBps),
              ),
              DetailMetricTile(
                label: context.l10n.averageRate,
                value: Formatters.formatSpeed(item.averageRateBps),
              ),
              DetailMetricTile(
                label: context.l10n.eta,
                value: item.etaSeconds != null
                    ? Formatters.formatEta(
                        item.remainingBytes,
                        item.downloadRateBps,
                      )
                    : 'N/A',
              ),
              DetailMetricTile(
                label: context.l10n.sources,
                value: context.l10n.sourcesLoopback(item.sourceCount),
              ),
              DetailMetricTile(
                label: context.l10n.attempts,
                value: '${item.connectionAttempts}',
              ),
            ],
          ),
          if (item.lastError != null && item.lastError!.isNotEmpty) ...[
            const SizedBox(height: 12),
            Container(
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: AppTheme.statusFailed.withValues(alpha: 0.15),
                borderRadius: BorderRadius.circular(6),
                border: Border.all(
                  color: AppTheme.statusFailed.withValues(alpha: 0.3),
                ),
              ),
              child: Row(
                children: [
                  const Icon(
                    Icons.warning_amber_rounded,
                    color: AppTheme.statusFailed,
                    size: 16,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(context.l10n.lastError('${item.lastError}'),
                      style: const TextStyle(
                        fontSize: 12,
                        color: AppTheme.statusFailed,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }
}
