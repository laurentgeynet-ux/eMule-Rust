import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../../app/providers.dart';
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

    return SettingsSectionScaffold(
      title: 'Avancé',
      subtitle: 'Diagnostic, outils de laboratoire et informations daemon',
      icon: Icons.build_circle_outlined,
      children: [
        _AdvancedTile(
          icon: Icons.medical_information_outlined,
          title: 'Diagnostic du daemon',
          subtitle: 'État de santé, connexions et statistiques internes',
          onTap: () => showDialog(
            context: context,
            builder: (_) => const DiagnosticDialog(),
          ),
        ),
        _AdvancedTile(
          icon: Icons.file_download_outlined,
          title: 'Importer un .part.met',
          subtitle: 'Reprendre un téléchargement eMule existant',
          onTap: () => showDialog(
            context: context,
            builder: (_) => const ImportPartMetDialog(),
          ),
        ),
        _AdvancedTile(
          icon: Icons.science_outlined,
          title: 'Guide du mode laboratoire',
          subtitle: 'Scénarios de test loopback sans trafic WAN',
          onTap: () => showDialog(
            context: context,
            builder: (_) => const LabGuideDialog(),
          ),
        ),
        const SizedBox(height: 8),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text(
            'Barre de simulation debug',
            style: TextStyle(fontSize: 13, color: AppColors.textPrimary),
          ),
          subtitle: const Text(
            'Affiche la barre de simulation en bas de l\'écran',
            style: TextStyle(fontSize: 11, color: AppColors.textMuted),
          ),
          value: shell.isDebugBarVisible,
          onChanged: (_) =>
              ref.read(shellControllerProvider).toggleDebugBar(),
        ),
        const SizedBox(height: 16),
        const Divider(color: AppTheme.darkBorder),
        const SizedBox(height: 12),
        if (daemonInfo != null)
          Text(
            'Daemon ${daemonInfo.version} — protocole ${daemonInfo.protocolVersion}',
            style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
          ),
      ],
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
