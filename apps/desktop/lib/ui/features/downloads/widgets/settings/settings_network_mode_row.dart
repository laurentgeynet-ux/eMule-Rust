import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Section 4: shortcut row opening the network mode / UPnP dialog.
class SettingsNetworkModeRow extends StatelessWidget {
  final VoidCallback onConfigure;

  const SettingsNetworkModeRow({super.key, required this.onConfigure});

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        const Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Mode Réseau & Traversée Box (UPnP)',
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.bold,
                  color: AppColors.textPrimary,
                ),
              ),
              SizedBox(height: 4),
              Text(
                'Bascule entre laboratoire local et réseau public mondial.',
                style: TextStyle(
                  fontSize: 11,
                  color: AppColors.textMuted,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(width: 10),
        FilledButton.icon(
          icon: const Icon(Icons.public, size: 16),
          label: const Text('Configurer'),
          style: FilledButton.styleFrom(
            backgroundColor: AppColors.warningShade800,
          ),
          onPressed: onConfigure,
        ),
      ],
    );
  }
}
