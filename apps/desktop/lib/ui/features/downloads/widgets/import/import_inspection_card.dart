import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Preview card summarizing a `downloads.inspectPartMet` result before the
/// user confirms the import.
class ImportInspectionCard extends StatelessWidget {
  final PartMetInspectionResult inspection;

  const ImportInspectionCard({super.key, required this.inspection});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppTheme.darkBackground,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: inspection.partFileExists
              ? AppTheme.darkBorder
              : AppTheme.statusFailed,
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(
                Icons.check_circle_outline,
                color: AppTheme.statusCompleted,
                size: 18,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  inspection.fileName,
                  style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.bold,
                    color: AppColors.textPrimary,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          _MetricRow(
            label: context.l10n.totalSize2,
            value: Formatters.formatBytes(inspection.totalSize),
          ),
          const SizedBox(height: 4),
          _MetricRow(
            label: context.l10n.estAcquiredGapsComplement,
            value: Formatters.formatBytes(
              inspection.estimatedTransferredBytes,
            ),
            valueColor: AppTheme.secondary,
          ),
          const SizedBox(height: 4),
          _MetricRow(
            label: context.l10n.partCount,
            value: '${inspection.partCount} part(s)',
          ),
          const SizedBox(height: 4),
          _MetricRow(
            label: '.part File on Disk:',
            value: inspection.partFileExists
                ? 'Present (${Formatters.formatBytes(inspection.partFileSize ?? 0)})'
                : 'MISSING (.part file not found)',
            valueColor: inspection.partFileExists
                ? AppTheme.statusCompleted
                : AppTheme.statusFailed,
          ),
          const SizedBox(height: 4),
          Row(
            children: [
              Text(context.l10n.ed2kHash,
                style: TextStyle(
                  fontSize: 11,
                  color: AppColors.textSecondary,
                ),
              ),
              Expanded(
                child: Text(
                  inspection.ed2kHash,
                  style: const TextStyle(
                    fontSize: 11,
                    fontFamily: 'monospace',
                    color: AppColors.textSoft,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _MetricRow extends StatelessWidget {
  final String label;
  final String value;
  final Color valueColor;

  const _MetricRow({
    required this.label,
    required this.value,
    this.valueColor = AppColors.textPrimary,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(
          label,
          style: const TextStyle(
            fontSize: 12,
            color: AppColors.textSecondary,
          ),
        ),
        Text(
          value,
          style: TextStyle(
            fontSize: 12,
            fontWeight: FontWeight.w600,
            color: valueColor,
          ),
        ),
      ],
    );
  }
}
