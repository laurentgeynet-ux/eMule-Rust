// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

/// UPnP IGD status card: detected gateway, WAN IP and TCP/UDP mapping
/// state for the eD2K/Kad ports.
class UpnpStatusCard extends StatelessWidget {
  final UpnpStatusModel status;
  final bool isLoading;
  final VoidCallback onRefresh;

  const UpnpStatusCard({
    super.key,
    required this.status,
    required this.isLoading,
    required this.onRefresh,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(context.l10n.statutUpnpIgdBoxRouteur,
          style: TextStyle(
            fontSize: 13,
            fontWeight: FontWeight.bold,
            color: AppColors.textPrimary,
          ),
        ),
        const SizedBox(height: 8),
        Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: AppTheme.darkCard,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: AppTheme.darkBorder),
          ),
          child: Column(
            children: [
              _UpnpRow(
                label: context.l10n.passerelleBoxDetectee,
                child: Text(
                  status.available
                      ? (status.gatewayAddr ?? context.l10n.upnpGatewayConnected)
                      : context.l10n.upnpGatewayAbsent,
                  style: TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                    color: status.available
                        ? AppColors.positive
                        : AppColors.accentOrange,
                  ),
                ),
              ),
              const Divider(color: AppTheme.darkBorder, height: 16),
              _UpnpRow(
                label: context.l10n.adresseIpExterneWan,
                child: Text(
                  status.externalIp ?? context.l10n.upnpIpUnknown,
                  style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.bold,
                    color: AppColors.accentTeal,
                  ),
                ),
              ),
              const Divider(color: AppTheme.darkBorder, height: 16),
              _UpnpRow(
                label: context.l10n.portTcpEd2k('${status.tcpPort}'),
                child: _MappedValue(
                  mapped: status.tcpMapped,
                  mappedLabel: context.l10n.upnpMappedHighId,
                ),
              ),
              const SizedBox(height: 8),
              _UpnpRow(
                label: context.l10n.portUdpKad('${status.udpPort}'),
                child: _MappedValue(
                  mapped: status.udpMapped,
                  mappedLabel: context.l10n.upnpMappedOpen,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 10),
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            OutlinedButton.icon(
              icon: const Icon(Icons.refresh, size: 14),
              label: Text(context.l10n.testerRafraichirUpnp),
              style: OutlinedButton.styleFrom(
                foregroundColor: AppColors.textSecondary,
                side: const BorderSide(color: AppTheme.darkBorder),
                padding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 8,
                ),
              ),
              onPressed: isLoading ? null : onRefresh,
            ),
          ],
        ),
      ],
    );
  }
}

class _UpnpRow extends StatelessWidget {
  final String label;
  final Widget child;

  const _UpnpRow({required this.label, required this.child});

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(
          label,
          style: const TextStyle(
            fontSize: 12,
            color: AppColors.textSecondary,
          ),
        ),
        child,
      ],
    );
  }
}

class _MappedValue extends StatelessWidget {
  final bool mapped;
  final String mappedLabel;

  const _MappedValue({required this.mapped, required this.mappedLabel});

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Icon(
          mapped ? Icons.check_circle : Icons.cancel_outlined,
          size: 14,
          color: mapped ? AppColors.positive : AppColors.textFaint,
        ),
        const SizedBox(width: 4),
        Text(
          mapped ? mappedLabel : context.l10n.upnpNotMapped,
          style: TextStyle(
            fontSize: 12,
            color: mapped ? AppColors.positive : AppColors.textSoft,
          ),
        ),
      ],
    );
  }
}
