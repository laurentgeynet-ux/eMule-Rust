import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../../app/providers.dart';
import '../../../../../core/system_actions.dart';
import '../../../downloads/widgets/settings/settings_directory_section.dart';
import '../../../downloads/widgets/settings/settings_inline_message.dart';
import '../../../downloads/widgets/settings/settings_language_section.dart';
import '../../../downloads/widgets/settings/settings_retention_section.dart';
import 'settings_section_scaffold.dart';

/// « Général » : dossier de téléchargement, partage automatique du dossier
/// Incoming, rétention de l'historique et langue.
class SettingsGeneralSection extends ConsumerStatefulWidget {
  const SettingsGeneralSection({super.key});

  @override
  ConsumerState<SettingsGeneralSection> createState() =>
      _SettingsGeneralSectionState();
}

class _SettingsGeneralSectionState
    extends ConsumerState<SettingsGeneralSection> {
  late TextEditingController _dirController;
  late TextEditingController _nickController;
  late TextEditingController _tempDirController;
  bool _isSaving = false;
  String? _inlineError;
  String? _inlineSuccess;

  @override
  void initState() {
    super.initState();
    _dirController =
        TextEditingController(text: ref.read(defaultDownloadDirectoryProvider));
    _nickController = TextEditingController();
    _tempDirController = TextEditingController();
    Future.microtask(() async {
      await ref.read(settingsControllerProvider).fetch();
      if (mounted) {
        final s = ref.read(settingsControllerProvider).settings;
        _nickController.text = s['nickname']?.toString() ?? '';
        _tempDirController.text = s['tempDir']?.toString() ?? '';
      }
    });
  }

  @override
  void dispose() {
    _dirController.dispose();
    _nickController.dispose();
    _tempDirController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(settingsControllerProvider).settings;
    final isCustom = settings['isCustom'] == true ||
        (settings['defaultDownloadDir'] != null &&
            settings['defaultDownloadDir'] != settings['portableDownloadDir']);
    final shareDir = settings['shareDownloadDir'] as bool? ?? true;

    return SettingsSectionScaffold(
      title: 'Général',
      subtitle: 'Identité, dossiers, partage automatique, historique et langue',
      icon: Icons.tune_rounded,
      children: [
        TextField(
          controller: _nickController,
          maxLength: 64,
          style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
          decoration: InputDecoration(
            labelText: 'Pseudo (nickname)',
            helperText:
                'Annoncé aux serveurs eD2K et aux pairs (eMule « nick » ; vide = défaut)',
            helperStyle: const TextStyle(fontSize: 11, color: AppColors.textMuted),
            isDense: true,
            border: const OutlineInputBorder(),
            counterText: '',
            suffixIcon: IconButton(
              icon: const Icon(Icons.save_rounded, size: 18),
              tooltip: 'Appliquer',
              onPressed: _saveNickname,
            ),
          ),
          onSubmitted: (_) => _saveNickname(),
        ),
        const SizedBox(height: 12),
        SettingsDirectorySection(
          dirController: _dirController,
          isCustom: isCustom,
          isSaving: _isSaving,
          onSave: _saveDefaultDir,
          onReset: _resetDefaultDir,
          onOpenFolder: () => systemActions
              .openDirectory(ref.read(defaultDownloadDirectoryProvider)),
        ),
        if (_inlineError != null)
          SettingsInlineMessage(message: _inlineError!, isError: true),
        if (_inlineSuccess != null)
          SettingsInlineMessage(message: _inlineSuccess!, isError: false),
        const SizedBox(height: 12),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text(
            'Partager automatiquement le dossier de téléchargement',
            style: TextStyle(fontSize: 13, color: AppColors.textPrimary),
          ),
          subtitle: const Text(
            'Les fichiers du dossier Incoming sont hashés et servis aux pairs '
            'au démarrage du daemon (comme le dossier Incoming d\'eMule).',
            style: TextStyle(fontSize: 11, color: AppColors.textMuted),
          ),
          value: shareDir,
          onChanged: (v) => ref
              .read(settingsControllerProvider)
              .updateSettings({'shareDownloadDir': v}),
        ),
        const SizedBox(height: 16),
        const Divider(color: AppTheme.darkBorder),
        const SizedBox(height: 12),
        TextField(
          controller: _tempDirController,
          style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
          decoration: InputDecoration(
            labelText: 'Dossier temporaire (.part)',
            helperText:
                'Où sont stockés les fichiers en cours (eMule « Répertoire temporaire » ; vide = dossier du daemon)',
            helperStyle: const TextStyle(fontSize: 11, color: AppColors.textMuted),
            isDense: true,
            border: const OutlineInputBorder(),
            suffixIcon: IconButton(
              icon: const Icon(Icons.save_rounded, size: 18),
              tooltip: 'Appliquer',
              onPressed: () => ref
                  .read(settingsControllerProvider)
                  .updateSettings({'tempDir': _tempDirController.text.trim()}),
            ),
          ),
          onSubmitted: (v) => ref
              .read(settingsControllerProvider)
              .updateSettings({'tempDir': v.trim()}),
        ),
        const SizedBox(height: 12),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: const Text(
            'ICH — vérification intelligente des parts',
            style: TextStyle(fontSize: 12, color: AppColors.textPrimary),
          ),
          subtitle: const Text(
            'Vérifie chaque part complétée par MD4 (eMule « ICH » ; désactiver saute la vérification intermédiaire)',
            style: TextStyle(fontSize: 10, color: AppColors.textMuted),
          ),
          value: settings['ichEnabled'] as bool? ?? true,
          onChanged: (v) => ref
              .read(settingsControllerProvider)
              .updateSettings({'ichEnabled': v}),
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: const Text(
            'Mémoriser les fichiers annulés',
            style: TextStyle(fontSize: 12, color: AppColors.textPrimary),
          ),
          subtitle: const Text(
            'Ajoute une entrée d\'historique à l\'annulation (eMule « Remember cancelled files »)',
            style: TextStyle(fontSize: 10, color: AppColors.textMuted),
          ),
          value: settings['rememberCancelled'] as bool? ?? true,
          onChanged: (v) => ref
              .read(settingsControllerProvider)
              .updateSettings({'rememberCancelled': v}),
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: const Text(
            'Mémoriser les fichiers téléchargés',
            style: TextStyle(fontSize: 12, color: AppColors.textPrimary),
          ),
          subtitle: const Text(
            'Ajoute une entrée d\'historique à la complétion (eMule « Remember downloaded files »)',
            style: TextStyle(fontSize: 10, color: AppColors.textMuted),
          ),
          value: settings['rememberDownloaded'] as bool? ?? true,
          onChanged: (v) => ref
              .read(settingsControllerProvider)
              .updateSettings({'rememberDownloaded': v}),
        ),
        const SizedBox(height: 12),
        SettingsRetentionSection(
          retentionDays:
              ref.watch(historyControllerProvider).retentionDays,
          onChanged:
              ref.read(historyControllerProvider).setRetentionDays,
        ),
        const SizedBox(height: 20),
        const Divider(color: AppTheme.darkBorder),
        const SizedBox(height: 12),
        const SettingsLanguageSection(),
      ],
    );
  }

  Future<void> _saveNickname() async {
    var nick = _nickController.text.trim();
    // eMule resets to the default nick when the field is cleared.
    if (nick.isEmpty) {
      nick = 'Emule-Sion';
      _nickController.text = nick;
    }
    await ref
        .read(settingsControllerProvider)
        .updateSettings({'nickname': nick});
  }

  Future<void> _saveDefaultDir() async {
    setState(() {
      _isSaving = true;
      _inlineError = null;
      _inlineSuccess = null;
    });

    final newDir = _dirController.text.trim();
    if (newDir.isEmpty) {
      setState(() {
        _isSaving = false;
        _inlineError = 'Directory path cannot be empty.';
      });
      return;
    }

    final success = await ref
        .read(settingsControllerProvider)
        .setDefaultDownloadDirectory(newDir);
    if (mounted) {
      setState(() {
        _isSaving = false;
        if (success) {
          _inlineSuccess =
              'Default download directory updated successfully.';
        } else {
          _inlineError = ref.read(shellControllerProvider).errorMessage ??
              'Directory write probe failed.';
        }
      });
    }
  }

  Future<void> _resetDefaultDir() async {
    setState(() {
      _isSaving = true;
      _inlineError = null;
      _inlineSuccess = null;
    });

    final success = await ref
        .read(settingsControllerProvider)
        .resetDefaultDownloadDirectory();
    if (mounted) {
      setState(() {
        _isSaving = false;
        if (success) {
          _dirController.text =
              ref.read(defaultDownloadDirectoryProvider);
          _inlineSuccess = 'Reset to portable default directory.';
        } else {
          _inlineError = ref.read(shellControllerProvider).errorMessage ??
              'Failed to reset download directory.';
        }
      });
    }
  }
}
