import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import '../../../../l10n/l10n.dart';
import 'add_link_dialog.dart';
import 'diagnostic_dialog.dart';
import 'speed_sparkline.dart';
import 'ip_filter_dialog.dart';
import 'lab_guide_dialog.dart';
import 'network_policy_dialog.dart';

/// Top action bar of the downloads workspace: menu button (compact layouts),
/// filter title, search field, and the action/badges cluster
/// (diagnostics, lab tools, speeds, network policy, dev tools, refresh,
/// add link).
class DownloadsTopBar extends ConsumerWidget {
    final bool isSmall;

  const DownloadsTopBar({
    super.key,
    this.isSmall = false,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      decoration: AppDecorations.headerBar,
      child: Row(
        children: [
          // Drawer opener button if on small screen
          if (isSmall) ...[
            Builder(
              builder: (ctx) => IconButton(
                icon: const Icon(Icons.menu_rounded, color: AppColors.textSecondary),
                tooltip: context.l10n.tooltipOpenDrawer,
                onPressed: () => Scaffold.of(ctx).openDrawer(),
              ),
            ),
            const SizedBox(width: AppSpacing.sm),
          ],
          // Filter title
          if (!isSmall) ...[
            Text(
              ref.watch(shellControllerProvider).filter.labelOf(context),
              style: AppTextStyles.sectionTitle,
            ),
            const SizedBox(width: AppSpacing.lg),
          ],

          // Search Field
          SizedBox(
            width: isSmall ? 130 : 180,
            height: 36,
            child: TextField(
              onChanged: ref.read(shellControllerProvider).setSearchQuery,
              style: const TextStyle(fontSize: 13),
              decoration: InputDecoration(
                hintText: context.l10n.searchPlaceholder,
                hintStyle: const TextStyle(fontSize: 12, color: AppColors.textFaint),
                isDense: true,
                prefixIcon: const Icon(Icons.search, size: 16, color: AppColors.textFaint),
                prefixIconConstraints: const BoxConstraints(minWidth: 30, minHeight: 30),
                contentPadding: const EdgeInsets.symmetric(vertical: 8, horizontal: 8),
                filled: true,
                fillColor: AppColors.card,
                border: OutlineInputBorder(
                  borderRadius: AppRadius.mdAll,
                  borderSide: BorderSide.none,
                ),
              ),
            ),
          ),

          Expanded(
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              reverse: true,
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  _buildModeBadge(context, ref),
                  if (ref.watch(isLabModeAvailableProvider)) _buildLabButton(context, ref),
                  const SizedBox(width: AppSpacing.sm - 2),
                  _buildSpeedBadge(ref),
                  const SizedBox(width: AppSpacing.sm),
                  _buildNetworkPolicyBadge(context, ref),
                  _buildKillSwitchButton(context, ref),
                  _buildIpFilterButton(context, ref),
                  if (ref.watch(devToolsEnabledProvider)) ...[
                    const SizedBox(width: AppSpacing.sm),
                    _buildDevBadge(context, ref),
                    _buildDebugBarButton(context, ref),
                  ],
                  const SizedBox(width: AppSpacing.xxs),
                  _buildRefreshButton(context, ref),
                  const SizedBox(width: AppSpacing.sm - 2),
                  _buildAddLinkButton(context, ref),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildModeBadge(BuildContext context, WidgetRef ref) {
    final isLab = ref.watch(isLabModeAvailableProvider);
    final accent = isLab ? AppColors.lab : AppColors.labDim;
    return Tooltip(
      message: isLab
          ? '${context.l10n.tooltipLoopbackLabMode}\n\n${context.l10n.tooltipDiagnostics}'
          : '${context.l10n.tooltipSystemDiagnostics}\n\n${context.l10n.tooltipDiagnostics}',
      preferBelow: true,
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: const Color(0xF01E2A3A),
        borderRadius: AppRadius.mdAll,
        border: Border.all(color: accent.withValues(alpha: 0.4), width: 0.5),
      ),
      textStyle: const TextStyle(
        fontSize: 12,
        color: AppColors.textSecondary,
        height: 1.4,
      ),
      child: InkWell(
        borderRadius: AppRadius.mdAll,
        onTap: () {
          showDialog(
            context: context,
            builder: (ctx) => const DiagnosticDialog(),
          );
        },
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
          decoration: BoxDecoration(
            color: accent.withValues(alpha: 0.15),
            borderRadius: AppRadius.mdAll,
            border: Border.all(color: accent.withValues(alpha: 0.3), width: 0.5),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                isLab ? Icons.science_outlined : Icons.analytics_outlined,
                size: 14,
                color: isLab ? AppColors.accentTeal : AppColors.textSecondary,
              ),
              const SizedBox(width: 5),
              Text(
                isLab
                    ? context.l10n.loopbackLabMode
                    : context.l10n.diagnosticsBadge,
                style: AppTextStyles.toolbarLabel.copyWith(
                  color: isLab ? AppColors.accentTeal : AppColors.textSecondary,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildLabButton(BuildContext context, WidgetRef ref) {
    return IconButton(
      constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
      padding: EdgeInsets.zero,
      visualDensity: VisualDensity.compact,
      icon: const Icon(Icons.biotech, color: AppColors.accentTeal, size: 18),
      tooltip: context.l10n.tooltipLabGuide,
      onPressed: () {
        showDialog(
          context: context,
          builder: (ctx) => const LabGuideDialog(),
        );
      },
    );
  }

  Widget _buildSpeedBadge(WidgetRef ref) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      decoration: AppDecorations.chip(),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.arrow_downward_rounded, size: 13, color: AppTheme.statusDownloading),
          const SizedBox(width: 3),
          Text(
            Formatters.formatSpeed(ref.watch(downloadsStoreControllerProvider).totalDownloadSpeed),
            style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.bold,
              color: AppTheme.statusDownloading,
            ),
          ),
          const SizedBox(width: 8),
          const Icon(Icons.arrow_upward_rounded, size: 13, color: AppColors.textFaint),
          const SizedBox(width: 3),
          Text(
            Formatters.formatSpeed(ref.watch(uploadsControllerProvider).totalUploadRateBps),
            style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.bold,
              color: AppColors.textFaint,
            ),
          ),
          const SizedBox(width: 8),
          SpeedSparkline(tracker: ref.read(speedTrackerProvider)),
        ],
      ),
    );
  }

  Widget _buildNetworkPolicyBadge(BuildContext context, WidgetRef ref) {
    final killSwitch = ref.watch(networkPolicyControllerProvider).isKillSwitchActive;
    final accent = killSwitch ? AppColors.tintRed : AppColors.tintCyan;
    final accentText = killSwitch ? AppColors.danger : AppColors.accentCyan;
    return Tooltip(
      message: killSwitch
          ? context.l10n.tooltipPolicyKill
          : context.l10n.tooltipPolicyInfo(
              ref.watch(networkPolicyControllerProvider).currentProfile.label,
              ref.watch(networkPolicyControllerProvider).currentState.label,
            ),
      child: InkWell(
        key: const Key('network_policy_badge'),
        borderRadius: AppRadius.mdAll,
        onTap: () {
          showDialog(
            context: context,
            builder: (ctx) => const NetworkPolicyDialog(),
          );
        },
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
          decoration: BoxDecoration(
            color: accent.withValues(alpha: killSwitch ? 0.2 : 0.12),
            borderRadius: AppRadius.mdAll,
            border: Border.all(
              color: accent.withValues(alpha: killSwitch ? 0.5 : 0.3),
              width: 0.5,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                killSwitch ? Icons.shield_rounded : Icons.security_rounded,
                size: 13,
                color: accentText,
              ),
              const SizedBox(width: 5),
              Text(
                killSwitch
                    ? context.l10n.killSwitchBadgeDisabled
                    : ref.watch(networkPolicyControllerProvider).currentProfile.toKebabCase(),
                style: AppTextStyles.badgeLabel.copyWith(color: accentText),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildKillSwitchButton(BuildContext context, WidgetRef ref) {
    final killSwitch = ref.watch(networkPolicyControllerProvider).isKillSwitchActive;
    return IconButton(
      key: const Key('network_kill_switch_button'),
      constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
      padding: EdgeInsets.zero,
      visualDensity: VisualDensity.compact,
      icon: Icon(
        killSwitch
            ? Icons.play_circle_outline_rounded
            : Icons.power_settings_new_rounded,
        color: killSwitch
            ? AppColors.positive
            : AppColors.danger.withValues(alpha: 0.85),
        size: 18,
      ),
      tooltip: killSwitch
          ? context.l10n.tooltipKillSwitchEnable
          : context.l10n.tooltipKillSwitchDisable,
      onPressed: () {
        if (killSwitch) {
          ref.read(networkPolicyControllerProvider).enableNetwork();
        } else {
          ref.read(networkPolicyControllerProvider).disconnectAll();
        }
      },
    );
  }

  Widget _buildIpFilterButton(BuildContext context, WidgetRef ref) {
    return IconButton(
      key: const Key('ip_filter_button'),
      constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
      padding: EdgeInsets.zero,
      visualDensity: VisualDensity.compact,
      icon: const Icon(
        Icons.filter_alt_outlined,
        color: AppColors.warning,
        size: 18,
      ),
      tooltip: context.l10n.tooltipIpFilter,
      onPressed: () {
        showDialog(
          context: context,
          builder: (ctx) => const IpFilterDialog(),
        );
      },
    );
  }

  Widget _buildDevBadge(BuildContext context, WidgetRef ref) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
      decoration: BoxDecoration(
        color: AppColors.warning.withValues(alpha: 0.2),
        borderRadius: AppRadius.smAll,
        border: Border.all(
          color: AppColors.warning.withValues(alpha: 0.4),
          width: 0.5,
        ),
      ),
      child: Text(
        context.l10n.devModeBadge,
        style: const TextStyle(
          fontSize: 10,
          fontWeight: FontWeight.bold,
          color: AppColors.accentAmber,
        ),
      ),
    );
  }

  Widget _buildDebugBarButton(BuildContext context, WidgetRef ref) {
    return IconButton(
      constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
      padding: EdgeInsets.zero,
      visualDensity: VisualDensity.compact,
      icon: Icon(
        Icons.bug_report_rounded,
        color: ref.watch(shellControllerProvider).isDebugBarVisible
            ? AppColors.accentAmber
            : AppColors.textMuted,
        size: 18,
      ),
      tooltip: context.l10n.tooltipDevTools,
      onPressed: ref.read(shellControllerProvider).toggleDebugBar,
    );
  }

  Widget _buildRefreshButton(BuildContext context, WidgetRef ref) {
    return IconButton(
      constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
      padding: EdgeInsets.zero,
      visualDensity: VisualDensity.compact,
      icon: const Icon(Icons.refresh_rounded, color: AppColors.textMuted, size: 18),
      tooltip: context.l10n.tooltipRefresh,
      onPressed: ref.read(appControllerProvider).refresh,
    );
  }

  Widget _buildAddLinkButton(BuildContext context, WidgetRef ref) {
    void showAddDialog() {
      showDialog(
        context: context,
        builder: (ctx) => const AddLinkDialog(),
      );
    }

    if (isSmall) {
      return IconButton.filled(
        icon: const Icon(Icons.add_rounded, size: 18),
        tooltip: context.l10n.btnAddLink,
        style: IconButton.styleFrom(backgroundColor: AppColors.accent),
        onPressed: showAddDialog,
      );
    }
    return FilledButton.icon(
      icon: const Icon(Icons.add_rounded, size: 16),
      label: Text(context.l10n.btnAddLink),
      style: FilledButton.styleFrom(
        backgroundColor: AppColors.accent,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      ),
      onPressed: showAddDialog,
    );
  }
}
