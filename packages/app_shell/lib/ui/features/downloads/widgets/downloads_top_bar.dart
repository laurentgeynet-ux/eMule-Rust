import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import '../../../../app/shell_controller.dart';
import '../../../../l10n/l10n.dart';
import 'package:go_router/go_router.dart';

import 'add_link_dialog.dart';
import 'speed_sparkline.dart';
import 'lab_guide_dialog.dart';

/// Navigates to a Settings section — via the router when available,
/// otherwise via the VM-driven section switch (standalone/test usage).
void _goToSettings(BuildContext context, WidgetRef ref, String section) {
  final router = GoRouter.maybeOf(context);
  if (router != null) {
    router.go('/settings?section=$section');
  } else {
    ref.read(shellControllerProvider).setSection(AppNavSection.settings);
  }
}

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
                  if (ref.watch(isLabModeAvailableProvider)) _buildLabButton(context, ref),
                  const SizedBox(width: AppSpacing.sm - 2),
                  _buildSpeedBadge(ref),
                  const SizedBox(width: AppSpacing.sm),
                  _buildSessionTotalsBadge(context, ref),
                  _buildLifetimeTotalsBadge(context, ref),
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

  /// Chip « Loopback Lab Mode » — indicateur de sécurité lab
  /// (loopback-only) ; ouvre le guide de test local.
  Widget _buildLabButton(BuildContext context, WidgetRef ref) {
    return Tooltip(
      message: context.l10n.tooltipLoopbackLabMode,
      preferBelow: true,
      child: InkWell(
        borderRadius: AppRadius.mdAll,
        onTap: () {
          showDialog(
            context: context,
            builder: (ctx) => const LabGuideDialog(),
          );
        },
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
          decoration: BoxDecoration(
            color: AppColors.lab.withValues(alpha: 0.15),
            borderRadius: AppRadius.mdAll,
            border: Border.all(
              color: AppColors.lab.withValues(alpha: 0.3),
              width: 0.5,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(
                Icons.science_outlined,
                size: 14,
                color: AppColors.accentTeal,
              ),
              const SizedBox(width: 5),
              Text(
                context.l10n.loopbackLabMode,
                style: AppTextStyles.toolbarLabel.copyWith(
                  color: AppColors.accentTeal,
                ),
              ),
            ],
          ),
        ),
      ),
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

  /// Session transfer totals (event.statsTick). Replaces the network
  /// profile badge — the profile remains visible in Settings → Connexion.
  /// When the kill switch is engaged the chip keeps its red warning state
  /// and still leads to the policy section.
  Widget _buildSessionTotalsBadge(BuildContext context, WidgetRef ref) {
    final killSwitch = ref.watch(networkPolicyControllerProvider).isKillSwitchActive;
    final stats = ref.watch(statsControllerProvider).latest;
    final accent = killSwitch ? AppColors.tintRed : AppColors.tintCyan;
    final accentText = killSwitch ? AppColors.danger : AppColors.accentCyan;
    return Tooltip(
      message: killSwitch
          ? context.l10n.tooltipPolicyKill
          : context.l10n.tooltipSessionTotals,
      child: InkWell(
        key: const Key('session_totals_badge'),
        borderRadius: AppRadius.mdAll,
        onTap: () {
          if (killSwitch) {
            _goToSettings(context, ref, 'connection');
          } else {
            final router = GoRouter.maybeOf(context);
            if (router != null) {
              router.go('/history');
            } else {
              ref
                  .read(shellControllerProvider)
                  .setSection(AppNavSection.history);
            }
          }
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
              if (killSwitch) ...[
                Icon(Icons.shield_rounded, size: 13, color: accentText),
                const SizedBox(width: 5),
                Text(
                  context.l10n.killSwitchBadgeDisabled,
                  style: AppTextStyles.badgeLabel.copyWith(color: accentText),
                ),
              ] else ...[
                const Icon(
                  Icons.arrow_downward_rounded,
                  size: 13,
                  color: AppTheme.statusDownloading,
                ),
                const SizedBox(width: 3),
                Text(
                  Formatters.formatBytes(stats?.sessionDownloadedBytes ?? 0),
                  style: AppTextStyles.badgeLabel.copyWith(
                    color: AppTheme.statusDownloading,
                  ),
                ),
                const SizedBox(width: 8),
                const Icon(
                  Icons.arrow_upward_rounded,
                  size: 13,
                  color: AppColors.accentTeal,
                ),
                const SizedBox(width: 3),
                Text(
                  Formatters.formatBytes(stats?.sessionUploadedBytes ?? 0),
                  style: AppTextStyles.badgeLabel.copyWith(
                    color: AppColors.accentTeal,
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  /// Cumul « depuis l'installation » (persisté côté daemon, remise à
  /// zéro dans Settings → Avancé). Tap → Historique & Statistiques.
  Widget _buildLifetimeTotalsBadge(BuildContext context, WidgetRef ref) {
    final stats = ref.watch(statsControllerProvider).latest;
    return Tooltip(
      message: context.l10n.tooltipLifetimeTotals,
      child: InkWell(
        key: const Key('lifetime_totals_badge'),
        borderRadius: AppRadius.mdAll,
        onTap: () {
          final router = GoRouter.maybeOf(context);
          if (router != null) {
            router.go('/history');
          } else {
            ref
                .read(shellControllerProvider)
                .setSection(AppNavSection.history);
          }
        },
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
          decoration: AppDecorations.chip(),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(
                Icons.functions_rounded,
                size: 12,
                color: AppColors.textFaint,
              ),
              const SizedBox(width: 4),
              const Icon(
                Icons.arrow_downward_rounded,
                size: 13,
                color: AppColors.textSecondary,
              ),
              const SizedBox(width: 3),
              Text(
                Formatters.formatBytes(stats?.lifetimeDownloadedBytes ?? 0),
                style: AppTextStyles.badgeLabel.copyWith(
                  color: AppColors.textSecondary,
                ),
              ),
              const SizedBox(width: 8),
              const Icon(
                Icons.arrow_upward_rounded,
                size: 13,
                color: AppColors.textSecondary,
              ),
              const SizedBox(width: 3),
              Text(
                Formatters.formatBytes(stats?.lifetimeUploadedBytes ?? 0),
                style: AppTextStyles.badgeLabel.copyWith(
                  color: AppColors.textSecondary,
                ),
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
        _goToSettings(context, ref, 'security');
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
