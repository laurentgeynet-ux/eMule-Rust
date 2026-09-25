// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

/// Secure identity card (`identity.status` / `identity.rotate`):
/// RSA-384 key fingerprint, SUI handshake counters and the manual
/// keypair rotation action — eMule Preferences → Security.
class IdentityStatusCard extends StatelessWidget {
  final SecureIdentityStatusModel? status;
  final bool isLoading;
  final VoidCallback onRotate;
  final VoidCallback onRefresh;

  const IdentityStatusCard({
    super.key,
    required this.status,
    required this.isLoading,
    required this.onRotate,
    required this.onRefresh,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final s = status;
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
                    l10n.identityTitle,
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
                  tooltip: l10n.identityRefresh,
                  padding: EdgeInsets.zero,
                  constraints:
                      const BoxConstraints(minWidth: 28, minHeight: 28),
                  onPressed: isLoading ? null : onRefresh,
                ),
              ],
            ),
            Text(
              l10n.identitySubtitle,
              style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
            ),
            const SizedBox(height: 10),
            if (s == null)
              const SizedBox.shrink()
            else ...[
              Row(
                children: [
                  Icon(
                    s.hasKey
                        ? Icons.verified_user_rounded
                        : Icons.gpp_bad_outlined,
                    size: 16,
                    color: s.hasKey
                        ? AppColors.positive
                        : AppColors.danger,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      s.hasKey
                          ? l10n.identityKeyBits(s.keyBits ?? 0)
                          : l10n.identityNoKey,
                      style: TextStyle(
                        fontSize: 12,
                        color: s.hasKey
                            ? AppColors.textPrimary
                            : AppColors.danger,
                      ),
                    ),
                  ),
                ],
              ),
              if (s.pubkeyHash != null) ...[
                const SizedBox(height: 8),
                InkWell(
                  borderRadius: BorderRadius.circular(4),
                  onTap: () => Clipboard.setData(
                    ClipboardData(text: s.pubkeyHash!),
                  ),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(vertical: 2),
                    child: Row(
                      children: [
                        Expanded(
                          child: Text(
                            '${l10n.identityPubkeyHash} : ${s.pubkeyHash}',
                            style: const TextStyle(
                              fontFamily: 'monospace',
                              fontSize: 11,
                              color: AppColors.textSecondary,
                            ),
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        const Icon(
                          Icons.copy_rounded,
                          size: 12,
                          color: AppColors.textMuted,
                        ),
                      ],
                    ),
                  ),
                ),
              ],
              if (s.quarantinedKey != null) ...[
                const SizedBox(height: 8),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Icon(
                      Icons.warning_amber_rounded,
                      size: 14,
                      color: AppColors.accentAmber,
                    ),
                    const SizedBox(width: 6),
                    Expanded(
                      child: Text(
                        l10n.identityQuarantined(s.quarantinedKey!),
                        style: const TextStyle(
                          fontSize: 11,
                          color: AppColors.accentAmber,
                        ),
                      ),
                    ),
                  ],
                ),
              ],
              if (s.sui != null) ...[
                const SizedBox(height: 8),
                Text(
                  l10n.identitySuiStats(
                    s.sui!.signaturesVerified,
                    s.sui!.signaturesRejected,
                    s.sui!.malformedPackets,
                  ),
                  style: const TextStyle(
                    fontSize: 10,
                    color: AppColors.textMuted,
                  ),
                ),
              ],
              const SizedBox(height: 10),
              Align(
                alignment: Alignment.centerRight,
                child: OutlinedButton.icon(
                  icon: const Icon(Icons.autorenew_rounded, size: 14),
                  label: Text(
                    l10n.identityRotate,
                    style: const TextStyle(fontSize: 12),
                  ),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppColors.accentAmber,
                    side: BorderSide(
                      color: AppColors.accentAmber.withValues(alpha: 0.6),
                    ),
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 6,
                    ),
                    visualDensity: VisualDensity.compact,
                  ),
                  onPressed: isLoading ? null : onRotate,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
