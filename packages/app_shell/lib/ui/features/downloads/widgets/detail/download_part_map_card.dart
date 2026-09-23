// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'detail_legend_item.dart';
import 'detail_section_card.dart';
import '../../../../../l10n/l10n.dart';

/// eD2K part-map card: one cell per 9.28 MB part. Each cell shows the
/// availability color of its missing gap (soft red = no source, sky
/// gradient rare→common like eMule), filled bottom-up with the received
/// fraction (slate, teal when hash-verified), plus an amber top stripe
/// while the part has in-flight block requests.
class DownloadPartMapCard extends StatelessWidget {
  final DownloadItem item;

  const DownloadPartMapCard({super.key, required this.item});

  bool _partHasPending(DownloadPartInfo part) {
    return item.pendingRanges.any(
      (r) => r.start < part.endOffset && r.end > part.startOffset,
    );
  }

  @override
  Widget build(BuildContext context) {
    final parts = item.parts;
    final verifiedCount = parts.where((p) => p.isVerified).length;
    final completedCount =
        parts.where((p) => p.state == 'completed').length;
    final pendingCount = parts.where(_partHasPending).length;
    final availableCount = parts
        .where((p) => p.state == 'missing' && p.availableSourceCount > 0)
        .length;
    final unavailableCount = parts
        .where((p) => p.state == 'missing' && p.availableSourceCount == 0)
        .length;

    return DetailSectionCard(
      title: context.l10n.ed2kPartMapPartsOf('${parts.length}'),
      icon: Icons.grid_view_rounded,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (parts.isEmpty)
            Padding(
              padding: EdgeInsets.symmetric(vertical: 8),
              child: Text(context.l10n.partBreakdownIsInitializing,
                style: TextStyle(fontSize: 12, color: AppColors.textMuted),
              ),
            )
          else ...[
            Wrap(
              spacing: 4.0,
              runSpacing: 4.0,
              children: parts.map((part) {
                final gapColor = partColorFor(part);
                final hasPending = _partHasPending(part);
                final fillFraction = part.progress;
                final fillColor =
                    part.isVerified ? partVerifiedColor : partHaveColor;
                return Tooltip(
                  message: context.l10n.part(
                      '${part.partIndex + 1}',
                      Formatters.formatBytes(part.startOffset),
                      Formatters.formatBytes(part.endOffset),
                      Formatters.formatBytes(part.size),
                      Formatters.formatBytes(part.receivedBytes),
                      part.isVerified ? "Verified (Hash valid)" : part.state,
                      '${part.availableSourceCount}'),
                  child: SizedBox(
                    width: 32.0,
                    height: 24,
                    child: Stack(
                      children: [
                        // Gap color = availability of the missing portion.
                        Positioned.fill(
                          child: Container(
                            decoration: BoxDecoration(
                              color: gapColor,
                              borderRadius: BorderRadius.circular(3),
                              border: Border.all(
                                color: AppColors.textPrimary
                                    .withValues(alpha: 0.15),
                                width: 0.5,
                              ),
                            ),
                          ),
                        ),
                        // Received fraction fills bottom-up (eMule draws
                        // the gaps; here the filled level shows progress).
                        if (fillFraction > 0)
                          Positioned(
                            left: 0,
                            right: 0,
                            bottom: 0,
                            height: 24 * fillFraction,
                            child: Container(
                              decoration: BoxDecoration(
                                color: fillColor,
                                borderRadius: fillFraction >= 1.0
                                    ? BorderRadius.circular(3)
                                    : const BorderRadius.vertical(
                                        bottom: Radius.circular(3)),
                              ),
                            ),
                          ),
                        // Amber stripe while the part has in-flight blocks.
                        if (hasPending)
                          Positioned(
                            left: 0,
                            right: 0,
                            top: 0,
                            height: 3,
                            child: Container(
                              decoration: const BoxDecoration(
                                color: partPendingColor,
                                borderRadius: BorderRadius.vertical(
                                    top: Radius.circular(3)),
                              ),
                            ),
                          ),
                        Positioned.fill(
                          child: Center(
                            child: Text('${part.partIndex + 1}',
                              style: TextStyle(
                                fontSize: 9,
                                fontWeight: FontWeight.bold,
                                color: AppColors.textPrimary,
                                shadows: const [
                                  Shadow(color: Colors.black54, blurRadius: 2),
                                ],
                              ),
                            ),
                          ),
                        ),
                      ],
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
                  color: partVerifiedColor,
                  label: context.l10n.verified2('$verifiedCount'),
                ),
                DetailLegendItem(
                  color: partHaveColor,
                  label: context.l10n.completed('$completedCount'),
                ),
                DetailLegendItem(
                  color: partPendingColor,
                  label: context.l10n.downloading('$pendingCount'),
                ),
                DetailLegendItem(
                  color: availabilityColor(3),
                  label: context.l10n.missing('$availableCount'),
                ),
                DetailLegendItem(
                  color: partMissingColor,
                  label: context.l10n.noSource('$unavailableCount'),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

