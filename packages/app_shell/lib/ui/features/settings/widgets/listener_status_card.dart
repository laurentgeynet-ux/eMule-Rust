// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

/// Inbound eD2K TCP listener card (`listener.status` / `start` / `stop`):
/// bind address, live upload counters and start/stop controls. The
/// listener auto-starts with the network profile — these controls are
/// for manual override (e.g. after a port change).
class ListenerStatusCard extends StatelessWidget {
  final InboundListenerStatusModel status;
  final bool isLoading;
  final VoidCallback onStart;
  final VoidCallback onStop;
  final VoidCallback onRefresh;

  const ListenerStatusCard({
    super.key,
    required this.status,
    required this.isLoading,
    required this.onStart,
    required this.onStop,
    required this.onRefresh,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Card(
      color: AppTheme.darkSurface,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    l10n.listenerTitle,
                    style: const TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                      color: AppColors.textPrimary,
                    ),
                  ),
                ),
                if (isLoading)
                  const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                IconButton(
                  icon: const Icon(Icons.refresh_rounded, size: 16),
                  tooltip: l10n.refresh,
                  padding: EdgeInsets.zero,
                  constraints:
                      const BoxConstraints(minWidth: 28, minHeight: 28),
                  onPressed: isLoading ? null : onRefresh,
                ),
              ],
            ),
            Text(
              l10n.listenerSubtitle,
              style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
            ),
            const SizedBox(height: 10),
            Row(
              children: [
                Icon(
                  status.isListening
                      ? Icons.hearing_rounded
                      : Icons.hearing_disabled_rounded,
                  size: 16,
                  color: status.isListening
                      ? AppColors.positive
                      : AppColors.textMuted,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    status.isListening
                        ? l10n.listenerListening(
                            status.listenAddress ?? '—',
                          )
                        : l10n.listenerStopped,
                    style: TextStyle(
                      fontSize: 12,
                      color: status.isListening
                          ? AppColors.textPrimary
                          : AppColors.textMuted,
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 10),
            Row(
              children: [
                _Metric(
                  label: l10n.listenerActiveUploads,
                  value: '${status.activeUploads}',
                ),
                const SizedBox(width: 16),
                _Metric(
                  label: l10n.listenerQueued,
                  value: '${status.queuedUploads}',
                ),
                const SizedBox(width: 16),
                _Metric(
                  label: l10n.listenerRate,
                  value: Formatters.formatSpeed(status.uploadRateBps),
                ),
                const Spacer(),
                if (status.isListening)
                  OutlinedButton.icon(
                    icon: const Icon(Icons.stop_rounded, size: 14),
                    label: Text(
                      l10n.listenerStop,
                      style: const TextStyle(fontSize: 12),
                    ),
                    style: OutlinedButton.styleFrom(
                      foregroundColor: AppColors.danger,
                      side: BorderSide(
                        color: AppColors.danger.withValues(alpha: 0.6),
                      ),
                      padding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 6,
                      ),
                      visualDensity: VisualDensity.compact,
                    ),
                    onPressed: isLoading ? null : onStop,
                  )
                else
                  FilledButton.icon(
                    icon: const Icon(Icons.play_arrow_rounded, size: 14),
                    label: Text(
                      l10n.listenerStart,
                      style: const TextStyle(fontSize: 12),
                    ),
                    style: FilledButton.styleFrom(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 6,
                      ),
                      visualDensity: VisualDensity.compact,
                    ),
                    onPressed: isLoading ? null : onStart,
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _Metric extends StatelessWidget {
  final String label;
  final String value;

  const _Metric({required this.label, required this.value});

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
          value,
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
