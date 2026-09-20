import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Sidebar brand block: app icon + name/tagline (hidden in compact mode).
class SidebarBrandHeader extends StatelessWidget {
  final bool isCompact;

  const SidebarBrandHeader({super.key, required this.isCompact});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: EdgeInsets.symmetric(
        horizontal: isCompact ? 10 : 20,
        vertical: 14,
      ),
      decoration: const BoxDecoration(
        border: Border(
          bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: Row(
        mainAxisAlignment: isCompact
            ? MainAxisAlignment.center
            : MainAxisAlignment.start,
        children: [
          Tooltip(
            message: '${AppStrings.appName} - ${AppStrings.appTagline}',
            child: Container(
              padding: const EdgeInsets.all(8),
              decoration: BoxDecoration(
                color: AppTheme.primary.withValues(alpha: 0.2),
                borderRadius: BorderRadius.circular(10),
              ),
              child: const Icon(
                Icons.cloud_download_rounded,
                color: AppTheme.primary,
                size: 26,
              ),
            ),
          ),
          if (!isCompact) ...[
            const SizedBox(width: 12),
            const Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    AppStrings.appName,
                    style: TextStyle(
                      fontSize: 18,
                      fontWeight: FontWeight.bold,
                      letterSpacing: 0.5,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                  Text(
                    AppStrings.appTagline,
                    style: TextStyle(
                      fontSize: 12,
                      color: AppColors.textMuted,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }
}
