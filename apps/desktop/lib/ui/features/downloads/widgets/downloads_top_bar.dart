import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/app_controller.dart';
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
class DownloadsTopBar extends StatelessWidget {
  final AppController viewModel;
  final bool isSmall;

  const DownloadsTopBar({
    super.key,
    required this.viewModel,
    this.isSmall = false,
  });

  @override
  Widget build(BuildContext context) {
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
              viewModel.currentFilter.labelOf(context),
              style: AppTextStyles.sectionTitle,
            ),
            const SizedBox(width: AppSpacing.lg),
          ],

          // Search Field
          SizedBox(
            width: isSmall ? 130 : 180,
            height: 36,
            child: TextField(
              onChanged: viewModel.setSearchQuery,
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
                  _buildModeBadge(context),
                  if (viewModel.isLabModeAvailable) _buildLabButton(context),
                  const SizedBox(width: AppSpacing.sm - 2),
                  _buildSpeedBadge(),
                  const SizedBox(width: AppSpacing.sm),
                  _buildNetworkPolicyBadge(context),
                  _buildKillSwitchButton(context),
                  _buildIpFilterButton(context),
                  if (viewModel.isDevToolsAvailable) ...[
                    const SizedBox(width: AppSpacing.sm),
                    _buildDevBadge(context),
                    _buildDebugBarButton(context),
                  ],
                  const SizedBox(width: AppSpacing.xxs),
                  _buildRefreshButton(context),
                  const SizedBox(width: AppSpacing.sm - 2),
                  _buildAddLinkButton(context),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildModeBadge(BuildContext context) {
    final isLab = viewModel.isLabModeAvailable;
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
            builder: (ctx) => DiagnosticDialog(viewModel: viewModel),
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

  Widget _buildLabButton(BuildContext context) {
    return IconButton(
      constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
      padding: EdgeInsets.zero,
      visualDensity: VisualDensity.compact,
      icon: const Icon(Icons.biotech, color: AppColors.accentTeal, size: 18),
      tooltip: context.l10n.tooltipLabGuide,
      onPressed: () {
        showDialog(
          context: context,
          builder: (ctx) => LabGuideDialog(viewModel: viewModel),
        );
      },
    );
  }

  Widget _buildSpeedBadge() {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      decoration: AppDecorations.chip(),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.arrow_downward_rounded, size: 13, color: AppTheme.statusDownloading),
          const SizedBox(width: 3),
          Text(
            Formatters.formatSpeed(viewModel.totalDownloadSpeed),
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
            Formatters.formatSpeed(viewModel.totalUploadSpeed),
            style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.bold,
              color: AppColors.textFaint,
            ),
          ),
          const SizedBox(width: 8),
          SpeedSparkline(tracker: viewModel.speedTracker),
        ],
      ),
    );
  }

  Widget _buildNetworkPolicyBadge(BuildContext context) {
    final killSwitch = viewModel.isNetworkKillSwitchActive;
    final accent = killSwitch ? AppColors.tintRed : AppColors.tintCyan;
    final accentText = killSwitch ? AppColors.danger : AppColors.accentCyan;
    return Tooltip(
      message: killSwitch
          ? context.l10n.tooltipPolicyKill
          : context.l10n.tooltipPolicyInfo(
              viewModel.currentNetworkProfile.label,
              viewModel.currentNetworkState.label,
            ),
      child: InkWell(
        key: const Key('network_policy_badge'),
        borderRadius: AppRadius.mdAll,
        onTap: () {
          showDialog(
            context: context,
            builder: (ctx) => NetworkPolicyDialog(viewModel: viewModel),
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
                    : viewModel.currentNetworkProfile.toKebabCase(),
                style: AppTextStyles.badgeLabel.copyWith(color: accentText),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildKillSwitchButton(BuildContext context) {
    final killSwitch = viewModel.isNetworkKillSwitchActive;
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
          viewModel.enableNetwork();
        } else {
          viewModel.disconnectAllNetwork();
        }
      },
    );
  }

  Widget _buildIpFilterButton(BuildContext context) {
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
          builder: (ctx) => IpFilterDialog(viewModel: viewModel),
        );
      },
    );
  }

  Widget _buildDevBadge(BuildContext context) {
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

  Widget _buildDebugBarButton(BuildContext context) {
    return IconButton(
      constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
      padding: EdgeInsets.zero,
      visualDensity: VisualDensity.compact,
      icon: Icon(
        Icons.bug_report_rounded,
        color: viewModel.isDebugBarVisible
            ? AppColors.accentAmber
            : AppColors.textMuted,
        size: 18,
      ),
      tooltip: context.l10n.tooltipDevTools,
      onPressed: viewModel.toggleDebugBar,
    );
  }

  Widget _buildRefreshButton(BuildContext context) {
    return IconButton(
      constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
      padding: EdgeInsets.zero,
      visualDensity: VisualDensity.compact,
      icon: const Icon(Icons.refresh_rounded, color: AppColors.textMuted, size: 18),
      tooltip: context.l10n.tooltipRefresh,
      onPressed: viewModel.refresh,
    );
  }

  Widget _buildAddLinkButton(BuildContext context) {
    void showAddDialog() {
      showDialog(
        context: context,
        builder: (ctx) => AddLinkDialog(viewModel: viewModel),
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
