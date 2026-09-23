// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Success view shown after a .part.met import: banner, scrollable audit
/// report (selectable + copyable) and a Done action.
class ImportReportView extends StatelessWidget {
  final String report;

  const ImportReportView({super.key, required this.report});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: AppTheme.statusCompleted.withValues(alpha: 0.15),
            borderRadius: BorderRadius.circular(8),
            border: Border.all(
              color: AppTheme.statusCompleted.withValues(alpha: 0.3),
            ),
          ),
          child: Row(
            children: [
              Icon(
                Icons.check_circle_outline,
                color: AppTheme.statusCompleted,
                size: 20,
              ),
              SizedBox(width: 10),
              Expanded(
                child: Text(context.l10n.importSuccessfulTheDownloadIs,
                  style: TextStyle(
                    color: AppColors.textPrimary,
                    fontSize: 13,
                    fontWeight: FontWeight.bold,
                  ),
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        Text(context.l10n.auditReport,
          style: TextStyle(
            fontSize: 13,
            fontWeight: FontWeight.bold,
            color: AppColors.textPrimary,
          ),
        ),
        const SizedBox(height: 6),
        Expanded(
          child: Container(
            width: double.infinity,
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: AppTheme.darkBackground,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: AppTheme.darkBorder),
            ),
            child: SingleChildScrollView(
              child: SelectableText(
                report,
                key: const Key('import_audit_report_text'),
                style: const TextStyle(
                  fontFamily: 'monospace',
                  fontSize: 11,
                  color: AppColors.textSecondary,
                  height: 1.4,
                ),
              ),
            ),
          ),
        ),
        const SizedBox(height: 16),
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            OutlinedButton.icon(
              key: const Key('copy_report_button'),
              onPressed: () {
                Clipboard.setData(ClipboardData(text: report));
              },
              icon: const Icon(Icons.copy, size: 16),
              label: Text(context.l10n.copyReport),
            ),
            const SizedBox(width: 12),
            ElevatedButton(
              key: const Key('finish_import_button'),
              onPressed: () => Navigator.of(context).pop(true),
              style: ElevatedButton.styleFrom(
                backgroundColor: AppTheme.primary,
                foregroundColor: AppColors.textPrimary,
              ),
              child: Text(context.l10n.done),
            ),
          ],
        ),
      ],
    );
  }
}
