import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

/// Card describing the currently active network profile
/// (public WAN vs confined lab).
class NetworkModeStatusCard extends StatelessWidget {
  final NetworkProfile profile;

  const NetworkModeStatusCard({super.key, required this.profile});

  @override
  Widget build(BuildContext context) {
    final isPublic = profile == NetworkProfile.publicEd2kKadPreview;
    final accent = isPublic ? AppColors.warning : AppColors.accentBlue;

    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(context.l10n.profilReseauActuel,
                style: TextStyle(
                  color: AppColors.textPrimary.withValues(alpha: 0.7),
                  fontSize: 13,
                ),
              ),
              Chip(
                label: Text(
                  isPublic
                      ? 'RÉSEAU PUBLIC (WAN)'
                      : 'LABORATOIRE (127.0.0.1)',
                  style: TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.bold,
                    color: accent,
                  ),
                ),
                backgroundColor: accent.withValues(alpha: 0.15),
                side: BorderSide(color: accent.withValues(alpha: 0.4)),
                padding: EdgeInsets.zero,
              ),
            ],
          ),
          const SizedBox(height: 8),
          Text(
            isPublic
                ? 'Votre client peut contacter des serveurs eD2K publics, des pairs externes et la DHT Kademlia mondiale.'
                : 'Toutes les sockets sont strictement confinées à la boucle locale 127.0.0.1. Zéro paquet sur Internet.',
            style: const TextStyle(
              fontSize: 12,
              color: AppColors.textSoft,
              height: 1.4,
            ),
          ),
        ],
      ),
    );
  }
}
