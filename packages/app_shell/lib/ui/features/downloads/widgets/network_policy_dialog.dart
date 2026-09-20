import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import 'network/network_audit_log_list.dart';
import 'network/network_profile_selector.dart';
import 'network/network_quotas_card.dart';
import 'network/network_state_banner.dart';
import '../../../../l10n/l10n.dart';

/// Network policy & defense guard: kill switch, connectivity profile,
/// socket budgets and the redacted audit log.
class NetworkPolicyDialog extends ConsumerStatefulWidget {
    const NetworkPolicyDialog({super.key, });

  @override
  ConsumerState<NetworkPolicyDialog> createState() => _NetworkPolicyDialogState();
}

class _NetworkPolicyDialogState extends ConsumerState<NetworkPolicyDialog> {
  bool _isLoading = false;

  @override
  void initState() {
    super.initState();
    Future.microtask(_loadData);
  }

  Future<void> _loadData() async {
    setState(() => _isLoading = true);
    await ref.read(networkPolicyControllerProvider).fetchPolicy();
    await ref.read(networkPolicyControllerProvider).fetchAuditLog(limit: 20);
    if (mounted) {
      setState(() => _isLoading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final isKillSwitchActive =
        ref.watch(networkPolicyControllerProvider).isKillSwitchActive;
    final profile = ref.watch(networkPolicyControllerProvider).currentProfile;

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
                        Text(context.l10n.networkPolicyDefenseGuard,
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text(context.l10n.defensiveSocketAuthorizationAntiSsrf,
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
                state: ref.watch(networkPolicyControllerProvider).currentState,
                isKillSwitchActive: isKillSwitchActive,
                onDisconnectAll: () async {
                  await ref.read(networkPolicyControllerProvider).disconnectAll();
                  await _loadData();
                },
                onReenable: () async {
                  await ref.read(networkPolicyControllerProvider).enableNetwork();
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
                        await ref.read(networkPolicyControllerProvider).setProfile(p);
                        await _loadData();
                      },
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    flex: 2,
                    child: NetworkQuotasCard(
                      policy: ref.watch(networkPolicyControllerProvider).policy,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              Text(context.l10n.redactedNetworkAuditLogRecent,
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.bold,
                ),
              ),
              const SizedBox(height: 6),
              Expanded(
                child: NetworkAuditLogList(
                  entries: ref.watch(networkPolicyControllerProvider).auditLog,
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
