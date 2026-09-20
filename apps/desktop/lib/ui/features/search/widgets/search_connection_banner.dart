import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'search_target.dart';
import '../../../../l10n/l10n.dart';

/// Contextual banner above the search form: Kad status summary, global-UDP
/// info, connected-server summary, or a "connect first" warning with a
/// navigation shortcut.
class SearchConnectionBanner extends StatelessWidget {
  final SearchTarget target;
  final ServerSessionModel session;
  final KadStatusModel kadStatus;
  final bool isConnected;
  final VoidCallback onGoToServers;
  final VoidCallback onGoToKad;

  const SearchConnectionBanner({
    super.key,
    required this.target,
    required this.session,
    required this.kadStatus,
    required this.isConnected,
    required this.onGoToServers,
    required this.onGoToKad,
  });

  @override
  Widget build(BuildContext context) {
    if (target == SearchTarget.kad) {
      if (kadStatus.isUdpActive) {
        return _banner(
          bg: AppColors.tintBlue.withAlpha(20),
          border: AppColors.accentLightBlue.withAlpha(80),
          icon: const Icon(
            Icons.hub_rounded,
            color: AppColors.accentLightBlue,
            size: 18,
          ),
          text:
              'Kad DHT Actif • Contacts: ${kadStatus.totalContacts} '
              '(${kadStatus.activeBuckets} k-buckets) • Bind: '
              '${kadStatus.udpBindAddress ?? "Inconnu"} • Profil: '
              '${kadStatus.networkProfile}',
          textColor: AppColors.textSecondary,
        );
      }
      return _banner(
        bg: AppColors.warning.withAlpha(20),
        border: AppColors.accentAmber.withAlpha(80),
        icon: const Icon(
          Icons.warning_amber_rounded,
          color: AppColors.accentAmber,
          size: 20,
        ),
        text:
            'Le réseau Kad DHT n\'est pas encore actif. Démarrez le socket '
            'Kad ou effectuez un bootstrap.',
        textColor: AppColors.accentAmber,
        action: _navButton(
          icon: Icons.hub_rounded,
          label: context.l10n.allerAKad,
          onPressed: onGoToKad,
        ),
      );
    }

    // Global UDP search targets the known-server list; a TCP connection is
    // helpful (the connected server is queried too) but not required.
    if (target == SearchTarget.global && !isConnected) {
      return _banner(
        bg: AppColors.tintBlue.withAlpha(20),
        border: AppColors.accentLightBlue.withAlpha(80),
        icon: const Icon(
          Icons.public_rounded,
          color: AppColors.accentLightBlue,
          size: 18,
        ),
        text:
            'Recherche globale UDP — interroge tous les serveurs connus '
            '(REQ3 si le serveur supporte EXT_GETFILES + LARGEFILES).',
        textColor: AppColors.textSecondary,
      );
    }

    if (isConnected) {
      return _banner(
        bg: AppColors.tintGreen.withAlpha(20),
        border: AppColors.positive.withAlpha(80),
        icon: const Icon(
          Icons.cloud_done_rounded,
          color: AppColors.positive,
          size: 18,
        ),
        text:
            'Connecté à ${session.serverName ?? session.serverAddress} '
            '(${session.serverAddress})'
            ' • Client ID: '
            '${session.clientId != null ? "0x${session.clientId!.toRadixString(16).padLeft(8, '0')}" : "Inconnu"} '
            '(${session.isHighId ? "HighID" : "LowID"})',
        textColor: AppColors.textSecondary,
      );
    }

    return Container(
      margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: AppColors.warning.withAlpha(20),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppColors.accentAmber.withAlpha(80)),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isNarrow = constraints.maxWidth < 650;
          final navBtn = _navButton(
            icon: Icons.dns_rounded,
            label: context.l10n.allerAuxServeurs,
            onPressed: onGoToServers,
          );

          if (isNarrow) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Icon(
                      Icons.warning_amber_rounded,
                      color: AppColors.accentAmber,
                      size: 20,
                    ),
                    SizedBox(width: 8),
                    Expanded(
                      child: Text(context.l10n.nonConnecteAUnServeur,
                        style: TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.bold,
                          color: AppColors.accentAmber,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 6),
                Text(context.l10n.connectezVousDAbordPour,
                  style: TextStyle(fontSize: 12, color: AppColors.accentAmber),
                ),
                const SizedBox(height: 8),
                navBtn,
              ],
            );
          }

          return Row(
            children: [
              const Icon(
                Icons.warning_amber_rounded,
                color: AppColors.accentAmber,
                size: 20,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Text(context.l10n.vousNEtesActuellementPas,
                  style: TextStyle(fontSize: 13, color: AppColors.accentAmber),
                ),
              ),
              const SizedBox(width: 10),
              navBtn,
            ],
          );
        },
      ),
    );
  }

  Widget _banner({
    required Color bg,
    required Color border,
    required Widget icon,
    required String text,
    required Color textColor,
    Widget? action,
  }) {
    return Container(
      margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: border),
      ),
      child: Row(
        children: [
          icon,
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              text,
              style: TextStyle(fontSize: 13, color: textColor),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          if (action != null) ...[const SizedBox(width: 10), action],
        ],
      ),
    );
  }

  Widget _navButton({
    required IconData icon,
    required String label,
    required VoidCallback onPressed,
  }) {
    return ElevatedButton.icon(
      icon: Icon(icon, size: 14),
      label: Text(label),
      style: ElevatedButton.styleFrom(
        backgroundColor: AppColors.warningShade700,
        foregroundColor: AppColors.textPrimary,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        textStyle: const TextStyle(fontSize: 12),
      ),
      onPressed: onPressed,
    );
  }
}
