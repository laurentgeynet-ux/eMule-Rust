import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Bottom-of-sidebar daemon status card: glowing status dot, state label,
/// daemon/protocol version, opens diagnostics on tap.
class SidebarStatusCard extends StatelessWidget {
  final bool isCompact;
  final Color statusColor;
  final String statusLabel;
  final String? versionLine;
  final String tooltipLabel;
  final VoidCallback onTap;

  const SidebarStatusCard({
    super.key,
    required this.isCompact,
    required this.statusColor,
    required this.statusLabel,
    required this.tooltipLabel,
    required this.onTap,
    this.versionLine,
  });

  @override
  Widget build(BuildContext context) {
    final dot = Container(
      width: isCompact ? 12 : 10,
      height: isCompact ? 12 : 10,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: statusColor,
        boxShadow: [
          BoxShadow(
            color: statusColor.withValues(alpha: 0.5),
            blurRadius: 6,
            spreadRadius: 1,
          ),
        ],
      ),
    );

    return Padding(
      padding: EdgeInsets.all(isCompact ? 8 : 16),
      child: Material(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Tooltip(
            message: isCompact
                ? 'Daemon Status: $tooltipLabel\nClick to view Diagnostics'
                : '',
            child: Container(
              padding: EdgeInsets.all(isCompact ? 10 : 14),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(8),
                border: Border.all(
                  color: AppTheme.darkBorder,
                  width: 0.5,
                ),
              ),
              child: isCompact
                  ? Center(child: dot)
                  : Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Row(
                          children: [
                            dot,
                            const SizedBox(width: 8),
                            Expanded(
                              child: Text(
                                statusLabel,
                                style: const TextStyle(
                                  fontSize: 13,
                                  fontWeight: FontWeight.w600,
                                ),
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                            const Icon(
                              Icons.info_outline,
                              size: 14,
                              color: AppColors.textFaint,
                            ),
                          ],
                        ),
                        if (versionLine != null) ...[
                          const SizedBox(height: 6),
                          Text(
                            versionLine!,
                            style: const TextStyle(
                              fontSize: 11,
                              color: AppColors.textMuted,
                            ),
                          ),
                        ],
                      ],
                    ),
            ),
          ),
        ),
      ),
    );
  }
}
