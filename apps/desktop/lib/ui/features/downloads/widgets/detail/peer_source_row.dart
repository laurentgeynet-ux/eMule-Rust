import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'peer_source_status_badge.dart';

/// One peer-source row: status icon, address, status/origin/cooldown
/// badges, last error, and the connect / force / disconnect action.
class PeerSourceRow extends StatelessWidget {
  final PeerSourceModel source;
  final String fileHash;
  final PeerSessionModel session;
  final bool isPeerConnecting;
  final Future<bool> Function(String address, {bool force}) onConnect;
  final void Function(String address) onDisconnect;

  const PeerSourceRow({
    super.key,
    required this.source,
    required this.fileHash,
    required this.session,
    required this.isPeerConnecting,
    required this.onConnect,
    required this.onDisconnect,
  });

  @override
  Widget build(BuildContext context) {
    final src = source;
    final isThisConnected =
        session.peerAddress == src.address &&
        session.status != PeerConnectionStatus.disconnected;

    final nowMs = DateTime.now().millisecondsSinceEpoch;
    final isInCooldown =
        src.cooldownUntilUnixMs != null && src.cooldownUntilUnixMs! > nowMs;
    final cooldownSecondsRemaining = isInCooldown
        ? ((src.cooldownUntilUnixMs! - nowMs) / 1000).ceil()
        : 0;

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      child: Row(
        children: [
          Icon(
            src.status == PeerSourceStatus.filtered
                ? Icons.block
                : src.status == PeerSourceStatus.rejected
                ? Icons.gpp_bad
                : src.status == PeerSourceStatus.failed
                ? Icons.error_outline
                : src.status == PeerSourceStatus.connected
                ? Icons.wifi_tethering
                : Icons.computer,
            size: 16,
            color: src.status == PeerSourceStatus.filtered
                ? AppColors.danger
                : src.status == PeerSourceStatus.rejected
                ? AppColors.accentOrange
                : src.status == PeerSourceStatus.failed
                ? AppColors.danger
                : src.status == PeerSourceStatus.connected
                ? AppColors.accentTeal
                : AppColors.textMuted,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(
                      src.address,
                      style: const TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.w600,
                        color: AppColors.textPrimary,
                        fontFamily: 'monospace',
                      ),
                    ),
                    const SizedBox(width: 8),
                    PeerSourceStatusBadge(status: src.status),
                    if (isInCooldown) ...[
                      const SizedBox(width: 6),
                      Container(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 5,
                          vertical: 1,
                        ),
                        decoration: BoxDecoration(
                          color: AppColors.warning.withValues(alpha: 0.2),
                          borderRadius: BorderRadius.circular(4),
                          border: Border.all(
                            color: AppColors.warning.withValues(alpha: 0.4),
                            width: 0.5,
                          ),
                        ),
                        child: Text(
                          'Cooldown ${cooldownSecondsRemaining}s',
                          style: const TextStyle(
                            fontSize: 9,
                            fontWeight: FontWeight.bold,
                            color: AppColors.accentAmber,
                          ),
                        ),
                      ),
                    ],
                    const SizedBox(width: 6),
                    Text(
                      '(${src.origin.toApiString()})',
                      style: const TextStyle(
                        fontSize: 10,
                        color: AppColors.textFaint,
                      ),
                    ),
                  ],
                ),
                if (src.lastError != null && src.lastError!.isNotEmpty)
                  Text(
                    src.lastError!,
                    style: const TextStyle(
                      fontSize: 10,
                      color: AppColors.danger,
                    ),
                  ),
              ],
            ),
          ),
          if (src.status != PeerSourceStatus.filtered) ...[
            if (isThisConnected)
              OutlinedButton(
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppColors.danger,
                  side: const BorderSide(color: AppColors.danger, width: 0.8),
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  minimumSize: Size.zero,
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                ),
                onPressed: () => onDisconnect(src.address),
                child: const Text(
                  'Déconnecter',
                  style: TextStyle(fontSize: 10),
                ),
              )
            else if (isInCooldown)
              OutlinedButton(
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppColors.accentAmber,
                  side: const BorderSide(
                    color: AppColors.accentAmber,
                    width: 0.8,
                  ),
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  minimumSize: Size.zero,
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                ),
                onPressed: isPeerConnecting
                    ? null
                    : () => onConnect(src.address, force: true),
                child: const Text('Forcer', style: TextStyle(fontSize: 10)),
              )
            else
              ElevatedButton(
                style: ElevatedButton.styleFrom(
                  backgroundColor: AppTheme.primary,
                  padding: const EdgeInsets.symmetric(
                    horizontal: 10,
                    vertical: 4,
                  ),
                  minimumSize: Size.zero,
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                ),
                onPressed: isPeerConnecting
                    ? null
                    : () => onConnect(src.address),
                child: Text(
                  src.status == PeerSourceStatus.failed
                      ? 'Réessayer'
                      : 'Connecter',
                  style: const TextStyle(
                    fontSize: 10,
                    color: AppColors.textPrimary,
                  ),
                ),
              ),
          ],
        ],
      ),
    );
  }
}
