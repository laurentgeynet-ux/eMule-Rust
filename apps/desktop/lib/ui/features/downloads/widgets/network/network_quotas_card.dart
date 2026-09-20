import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'network_policy_helpers.dart';

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
          const Text(
            'Active Connections & Limits',
            style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 10),
          NetworkQuotaRow(
            label: 'Peer Sockets',
            value:
                '${policy?.activeConnections.peers ?? 0} / ${policy?.budgets.maxActivePeerConnections ?? 20}',
          ),
          NetworkQuotaRow(
            label: 'Server Sockets',
            value:
                '${policy?.activeConnections.servers ?? 0} / ${policy?.budgets.maxActiveServerConnections ?? 2}',
          ),
          NetworkQuotaRow(
            label: 'Listeners',
            value:
                '${policy?.activeConnections.listeners ?? 0} / ${policy?.budgets.maxActiveListeners ?? 2}',
          ),
          const Divider(height: 16, color: AppTheme.darkBorder),
          NetworkQuotaRow(
            label: 'Connect Timeout',
            value:
                '${(policy?.budgets.connectTimeoutMs ?? 10000) ~/ 1000}s',
          ),
          NetworkQuotaRow(
            label: 'Max Frame Size',
            value:
                '${((policy?.budgets.maxFrameSizeBytes ?? 10485760) / (1024 * 1024)).toStringAsFixed(0)} MiB',
          ),
          NetworkQuotaRow(
            label: 'Max Buffer Memory',
            value:
                '${((policy?.budgets.maxTotalBufferBytes ?? 33554432) / (1024 * 1024)).toStringAsFixed(0)} MiB',
          ),
        ],
      ),
    );
  }
}
