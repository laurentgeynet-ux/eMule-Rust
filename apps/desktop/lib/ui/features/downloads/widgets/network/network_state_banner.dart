import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Kill-switch status banner with the Disconnect All / Re-enable action.
class NetworkStateBanner extends StatelessWidget {
  final NetworkSubsystemState state;
  final bool isKillSwitchActive;
  final VoidCallback onDisconnectAll;
  final VoidCallback onReenable;

  const NetworkStateBanner({
    super.key,
    required this.state,
    required this.isKillSwitchActive,
    required this.onDisconnectAll,
    required this.onReenable,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      decoration: BoxDecoration(
        color: isKillSwitchActive
            ? AppColors.tintRed.withValues(alpha: 0.1)
            : AppTheme.darkCard,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(
          color: isKillSwitchActive
              ? AppColors.danger.withValues(alpha: 0.4)
              : AppTheme.darkBorder,
        ),
      ),
      child: Row(
        children: [
          Icon(
            isKillSwitchActive
                ? Icons.highlight_off_rounded
                : Icons.check_circle_outline_rounded,
            color: isKillSwitchActive
                ? AppColors.danger
                : AppColors.positive,
            size: 20,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'Status: ${state.label}',
                  style: TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.bold,
                    color: isKillSwitchActive
                        ? AppColors.danger
                        : AppColors.textPrimary,
                  ),
                ),
                Text(
                  isKillSwitchActive
                      ? 'All socket operations are stopped. Inbound and outbound connections blocked.'
                      : 'Socket permits strictly governed by active profile constraints.',
                  style: const TextStyle(
                    fontSize: 11,
                    color: AppColors.textSoft,
                  ),
                ),
              ],
            ),
          ),
          if (isKillSwitchActive)
            FilledButton.icon(
              key: const Key('reenable_network_button'),
              icon: const Icon(Icons.play_arrow_rounded, size: 16),
              label: const Text('Re-enable'),
              style: FilledButton.styleFrom(
                backgroundColor: AppColors.positiveShade700,
                padding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 8,
                ),
              ),
              onPressed: onReenable,
            )
          else
            FilledButton.icon(
              key: const Key('kill_switch_dialog_button'),
              icon: const Icon(
                Icons.power_settings_new_rounded,
                size: 16,
              ),
              label: const Text('Disconnect All'),
              style: FilledButton.styleFrom(
                backgroundColor: AppColors.dangerShade700,
                padding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 8,
                ),
              ),
              onPressed: onDisconnectAll,
            ),
        ],
      ),
    );
  }
}
