// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'network_policy_helpers.dart';
import '../../../../../l10n/l10n.dart';

/// Right-hand card: active socket counts vs budgets plus frame/buffer
/// limits.
class NetworkQuotasCard extends StatelessWidget {
  final NetworkPolicySnapshot? policy;

  const NetworkQuotasCard({super.key, required this.policy});

  @override
  Widget build(BuildContext context) {
    final policy = this.policy;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: AppTheme.darkBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(context.l10n.activeConnectionsLimits,
            style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 10),
          NetworkQuotaRow(
            label: context.l10n.peerSockets,
            value:
                '${policy?.activeConnections.peers ?? 0} / ${policy?.budgets.maxActivePeerConnections ?? 20}',
          ),
          NetworkQuotaRow(
            label: context.l10n.serverSockets,
            value:
                '${policy?.activeConnections.servers ?? 0} / ${policy?.budgets.maxActiveServerConnections ?? 2}',
          ),
          NetworkQuotaRow(
            label: context.l10n.listeners,
            value:
                '${policy?.activeConnections.listeners ?? 0} / ${policy?.budgets.maxActiveListeners ?? 2}',
          ),
          const Divider(height: 16, color: AppTheme.darkBorder),
          NetworkQuotaRow(
            label: context.l10n.connectTimeout,
            value:
                '${(policy?.budgets.connectTimeoutMs ?? 10000) ~/ 1000}s',
          ),
          NetworkQuotaRow(
            label: context.l10n.maxFrameSize,
            value:
                '${((policy?.budgets.maxFrameSizeBytes ?? 10485760) / (1024 * 1024)).toStringAsFixed(0)} MiB',
          ),
          NetworkQuotaRow(
            label: context.l10n.maxBufferMemory,
            value:
                '${((policy?.budgets.maxTotalBufferBytes ?? 33554432) / (1024 * 1024)).toStringAsFixed(0)} MiB',
          ),
        ],
      ),
    );
  }
}
