// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'ip_filter_report_card.dart';
import 'ip_filter_status_card.dart';
import '../../../../../core/file_picker_helper.dart';
import '../../../../../l10n/l10n.dart';

/// First tab of the IP filter dialog: status card + local ipfilter.dat
/// import/clear + last import report.
class IpFilterTab extends StatelessWidget {
  final FilterStatusModel? status;
  final TextEditingController filePathController;
  final ValueChanged<bool> onToggle;
  final ValueChanged<String> onImport;
  final VoidCallback onClear;

  const IpFilterTab({
    super.key,
    required this.status,
    required this.filePathController,
    required this.onToggle,
    required this.onImport,
    required this.onClear,
  });

  @override
  Widget build(BuildContext context) {
    final report = status?.lastImportReport;
    final rangeCount = status?.rangeCount ?? 0;

    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          IpFilterStatusCard(status: status, onToggle: onToggle),
          const SizedBox(height: 16),
          Text(context.l10n.importIpfilterDat,
            style: TextStyle(
              fontWeight: FontWeight.w600,
              color: AppColors.textPrimary,
              fontSize: 13,
            ),
          ),
          const SizedBox(height: 6),
          Text(context.l10n.loadALocalPeerguardianEmule,
            style: TextStyle(fontSize: 11, color: AppColors.textSoft),
          ),
          const SizedBox(height: 10),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: filePathController,
                  style: const TextStyle(
                    color: AppColors.textPrimary,
                    fontSize: 12,
                  ),
                  decoration: InputDecoration(
                    hintText: 'C:\\path\\to\\ipfilter.dat',
                    hintStyle: const TextStyle(
                      color: AppColors.textFaint,
                      fontSize: 12,
                    ),
                    filled: true,
                    fillColor: AppTheme.darkBackground,
                    contentPadding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 10,
                    ),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide:
                          const BorderSide(color: AppTheme.darkBorder),
                    ),
                  ),
                ),
              ),
              const SizedBox(width: 8),
              IconButton(
                icon: const Icon(Icons.folder_open_rounded, size: 18),
                tooltip: context.l10n.browse,
                onPressed: () async {
                  final title = context.l10n.importIpfilterDat;
                  final picked = await pickFile(
                    description: title,
                    allowedExtensions: const ['dat'],
                  );
                  if (picked != null) filePathController.text = picked;
                },
              ),
              ElevatedButton.icon(
                onPressed: () {
                  final path = filePathController.text.trim();
                  if (path.isNotEmpty) onImport(path);
                },
                style: ElevatedButton.styleFrom(
                  backgroundColor: AppColors.warningShade700,
                  foregroundColor: AppColors.onAccent,
                  padding: const EdgeInsets.symmetric(
                    horizontal: 14,
                    vertical: 12,
                  ),
                ),
                icon: const Icon(Icons.file_upload, size: 16),
                label: Text(context.l10n.import,
                  style: TextStyle(fontWeight: FontWeight.bold),
                ),
              ),
              if (rangeCount > 0) ...[
                const SizedBox(width: 8),
                OutlinedButton(
                  onPressed: onClear,
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppColors.danger,
                    side: const BorderSide(color: AppColors.danger),
                  ),
                  child: Text(context.l10n.clear),
                ),
              ],
            ],
          ),
          const SizedBox(height: 16),
          if (report != null) IpFilterReportCard(report: report),
        ],
      ),
    );
  }
}
