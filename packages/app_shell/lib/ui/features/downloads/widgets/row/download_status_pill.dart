import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Colored status pill shown on download rows (compact and full layouts).
class DownloadStatusPill extends StatelessWidget {
  final DownloadStatus status;
  final bool compact;

  const DownloadStatusPill({
    super.key,
    required this.status,
    this.compact = false,
  });

  @override
  Widget build(BuildContext context) {
    final statusColor = AppTheme.getStatusColor(status);
    return Container(
      padding: EdgeInsets.symmetric(
        horizontal: compact ? 8 : 10,
        vertical: compact ? 2 : 4,
      ),
      decoration: BoxDecoration(
        color: statusColor.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(compact ? 10 : 12),
        border: Border.all(
          color: statusColor.withValues(alpha: 0.3),
          width: 0.8,
        ),
      ),
      child: Text(
        status.label,
        style: TextStyle(
          fontSize: compact ? 10 : 11,
          fontWeight: FontWeight.w600,
          color: statusColor,
        ),
      ),
    );
  }
}
