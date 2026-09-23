// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../../app/providers.dart';
import '../../../../../l10n/l10n.dart';
import '../../../downloads/widgets/diagnostic_dialog.dart';
import '../../../downloads/widgets/import_part_met_dialog.dart';
import '../../../downloads/widgets/lab_guide_dialog.dart';
import 'settings_section_scaffold.dart';

/// « Avancé » : diagnostic daemon, outils de lab, barre de debug et
/// informations de version.
class SettingsAdvancedSection extends ConsumerWidget {
  const SettingsAdvancedSection({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final shell = ref.watch(shellControllerProvider);
    final daemonInfo = ref.watch(daemonInfoProvider);
    final l10n = context.l10n;

    return SettingsSectionScaffold(
      title: l10n.settingsSectionAdvanced,
      subtitle: l10n.settingsAdvancedSubtitle,
      icon: Icons.build_circle_outlined,
      children: [
        _AdvancedTile(
          icon: Icons.medical_information_outlined,
          title: l10n.settingsDiagnosticTile,
          subtitle: l10n.settingsDiagnosticSub,
          onTap: () => showDialog(
            context: context,
            builder: (_) => const DiagnosticDialog(),
          ),
        ),
        _AdvancedTile(
          icon: Icons.file_download_outlined,
          title: l10n.settingsImportPartTile,
          subtitle: l10n.settingsImportPartSub,
          onTap: () => showDialog(
            context: context,
            builder: (_) => const ImportPartMetDialog(),
          ),
        ),
        // Guide lab : uniquement quand les outils lab sont réellement
        // disponibles (daemon portable avec capacité `lab-tools`, ou dev
        // tools) — masqué en release standard.
        if (ref.watch(isLabModeAvailableProvider))
          _AdvancedTile(
            icon: Icons.science_outlined,
            title: l10n.settingsLabGuideTile,
            subtitle: l10n.settingsLabGuideSub,
            onTap: () => showDialog(
              context: context,
              builder: (_) => const LabGuideDialog(),
            ),
          ),
        _AdvancedTile(
          icon: Icons.restart_alt_rounded,
          title: l10n.settingsLifetimeResetTile,
          subtitle: l10n.settingsLifetimeResetSub,
          onTap: () => _confirmLifetimeReset(context, ref),
        ),
        const SizedBox(height: 8),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(
            l10n.settingsDebugBarTitle,
            style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
          ),
          subtitle: Text(
            shell.isDevToolsAvailable
                ? l10n.settingsDebugBarOn
                : l10n.settingsDebugBarOff,
            style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
          ),
          value: shell.isDebugBarVisible,
          onChanged: shell.isDevToolsAvailable
              ? (_) => ref.read(shellControllerProvider).toggleDebugBar()
              : null,
        ),
        const SizedBox(height: 16),
        const Divider(color: AppTheme.darkBorder),
        const SizedBox(height: 12),
        if (daemonInfo != null)
          Text(
            l10n.settingsDaemonVersion(
              daemonInfo.version,
              '${daemonInfo.protocolVersion}',
            ),
            style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
          ),
        const SizedBox(height: 4),
        const Text(
          '© 2026 Laurent Geynet "Loulach" — GPL-2.0-or-later',
          style: TextStyle(fontSize: 11, color: AppColors.textMuted),
        ),
      ],
    );
  }

  /// Confirmation avant remise à zéro des compteurs « depuis
  /// l'installation » (`stats.resetLifetimeTotals`) — les totaux de
  /// session sont conservés.
  void _confirmLifetimeReset(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(l10n.lifetimeResetConfirmTitle),
        content: Text(
          l10n.lifetimeResetConfirmBody,
          style: const TextStyle(fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: AppTheme.statusFailed,
            ),
            onPressed: () async {
              Navigator.of(ctx).pop();
              try {
                await ref
                    .read(muleDaemonClientProvider)
                    .resetLifetimeTotals();
              } catch (_) {}
            },
            child: Text(l10n.lifetimeResetConfirmButton),
          ),
        ],
      ),
    );
  }
}

class _AdvancedTile extends StatelessWidget {
  final IconData icon;
  final String title;
  final String subtitle;
  final VoidCallback onTap;

  const _AdvancedTile({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: Icon(icon, size: 20, color: AppColors.accent),
      title: Text(
        title,
        style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
      ),
      subtitle: Text(
        subtitle,
        style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
      ),
      trailing: const Icon(Icons.chevron_right_rounded,
          size: 18, color: AppColors.textFaint),
      onTap: onTap,
    );
  }
}
