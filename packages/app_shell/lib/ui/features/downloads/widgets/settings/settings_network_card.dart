import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Section 3: read-only card describing the active network profile
/// (confined lab vs public WAN/Kad).
class SettingsNetworkCard extends StatelessWidget {
  final NetworkProfile profile;

  const SettingsNetworkCard({super.key, required this.profile});

  @override
  Widget build(BuildContext context) {
    final isLab = profile == NetworkProfile.localLab;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Icon(
            Icons.shield_outlined,
            color: AppColors.accentTeal,
            size: 18,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  !isLab
                      ? 'Mode Réseau Public Ouvert (WAN & Kad)'
                      : 'Mode Laboratoire Confiné (127.0.0.1)',
                  style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.bold,
                    color: AppColors.textPrimary,
                  ),
                ),
                const SizedBox(height: 4),
                Text(
                  !isLab
                      ? 'Emule-Sion est connecté aux réseaux eD2K et Kademlia mondiaux avec ports ouverts et protection Anti-SSRF active. Les transferts actifs sont sécurisés dans data/temp/.'
                      : 'Emule-Sion fonctionne exclusivement sur la boucle locale (127.0.0.1). Aucune connexion publique entrante ou sortante n\'est effectuée.',
                  style: const TextStyle(
                    fontSize: 11,
                    color: AppColors.textMuted,
                    height: 1.4,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
