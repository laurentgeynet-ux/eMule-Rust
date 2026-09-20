import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../view_models/downloads_view_model.dart';

class NetworkPolicyDialog extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const NetworkPolicyDialog({super.key, required this.viewModel});

  @override
  State<NetworkPolicyDialog> createState() => _NetworkPolicyDialogState();
}

class _NetworkPolicyDialogState extends State<NetworkPolicyDialog> {
  bool _isLoading = false;

  @override
  void initState() {
    super.initState();
    _loadData();
  }

  Future<void> _loadData() async {
    setState(() => _isLoading = true);
    await widget.viewModel.fetchNetworkPolicy();
    await widget.viewModel.fetchNetworkAuditLog(limit: 20);
    if (mounted) {
      setState(() => _isLoading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final policy = widget.viewModel.networkPolicy;
    final isKillSwitchActive = widget.viewModel.isNetworkKillSwitchActive;
    final profile = widget.viewModel.currentNetworkProfile;
    final state = widget.viewModel.currentNetworkState;

    return Dialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 750, maxHeight: 680),
        child: Padding(
          padding: const EdgeInsets.all(22),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Header
              Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: isKillSwitchActive
                          ? Colors.red.withValues(alpha: 0.15)
                          : Colors.cyan.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Icon(
                      isKillSwitchActive ? Icons.shield_rounded : Icons.security_rounded,
                      color: isKillSwitchActive ? Colors.redAccent : Colors.cyanAccent,
                      size: 22,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text(
                          'Network Policy & Defense Guard',
                          style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          'Defensive socket authorization, anti-SSRF isolation, and resource quotas',
                          style: TextStyle(fontSize: 12, color: Colors.white.withValues(alpha: 0.5)),
                        ),
                      ],
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.close_rounded, size: 20),
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                ],
              ),

              const SizedBox(height: 18),

              // Subsystem State & Kill Switch Banner
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
                decoration: BoxDecoration(
                  color: isKillSwitchActive
                      ? Colors.red.withValues(alpha: 0.1)
                      : AppTheme.darkCard,
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(
                    color: isKillSwitchActive
                        ? Colors.redAccent.withValues(alpha: 0.4)
                        : AppTheme.darkBorder,
                  ),
                ),
                child: Row(
                  children: [
                    Icon(
                      isKillSwitchActive
                          ? Icons.highlight_off_rounded
                          : Icons.check_circle_outline_rounded,
                      color: isKillSwitchActive ? Colors.redAccent : Colors.greenAccent,
                      size: 20,
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Status: ${state.label}',
                            style: TextStyle(
                              fontSize: 13,
                              fontWeight: FontWeight.bold,
                              color: isKillSwitchActive ? Colors.redAccent : Colors.white,
                            ),
                          ),
                          Text(
                            isKillSwitchActive
                                ? 'All socket operations are stopped. Inbound and outbound connections blocked.'
                                : 'Socket permits strictly governed by active profile constraints.',
                            style: const TextStyle(fontSize: 11, color: Colors.white60),
                          ),
                        ],
                      ),
                    ),
                    if (isKillSwitchActive)
                      FilledButton.icon(
                        key: const Key('reenable_network_button'),
                        icon: const Icon(Icons.play_arrow_rounded, size: 16),
                        label: const Text('Re-enable'),
                        style: FilledButton.styleFrom(
                          backgroundColor: Colors.green.shade700,
                          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                        ),
                        onPressed: () async {
                          await widget.viewModel.enableNetwork();
                          await _loadData();
                        },
                      )
                    else
                      FilledButton.icon(
                        key: const Key('kill_switch_dialog_button'),
                        icon: const Icon(Icons.power_settings_new_rounded, size: 16),
                        label: const Text('Disconnect All'),
                        style: FilledButton.styleFrom(
                          backgroundColor: Colors.red.shade700,
                          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                        ),
                        onPressed: () async {
                          await widget.viewModel.disconnectAllNetwork();
                          await _loadData();
                        },
                      ),
                  ],
                ),
              ),

              const SizedBox(height: 16),

              // Profile Selector & Quotas Row
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // Profile Selector
                  Expanded(
                    flex: 3,
                    child: Container(
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
                            'Active Connectivity Profile',
                            style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold),
                          ),
                          const SizedBox(height: 8),
                          for (final p in NetworkProfile.values)
                            InkWell(
                              key: Key('profile_${p.toKebabCase()}'),
                              borderRadius: BorderRadius.circular(8),
                              onTap: () async {
                                if (p != profile) {
                                  await widget.viewModel.setNetworkProfile(p);
                                  await _loadData();
                                }
                              },
                              child: Padding(
                                padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 4),
                                child: Row(
                                  children: [
                                    Icon(
                                      p == profile
                                          ? Icons.radio_button_checked
                                          : Icons.radio_button_off,
                                      size: 16,
                                      color: p == profile ? Colors.cyanAccent : Colors.white38,
                                    ),
                                    const SizedBox(width: 8),
                                    Expanded(
                                      child: Column(
                                        crossAxisAlignment: CrossAxisAlignment.start,
                                        children: [
                                          Text(
                                            p.label,
                                            style: TextStyle(
                                              fontSize: 12,
                                              fontWeight: FontWeight.w600,
                                              color: p == profile ? Colors.cyanAccent : Colors.white,
                                            ),
                                          ),
                                          Text(
                                            _profileDescription(p),
                                            style: TextStyle(
                                              fontSize: 10,
                                              color: Colors.white.withValues(alpha: 0.5),
                                            ),
                                          ),
                                        ],
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            ),
                        ],
                      ),
                    ),
                  ),

                  const SizedBox(width: 12),

                  // Quotas & Active Sockets
                  Expanded(
                    flex: 2,
                    child: Container(
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
                          _buildQuotaRow(
                            'Peer Sockets',
                            '${policy?.activeConnections.peers ?? 0} / ${policy?.budgets.maxActivePeerConnections ?? 20}',
                          ),
                          _buildQuotaRow(
                            'Server Sockets',
                            '${policy?.activeConnections.servers ?? 0} / ${policy?.budgets.maxActiveServerConnections ?? 2}',
                          ),
                          _buildQuotaRow(
                            'Listeners',
                            '${policy?.activeConnections.listeners ?? 0} / ${policy?.budgets.maxActiveListeners ?? 2}',
                          ),
                          const Divider(height: 16, color: AppTheme.darkBorder),
                          _buildQuotaRow(
                            'Connect Timeout',
                            '${(policy?.budgets.connectTimeoutMs ?? 10000) ~/ 1000}s',
                          ),
                          _buildQuotaRow(
                            'Max Frame Size',
                            '${((policy?.budgets.maxFrameSizeBytes ?? 10485760) / (1024 * 1024)).toStringAsFixed(0)} MiB',
                          ),
                          _buildQuotaRow(
                            'Max Buffer Memory',
                            '${((policy?.budgets.maxTotalBufferBytes ?? 33554432) / (1024 * 1024)).toStringAsFixed(0)} MiB',
                          ),
                        ],
                      ),
                    ),
                  ),
                ],
              ),

              const SizedBox(height: 16),

              // Redacted Audit Log
              const Text(
                'Redacted Network Audit Log (Recent Decisions)',
                style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold),
              ),
              const SizedBox(height: 6),

              Expanded(
                child: Container(
                  decoration: BoxDecoration(
                    color: AppTheme.darkCard,
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(color: AppTheme.darkBorder),
                  ),
                  child: _isLoading
                      ? const Center(child: CircularProgressIndicator())
                      : widget.viewModel.networkAuditLog.isEmpty
                          ? const Center(
                              child: Text(
                                'No network events recorded yet.',
                                style: TextStyle(color: Colors.white38, fontSize: 12),
                              ),
                            )
                          : ListView.separated(
                              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                              itemCount: widget.viewModel.networkAuditLog.length,
                              separatorBuilder: (_, _) =>
                                  const Divider(height: 1, color: AppTheme.darkBorder),
                              itemBuilder: (context, index) {
                                final entry = widget.viewModel.networkAuditLog[index];
                                final isAllowed = entry.decision.toLowerCase() == 'allowed';
                                return Padding(
                                  padding: const EdgeInsets.symmetric(vertical: 6),
                                  child: Row(
                                    children: [
                                      Icon(
                                        isAllowed
                                            ? Icons.check_circle_outline
                                            : Icons.block_flipped,
                                        size: 15,
                                        color: isAllowed ? Colors.greenAccent : Colors.redAccent,
                                      ),
                                      const SizedBox(width: 8),
                                      Text(
                                        entry.eventType,
                                        style: const TextStyle(
                                          fontSize: 11,
                                          fontWeight: FontWeight.bold,
                                        ),
                                      ),
                                      const SizedBox(width: 8),
                                      if (entry.redactedTarget != null)
                                        Container(
                                          padding: const EdgeInsets.symmetric(
                                            horizontal: 5,
                                            vertical: 2,
                                          ),
                                          decoration: BoxDecoration(
                                            color: Colors.black26,
                                            borderRadius: BorderRadius.circular(4),
                                          ),
                                          child: Text(
                                            entry.redactedTarget!,
                                            style: const TextStyle(
                                              fontSize: 10,
                                              fontFamily: 'monospace',
                                              color: Colors.white70,
                                            ),
                                          ),
                                        ),
                                      const SizedBox(width: 8),
                                      Expanded(
                                        child: Text(
                                          entry.reason ?? '',
                                          style: const TextStyle(
                                            fontSize: 10,
                                            color: Colors.white54,
                                          ),
                                          overflow: TextOverflow.ellipsis,
                                        ),
                                      ),
                                    ],
                                  ),
                                );
                              },
                            ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildQuotaRow(String label, String value) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Expanded(
            child: Text(
              label,
              style: const TextStyle(fontSize: 11, color: Colors.white60),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          const SizedBox(width: 6),
          Text(
            value,
            style: const TextStyle(fontSize: 11, fontWeight: FontWeight.bold, color: Colors.white),
          ),
        ],
      ),
    );
  }

  String _profileDescription(NetworkProfile profile) {
    switch (profile) {
      case NetworkProfile.localLab:
        return 'Confined to loopback (127.0.0.1 / ::1). Zero external sockets permitted.';
      case NetworkProfile.publicEd2kPreview:
        return 'Outbound TCP to public eD2K peers/servers. Rejects private LAN & loopback (anti-SSRF).';
      case NetworkProfile.publicEd2kKadPreview:
        return 'Preview WAN with UDP support for Kademlia routing. Strict address validation.';
    }
  }
}
