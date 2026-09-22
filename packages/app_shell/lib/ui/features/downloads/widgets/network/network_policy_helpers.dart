import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../l10n/l10n.dart';

/// Human description of a [NetworkProfile] shown under its name.
String networkProfileDescription(BuildContext context, NetworkProfile profile) {
  switch (profile) {
    case NetworkProfile.localLab:
      return context.l10n.netProfileLocalLabDesc;
    case NetworkProfile.publicEd2kPreview:
      return context.l10n.netProfileEd2kDesc;
    case NetworkProfile.publicEd2kKadPreview:
      return context.l10n.netProfileEd2kKadDesc;
  }
}

/// `label ........ value` quota row used in the limits card.
class NetworkQuotaRow extends StatelessWidget {
  final String label;
  final String value;

  const NetworkQuotaRow({
    super.key,
    required this.label,
    required this.value,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Expanded(
            child: Text(
              label,
              style: const TextStyle(
                fontSize: 11,
                color: AppColors.textSoft,
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          const SizedBox(width: 6),
          Text(
            value,
            style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.bold,
              color: AppColors.textPrimary,
            ),
          ),
        ],
      ),
    );
  }
}
