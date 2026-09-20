import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

/// Informational banner: managing the server list opens no outbound
/// connection; IP-filtered addresses are flagged. Shortcut to the network
/// policy dialog.
class ServersSafetyBanner extends StatelessWidget {
  final bool hasConnectionCapability;
  final VoidCallback onShowPolicy;

  const ServersSafetyBanner({
    super.key,
    required this.hasConnectionCapability,
    required this.onShowPolicy,
  });

  @override
  Widget build(BuildContext context) {
    final text = hasConnectionCapability
        ? 'Connexions eD2K disponibles. Vérifiez votre profil réseau (LocalLab vs PublicEd2kPreview) avant de vous connecter. Les adresses dans votre filtre IP sont automatiquement signalées.'
        : 'Avertissement Réseau : Importer ou gérer une liste de serveurs n\'établit aucune connexion réseau sortante. Les adresses figurant dans votre filtre IP actif sont automatiquement signalées.';

    return Container(
      margin: const EdgeInsets.fromLTRB(20, 14, 20, 4),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppColors.tintBlue.withAlpha(25),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppColors.tintBlue.withAlpha(80)),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isNarrow = constraints.maxWidth < 650;

          final securityBtn = OutlinedButton.icon(
            style: OutlinedButton.styleFrom(
              side: BorderSide(color: AppColors.accentBlue.withAlpha(120)),
              foregroundColor: AppColors.accentBlue,
              padding: const EdgeInsets.symmetric(
                horizontal: 10,
                vertical: 6,
              ),
              minimumSize: Size.zero,
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            ),
            icon: const Icon(Icons.security_rounded, size: 14),
            label: Text(context.l10n.securiteProfil,
              style: TextStyle(fontSize: 11),
            ),
            onPressed: onShowPolicy,
          );

          if (isNarrow) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Icon(
                      Icons.shield_outlined,
                      color: AppColors.accentBlue,
                      size: 20,
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Text(
                        text,
                        style: const TextStyle(
                          fontSize: 12,
                          color: AppColors.textSecondary,
                          height: 1.3,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                Align(
                  alignment: Alignment.centerRight,
                  child: securityBtn,
                ),
              ],
            );
          }

          return Row(
            children: [
              const Icon(
                Icons.shield_outlined,
                color: AppColors.accentBlue,
                size: 20,
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  text,
                  style: const TextStyle(
                    fontSize: 12.5,
                    color: AppColors.textSecondary,
                    height: 1.3,
                  ),
                ),
              ),
              const SizedBox(width: 10),
              securityBtn,
            ],
          );
        },
      ),
    );
  }
}
