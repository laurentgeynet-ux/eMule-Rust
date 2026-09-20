import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Filter-status badge shared by the servers data table and the mobile
/// cards list: blocked by IP filter / disabled / allowed.
class ServerStatusBadge extends StatelessWidget {
  final ServerFilterStatus filterStatus;
  final bool compact;

  const ServerStatusBadge({
    super.key,
    required this.filterStatus,
    this.compact = false,
  });

  @override
  Widget build(BuildContext context) {
    final isBlocked = filterStatus == ServerFilterStatus.blockedByFilter;
    final isDisabled = filterStatus == ServerFilterStatus.disabled;
    final padding = compact
        ? const EdgeInsets.symmetric(horizontal: 7, vertical: 2)
        : const EdgeInsets.symmetric(horizontal: 8, vertical: 3);
    final fontSize = compact ? 10.5 : 11.0;

    if (isBlocked) {
      return _badge(
        padding,
        color: AppColors.tintRed.withAlpha(40),
        border: AppColors.danger.withAlpha(100),
        text: 'Bloque par IP Filter',
        textColor: AppColors.danger,
        fontSize: fontSize,
        bold: true,
      );
    }
    if (isDisabled) {
      return _badge(
        padding,
        color: AppColors.tintGrey.withAlpha(40),
        border: AppColors.tintGrey.withAlpha(100),
        text: 'Desactive',
        textColor: AppColors.textSoft,
        fontSize: fontSize,
      );
    }
    return _badge(
      padding,
      color: AppColors.tintGreen.withAlpha(40),
      border: AppColors.positive.withAlpha(100),
      text: 'Autorise',
      textColor: AppColors.positive,
      fontSize: fontSize,
      bold: true,
    );
  }

  Widget _badge(
    EdgeInsets padding, {
    required Color color,
    required Color border,
    required String text,
    required Color textColor,
    required double fontSize,
    bool bold = false,
  }) {
    return Container(
      padding: padding,
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(4),
        border: Border.all(color: border),
      ),
      child: Text(
        text,
        style: TextStyle(
          color: textColor,
          fontSize: fontSize,
          fontWeight: bold ? FontWeight.bold : FontWeight.normal,
        ),
      ),
    );
  }
}
