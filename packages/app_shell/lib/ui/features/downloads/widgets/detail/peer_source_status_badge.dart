import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Status badge for a peer source row (allowed / filtered / rejected /
/// connected / tried / failed / candidate).
class PeerSourceStatusBadge extends StatelessWidget {
  final PeerSourceStatus status;

  const PeerSourceStatusBadge({super.key, required this.status});

  @override
  Widget build(BuildContext context) {
    final (bg, fg, text) = switch (status) {
      PeerSourceStatus.allowed => (
        AppColors.positive.withValues(alpha: 0.15),
        AppColors.positive,
        'Autorisé',
      ),
      PeerSourceStatus.filtered => (
        AppColors.danger.withValues(alpha: 0.15),
        AppColors.danger,
        'Filtré IP',
      ),
      PeerSourceStatus.rejected => (
        AppColors.accentOrange.withValues(alpha: 0.15),
        AppColors.accentOrange,
        'Rejeté profil',
      ),
      PeerSourceStatus.connected => (
        AppColors.accentTeal.withValues(alpha: 0.15),
        AppColors.accentTeal,
        'Connecté',
      ),
      PeerSourceStatus.tried => (
        AppColors.accentBlue.withValues(alpha: 0.15),
        AppColors.accentBlue,
        'Essayé',
      ),
      PeerSourceStatus.failed => (
        AppColors.tintRed.withValues(alpha: 0.15),
        AppColors.tintRed,
        'Échec',
      ),
      PeerSourceStatus.candidate => (
        AppColors.fillLight,
        AppColors.textSecondary,
        'Candidat',
      ),
    };

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        text,
        style: TextStyle(fontSize: 9, fontWeight: FontWeight.bold, color: fg),
      ),
    );
  }
}
