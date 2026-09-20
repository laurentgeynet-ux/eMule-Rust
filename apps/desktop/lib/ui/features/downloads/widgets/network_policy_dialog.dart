import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../view_models/downloads_view_model.dart';
import 'network/network_audit_log_list.dart';
import 'network/network_profile_selector.dart';
import 'network/network_quotas_card.dart';
import 'network/network_state_banner.dart';

/// Network policy & defense guard: kill switch, connectivity profile,
/// socket budgets and the redacted audit log.
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
    final isKillSwitchActive =
        widget.viewModel.isNetworkKillSwitchActive;
    final profile = widget.viewModel.currentNetworkProfile;

    return Dialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 750, maxHeight: 680),
        child: Padding(
          padding: const EdgeInsets.all(22),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: isKillSwitchActive
                          ? AppColors.tintRed.withValues(alpha: 0.15)
                          : AppColors.tintCyan.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Icon(
                      isKillSwitchActive
                          ? Icons.shield_rounded
                          : Icons.security_rounded,
                      color: isKillSwitchActive
                          ? AppColors.danger
                          : AppColors.accentCyan,
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
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          'Defensive socket authorization, anti-SSRF isolation, and resource quotas',
                          style: TextStyle(
                            fontSize: 12,
                            color: AppColors.textPrimary
                                .withValues(alpha: 0.5),
                          ),
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
              NetworkStateBanner(
                state: widget.viewModel.currentNetworkState,
                isKillSwitchActive: isKillSwitchActive,
                onDisconnectAll: () async {
                  await widget.viewModel.disconnectAllNetwork();
                  await _loadData();
                },
                onReenable: () async {
                  await widget.viewModel.enableNetwork();
                  await _loadData();
                },
              ),
              const SizedBox(height: 16),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(
                    flex: 3,
                    child: NetworkProfileSelector(
                      active: profile,
                      onSelected: (p) async {
                        await widget.viewModel.setNetworkProfile(p);
                        await _loadData();
                      },
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    flex: 2,
                    child: NetworkQuotasCard(
                      policy: widget.viewModel.networkPolicy,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              const Text(
                'Redacted Network Audit Log (Recent Decisions)',
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.bold,
                ),
              ),
              const SizedBox(height: 6),
              Expanded(
                child: NetworkAuditLogList(
                  entries: widget.viewModel.networkAuditLog,
                  isLoading: _isLoading,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
