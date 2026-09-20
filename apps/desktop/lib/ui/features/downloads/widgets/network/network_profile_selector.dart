import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'network_policy_helpers.dart';
import '../../../../../l10n/l10n.dart';

/// Radio-style list to switch the active connectivity profile.
class NetworkProfileSelector extends StatelessWidget {
  final NetworkProfile active;
  final ValueChanged<NetworkProfile> onSelected;

  const NetworkProfileSelector({
    super.key,
    required this.active,
    required this.onSelected,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: AppTheme.darkBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(context.l10n.activeConnectivityProfile,
            style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 8),
          for (final p in NetworkProfile.values)
            InkWell(
              key: Key('profile_${p.toKebabCase()}'),
              borderRadius: BorderRadius.circular(8),
              onTap: () {
                if (p != active) onSelected(p);
              },
              child: Padding(
                padding: const EdgeInsets.symmetric(
                  vertical: 6,
                  horizontal: 4,
                ),
                child: Row(
                  children: [
                    Icon(
                      p == active
                          ? Icons.radio_button_checked
                          : Icons.radio_button_off,
                      size: 16,
                      color: p == active
                          ? AppColors.accentCyan
                          : AppColors.textFaint,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            p.label,
                            style: TextStyle(
                              fontSize: 12,
                              fontWeight: FontWeight.w600,
                              color: p == active
                                  ? AppColors.accentCyan
                                  : AppColors.textPrimary,
                            ),
                          ),
                          Text(
                            networkProfileDescription(p),
                            style: TextStyle(
                              fontSize: 10,
                              color: AppColors.textPrimary
                                  .withValues(alpha: 0.5),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }
}
