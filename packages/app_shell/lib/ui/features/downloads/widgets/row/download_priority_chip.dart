import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../l10n/enum_labels.dart';
import '../../../../../l10n/l10n.dart';

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
    final color = _colorFor(value);
    return PopupMenuButton<DownloadPriority>(
      tooltip: context.l10n.tooltipDownloadPriority,
      initialValue: value,
      onSelected: onChanged,
      padding: EdgeInsets.zero,
      color: AppTheme.darkSurface,
      itemBuilder: (context) => [
        for (final p in DownloadPriority.values)
          PopupMenuItem<DownloadPriority>(
            value: p,
            height: 34,
            child: Row(
              children: [
                Icon(
                  _iconFor(p),
                  size: 14,
                  color: _colorFor(p),
                ),
                const SizedBox(width: 8),
                Text(
                  p.labelL10n(context.l10n),
                  style: TextStyle(
                    fontSize: 12,
                    color: p == value
                        ? _colorFor(p)
                        : AppColors.textSecondary,
                    fontWeight:
                        p == value ? FontWeight.w600 : FontWeight.w400,
                  ),
                ),
              ],
            ),
          ),
      ],
      child: Container(
        padding: EdgeInsets.symmetric(
          horizontal: compact ? 6 : 8,
          vertical: 2,
        ),
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(20),
          color: color.withValues(alpha: 0.12),
          border: Border.all(
            color: color.withValues(alpha: 0.35),
            width: 0.5,
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              _iconFor(value),
              size: compact ? 10 : 11,
              color: color,
            ),
            const SizedBox(width: 3),
            Text(
              value.labelL10n(context.l10n),
              style: TextStyle(
                fontSize: compact ? 9 : 10,
                color: color,
                fontWeight: FontWeight.w600,
              ),
            ),
            Icon(
              Icons.arrow_drop_down,
              size: compact ? 10 : 12,
              color: color.withValues(alpha: 0.6),
            ),
          ],
        ),
      ),
    );
  }

  /// 3 couleurs franches par priorité : ambre = haute (attention),
  /// bleu ciel = normale (défaut), ardoise = basse (discrète).
  static Color _colorFor(DownloadPriority p) => switch (p) {
        DownloadPriority.high => AppTheme.statusPaused, // Amber 400
        DownloadPriority.normal => AppTheme.statusDownloading, // Sky 400
        DownloadPriority.low => AppTheme.statusQueued, // Slate 400
      };

  static IconData _iconFor(DownloadPriority p) => switch (p) {
        DownloadPriority.high => Icons.keyboard_double_arrow_up_rounded,
        DownloadPriority.normal => Icons.remove_rounded,
        DownloadPriority.low => Icons.keyboard_double_arrow_down_rounded,
      };
}
