import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Status card for the ipfilter.dat engine: enabled switch, loaded range
/// count and total blocked hits.
class IpFilterStatusCard extends StatelessWidget {
  final FilterStatusModel? status;
  final ValueChanged<bool> onToggle;

  const IpFilterStatusCard({
    super.key,
    required this.status,
    required this.onToggle,
  });

  @override
  Widget build(BuildContext context) {
    final rangeCount = status?.rangeCount ?? 0;
    final hitsCount = status?.hitsCount ?? 0;
    final enabled = status?.enabled ?? true;

    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: AppTheme.darkBorder),
      ),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(
                      enabled ? 'Filtre actif' : 'Filtre désactivé',
                      style: TextStyle(
                        fontWeight: FontWeight.bold,
                        color: enabled
                            ? AppColors.positive
                            : AppColors.tintGrey,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 6,
                        vertical: 2,
                      ),
                      decoration: BoxDecoration(
                        color: AppColors.textPrimary
                            .withValues(alpha: 0.1),
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(context.l10n.rangesLoaded('$rangeCount'),
                        style: const TextStyle(
                          fontSize: 11,
                          color: AppColors.textSecondary,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 4),
                Text(context.l10n.totalBlockedConnectionAttempts('$hitsCount'),
                  style: const TextStyle(
                    fontSize: 12,
                    color: AppColors.textSoft,
                  ),
                ),
              ],
            ),
          ),
          Switch(
            value: enabled,
            activeThumbColor: AppColors.warning,
            onChanged: onToggle,
          ),
        ],
      ),
    );
  }
}
