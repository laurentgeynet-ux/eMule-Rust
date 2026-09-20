import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'detail_legend_item.dart';
import 'detail_section_card.dart';

/// eD2K part-map card: one colored block per 9.28 MB part (state via
/// `partColorFor`) plus the counts legend.
class DownloadPartMapCard extends StatelessWidget {
  final DownloadItem item;

  const DownloadPartMapCard({super.key, required this.item});

  @override
  Widget build(BuildContext context) {
    final parts = item.parts;
    final verifiedCount = parts.where((p) => p.isVerified).length;
    final completedCount =
        parts.where((p) => p.state == 'completed').length;
    final downloadingCount =
        parts.where((p) => p.state == 'downloading').length;
    final missingCount = parts.where((p) => p.state == 'missing').length;
    final unavailableCount = parts
        .where((p) => p.state == 'missing' && p.availableSourceCount == 0)
        .length;

    return DetailSectionCard(
      title: 'eD2K Part Map (${parts.length} parts of 9.28 MB)',
      icon: Icons.grid_view_rounded,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (parts.isEmpty)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 8),
              child: Text(
                'Part breakdown is initializing...',
                style: TextStyle(fontSize: 12, color: AppColors.textMuted),
              ),
            )
          else ...[
            Wrap(
              spacing: 4.0,
              runSpacing: 4.0,
              children: parts.map((part) {
                final color = partColorFor(part);
                return Tooltip(
                  message:
                      'Part #${part.partIndex + 1}\n'
                      'Range: ${Formatters.formatBytes(part.startOffset)} - ${Formatters.formatBytes(part.endOffset)}\n'
                      'Size: ${Formatters.formatBytes(part.size)}\n'
                      'Received: ${Formatters.formatBytes(part.receivedBytes)}\n'
                      'Status: ${part.isVerified ? "Verified (Hash valid)" : part.state}\n'
                      'Sources: ${part.availableSourceCount}',
                  child: Container(
                    width: 32.0,
                    height: 24,
                    decoration: BoxDecoration(
                      color: color,
                      borderRadius: BorderRadius.circular(3),
                      border: Border.all(
                        color: AppColors.textPrimary.withValues(alpha: 0.15),
                        width: 0.5,
                      ),
                    ),
                    alignment: Alignment.center,
                    child: Text(
                      '${part.partIndex + 1}',
                      style: TextStyle(
                        fontSize: 9,
                        fontWeight: FontWeight.bold,
                        color: part.state == 'missing'
                            ? AppColors.textFaint
                            : AppColors.textPrimary,
                      ),
                    ),
                  ),
                );
              }).toList(),
            ),
            const SizedBox(height: 14),
            Wrap(
              spacing: 16,
              runSpacing: 8,
              children: [
                DetailLegendItem(
                  color: AppColors.accentTeal,
                  label: 'Verified ($verifiedCount)',
                ),
                DetailLegendItem(
                  color: AppColors.accentBlue,
                  label: 'Completed ($completedCount)',
                ),
                DetailLegendItem(
                  color: AppColors.accentAmber,
                  label: 'Downloading ($downloadingCount)',
                ),
                DetailLegendItem(
                  color: AppColors.fillMedium,
                  label: 'Missing ($missingCount)',
                ),
                DetailLegendItem(
                  color: AppColors.danger,
                  label: 'No source ($unavailableCount)',
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}
