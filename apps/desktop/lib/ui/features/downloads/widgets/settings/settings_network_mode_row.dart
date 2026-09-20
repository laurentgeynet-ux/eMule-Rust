import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Section 4: shortcut row opening the network mode / UPnP dialog.
class SettingsNetworkModeRow extends StatelessWidget {
  final VoidCallback onConfigure;

  const SettingsNetworkModeRow({super.key, required this.onConfigure});

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(context.l10n.modeReseauTraverseeBoxUpnp,
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.bold,
                  color: AppColors.textPrimary,
                ),
              ),
              SizedBox(height: 4),
              Text(context.l10n.basculeEntreLaboratoireLocalEt,
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
          label: Text(context.l10n.configurer),
          style: FilledButton.styleFrom(
            backgroundColor: AppColors.warningShade800,
          ),
          onPressed: onConfigure,
        ),
      ],
    );
  }
}
