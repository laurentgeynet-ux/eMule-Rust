import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Compact eMule-style priority chip shown directly on a download row:
/// current priority label + popup menu to change it (no need to expand
/// the sources panel).
class DownloadPriorityChip extends StatelessWidget {
  final DownloadPriority value;
  final ValueChanged<DownloadPriority> onChanged;
  final bool compact;

  const DownloadPriorityChip({
    super.key,
    required this.value,
    required this.onChanged,
    this.compact = false,
  });

  @override
  Widget build(BuildContext context) {
    return PopupMenuButton<DownloadPriority>(
      tooltip: 'Priorité du téléchargement',
      initialValue: value,
      onSelected: onChanged,
      padding: EdgeInsets.zero,
      color: AppTheme.darkSurface,
      itemBuilder: (context) => [
        for (final p in DownloadPriority.values)
          PopupMenuItem<DownloadPriority>(
            value: p,
            height: 32,
            child: Text(
              p.label,
              style: TextStyle(
                fontSize: 12,
                color: p == value
                    ? AppTheme.primary
                    : AppColors.textSecondary,
                fontWeight: p == value ? FontWeight.w600 : FontWeight.w400,
              ),
            ),
          ),
      ],
      child: Container(
        padding: EdgeInsets.symmetric(
          horizontal: compact ? 5 : 7,
          vertical: 2,
        ),
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(4),
          border: Border.all(
            color: AppTheme.darkBorder,
            width: 0.5,
          ),
          color: AppColors.fillLight.withValues(alpha: 0.4),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              Icons.flag_outlined,
              size: compact ? 9 : 10,
              color: _colorFor(value),
            ),
            const SizedBox(width: 3),
            Text(
              value.label,
              style: TextStyle(
                fontSize: compact ? 9 : 10,
                color: _colorFor(value),
                fontWeight: FontWeight.w600,
              ),
            ),
            Icon(
              Icons.arrow_drop_down,
              size: compact ? 10 : 12,
              color: AppColors.textFaint,
            ),
          ],
        ),
      ),
    );
  }

  static Color _colorFor(DownloadPriority p) => switch (p) {
        DownloadPriority.high => AppTheme.statusDownloading,
        DownloadPriority.normal => AppColors.textSecondary,
        DownloadPriority.low => AppColors.textMuted,
      };
}
