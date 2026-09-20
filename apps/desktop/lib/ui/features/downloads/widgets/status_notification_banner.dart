import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Thin teal banner for transient status messages (e.g. "Download added").
/// Rendered by the app shell above the routed content.
class StatusNotificationBanner extends StatelessWidget {
  final String message;
  final VoidCallback onClose;

  const StatusNotificationBanner({
    super.key,
    required this.message,
    required this.onClose,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      decoration: BoxDecoration(
        color: AppColors.lab.withValues(alpha: 0.15),
        border: const Border(
          bottom: BorderSide(color: AppColors.lab, width: 0.5),
        ),
      ),
      child: Row(
        children: [
          const Icon(Icons.check_circle_outline, size: 16, color: AppColors.success),
          const SizedBox(width: AppSpacing.sm),
          Expanded(
            child: Text(
              message,
              style: const TextStyle(fontSize: 12, color: AppColors.success),
            ),
          ),
          InkWell(
            onTap: onClose,
            child: const Padding(
              padding: EdgeInsets.all(AppSpacing.xs),
              child: Icon(Icons.close, size: 14, color: AppColors.success),
            ),
          ),
        ],
      ),
    );
  }
}
