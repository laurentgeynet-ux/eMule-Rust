// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Last ipfilter.dat import report (lines read, coalesced ranges, skipped
/// invalid lines).
class IpFilterReportCard extends StatelessWidget {
  final FilterReportModel report;

  const IpFilterReportCard({super.key, required this.report});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppTheme.darkBackground,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(context.l10n.lastImportReport,
            style: TextStyle(
              fontWeight: FontWeight.bold,
              fontSize: 12,
              color: AppColors.warning,
            ),
          ),
          const SizedBox(height: 6),
          _line(context.l10n.ipReportTotalLines(report.totalLines)),
          _line(context.l10n.ipReportValidEntries(report.validEntriesParsed)),
          _line(context.l10n.ipReportCoalesced(report.coalescedCount)),
          _line(context.l10n.ipReportFinalRanges(report.finalRangeCount)),
          if (report.invalidLines > 0)
            Text(context.l10n.invalidLinesSkipped('${report.invalidLines}'),
              style: const TextStyle(
                fontSize: 11,
                color: AppColors.accentOrange,
              ),
            ),
        ],
      ),
    );
  }

  Widget _line(String text) {
    return Text(
      text,
      style: const TextStyle(
        fontSize: 11,
        color: AppColors.textSecondary,
      ),
    );
  }
}
