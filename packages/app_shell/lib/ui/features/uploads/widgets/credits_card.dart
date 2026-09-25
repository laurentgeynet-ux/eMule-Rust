// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

/// Peer-credit store card (`credits.list` + `credits.stats`): the
/// persistent upload/download ratios peers earned under secure
/// identification — eMule `CClientCreditsList`. Peers with a positive
/// balance get a queue score multiplier (shown per-session in the
/// uploads table).
class CreditsCard extends StatelessWidget {
  final List<PeerCreditModel> credits;
  final PeerCreditStatsModel? stats;
  final VoidCallback onRefresh;

  const CreditsCard({
    super.key,
    required this.credits,
    required this.stats,
    required this.onRefresh,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Container(
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  l10n.creditsTitle,
                  style: const TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: AppColors.textPrimary,
                  ),
                ),
              ),
              IconButton(
                icon: const Icon(Icons.refresh_rounded, size: 16),
                tooltip: l10n.refresh,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
                onPressed: onRefresh,
              ),
            ],
          ),
          Text(
            l10n.creditsSubtitle,
            style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
          ),
          const SizedBox(height: 10),
          if (stats != null)
            Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Wrap(
                spacing: 16,
                runSpacing: 4,
                children: [
                  _Stat(
                    label: l10n.creditsPeersCount(stats!.totalPeers),
                    sub: l10n.creditsActiveCount(stats!.activeCreditedPeers),
                  ),
                  _Stat(
                    label: l10n.creditsTotalUploaded,
                    sub: Formatters.formatBytes(stats!.totalUploadedBytes),
                  ),
                  _Stat(
                    label: l10n.creditsTotalDownloaded,
                    sub: Formatters.formatBytes(stats!.totalDownloadedBytes),
                  ),
                ],
              ),
            ),
          if (credits.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 12),
              child: Center(
                child: Text(
                  l10n.creditsEmpty,
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                    fontSize: 12,
                    color: AppColors.textSecondary,
                  ),
                ),
              ),
            )
          else ...[
            _HeaderRow(),
            const Divider(height: 1, color: AppTheme.darkBorder),
            ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 240),
              child: ListView.separated(
                shrinkWrap: true,
                itemCount: credits.length,
                separatorBuilder: (_, __) =>
                    const Divider(height: 1, color: AppTheme.darkBorder),
                itemBuilder: (context, i) =>
                    _CreditRow(credit: credits[i]),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _Stat extends StatelessWidget {
  final String label;
  final String sub;

  const _Stat({required this.label, required this.sub});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
        ),
        Text(
          sub,
          style: const TextStyle(
            fontSize: 12,
            fontWeight: FontWeight.w600,
            color: AppColors.textPrimary,
          ),
        ),
      ],
    );
  }
}

class _HeaderRow extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    const style = TextStyle(
      fontSize: 10,
      fontWeight: FontWeight.w700,
      color: AppColors.textSecondary,
    );
    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Row(
        children: [
          Expanded(flex: 4, child: Text(l10n.creditsColPeer, style: style)),
          Expanded(
            flex: 2,
            child: Text(
              l10n.creditsColUploaded,
              style: style,
              textAlign: TextAlign.right,
            ),
          ),
          Expanded(
            flex: 2,
            child: Text(
              l10n.creditsColDownloaded,
              style: style,
              textAlign: TextAlign.right,
            ),
          ),
          Expanded(
            flex: 1,
            child: Text(
              l10n.creditsColMultiplier,
              style: style,
              textAlign: TextAlign.right,
            ),
          ),
          Expanded(
            flex: 2,
            child: Text(
              l10n.creditsColLastSeen,
              style: style,
              textAlign: TextAlign.right,
            ),
          ),
        ],
      ),
    );
  }
}

class _CreditRow extends StatelessWidget {
  final PeerCreditModel credit;

  const _CreditRow({required this.credit});

  @override
  Widget build(BuildContext context) {
    const primary = TextStyle(fontSize: 11, color: AppColors.textPrimary);
    final lastSeen = credit.lastSeenUnixMs > 0
        ? DateTime.fromMillisecondsSinceEpoch(credit.lastSeenUnixMs)
        : null;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 5),
      child: Row(
        children: [
          Expanded(
            flex: 4,
            child: Tooltip(
              message: credit.pubkeyHash,
              child: Text(
                credit.pubkeyHash,
                style: const TextStyle(
                  fontFamily: 'monospace',
                  fontSize: 10,
                  color: AppColors.textSecondary,
                ),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ),
          Expanded(
            flex: 2,
            child: Text(
              Formatters.formatBytes(credit.uploadedBytes),
              style: primary,
              textAlign: TextAlign.right,
            ),
          ),
          Expanded(
            flex: 2,
            child: Text(
              Formatters.formatBytes(credit.downloadedBytes),
              style: primary,
              textAlign: TextAlign.right,
            ),
          ),
          Expanded(
            flex: 1,
            child: Text(
              'x${credit.creditMultiplier.toStringAsFixed(1)}',
              style: TextStyle(
                fontSize: 11,
                color: credit.creditMultiplier > 1
                    ? AppColors.positive
                    : AppColors.textMuted,
              ),
              textAlign: TextAlign.right,
            ),
          ),
          Expanded(
            flex: 2,
            child: Text(
              lastSeen == null
                  ? '—'
                  : '${lastSeen.day.toString().padLeft(2, '0')}/'
                        '${lastSeen.month.toString().padLeft(2, '0')} '
                        '${lastSeen.hour.toString().padLeft(2, '0')}:'
                        '${lastSeen.minute.toString().padLeft(2, '0')}',
              style: const TextStyle(
                fontSize: 10,
                color: AppColors.textMuted,
              ),
              textAlign: TextAlign.right,
            ),
          ),
        ],
      ),
    );
  }
}
