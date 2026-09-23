// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../../app/providers.dart';
import '../../../../../core/folder_picker.dart';
import '../../../../../core/system_actions.dart';
import '../../../../../l10n/l10n.dart';
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

  /// Champs en cours d'édition : jamais écrasés par une resynchronisation
  /// externe (`settings.changed`, autre client, MCP).
  final Set<TextEditingController> _dirty = {};

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
      if (mounted) _syncFromSettings();
    });
  }

  @override
  void dispose() {
    _dirController.dispose();
    _nickController.dispose();
    _tempDirController.dispose();
    super.dispose();
  }

  void _syncText(TextEditingController ctrl, Object? value) {
    final v = value?.toString() ?? '';
    if (!_dirty.contains(ctrl) && ctrl.text != v) ctrl.text = v;
  }

  /// Réécrit les champs depuis la map de settings (fetch initial ou
  /// événement `settings.changed`), sauf ceux en cours d'édition.
  void _syncFromSettings() {
    final s = ref.read(settingsControllerProvider).settings;
    _syncText(_nickController, s['nickname']);
    _syncText(_tempDirController, s['tempDir']);
    _syncText(
      _dirController,
      s['defaultDownloadDir'] ?? ref.read(defaultDownloadDirectoryProvider),
    );
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(settingsControllerProvider, (_, __) => _syncFromSettings());
    final settings = ref.watch(settingsControllerProvider).settings;
    final l10n = context.l10n;
    final isCustom = settings['isCustom'] == true ||
        (settings['defaultDownloadDir'] != null &&
            settings['defaultDownloadDir'] != settings['portableDownloadDir']);
    final shareDir = settings['shareDownloadDir'] as bool? ?? true;

    return SettingsSectionScaffold(
      title: l10n.settingsSectionGeneral,
      subtitle: l10n.settingsGeneralSubtitle,
      icon: Icons.tune_rounded,
      children: [
        TextField(
          controller: _nickController,
          maxLength: 64,
          style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
          decoration: InputDecoration(
            labelText: l10n.settingsNicknameLabel,
            helperText: l10n.settingsNicknameHelper,
            helperStyle: const TextStyle(fontSize: 11, color: AppColors.textMuted),
            isDense: true,
            border: const OutlineInputBorder(),
            counterText: '',
            suffixIcon: IconButton(
              icon: const Icon(Icons.save_rounded, size: 18),
              tooltip: l10n.settingsApply,
              onPressed: _saveNickname,
            ),
          ),
          onChanged: (_) => _dirty.add(_nickController),
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
          onBrowse: () => _browseDir(
            _dirController,
            l10n.globalDefaultDownloadDirectory,
          ),
          onChanged: (_) => _dirty.add(_dirController),
        ),
        if (_inlineError != null)
          SettingsInlineMessage(message: _inlineError!, isError: true),
        if (_inlineSuccess != null)
          SettingsInlineMessage(message: _inlineSuccess!, isError: false),
        const SizedBox(height: 12),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(
            l10n.settingsShareDownloadDirTitle,
            style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
          ),
          subtitle: Text(
            l10n.settingsShareDownloadDirSub,
            style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
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
            labelText: l10n.settingsTempDirLabel,
            helperText: l10n.settingsTempDirHelper,
            helperStyle: const TextStyle(fontSize: 11, color: AppColors.textMuted),
            isDense: true,
            border: const OutlineInputBorder(),
            suffixIcon: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                IconButton(
                  icon: const Icon(Icons.folder_open_rounded, size: 18),
                  tooltip: l10n.browse,
                  onPressed: () => _browseDir(
                    _tempDirController,
                    l10n.settingsTempDirLabel,
                  ),
                ),
                IconButton(
                  icon: const Icon(Icons.save_rounded, size: 18),
                  tooltip: l10n.settingsApply,
                  onPressed: _saveTempDir,
                ),
              ],
            ),
          ),
          onChanged: (_) => _dirty.add(_tempDirController),
          onSubmitted: (_) => _saveTempDir(),
        ),
        const SizedBox(height: 12),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: Text(
            l10n.settingsIchTitle,
            style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
          ),
          subtitle: Text(
            l10n.settingsIchSub,
            style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
          ),
          value: settings['ichEnabled'] as bool? ?? true,
          onChanged: (v) => ref
              .read(settingsControllerProvider)
              .updateSettings({'ichEnabled': v}),
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: Text(
            l10n.settingsRememberCancelledTitle,
            style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
          ),
          subtitle: Text(
            l10n.settingsRememberCancelledSub,
            style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
          ),
          value: settings['rememberCancelled'] as bool? ?? true,
          onChanged: (v) => ref
              .read(settingsControllerProvider)
              .updateSettings({'rememberCancelled': v}),
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: Text(
            l10n.settingsRememberDownloadedTitle,
            style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
          ),
          subtitle: Text(
            l10n.settingsRememberDownloadedSub,
            style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
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

  /// Ouvre l'explorateur de dossiers natif et remplit [ctrl] avec le chemin
  /// choisi — marqué dirty pour ne pas être écrasé par une resync externe.
  Future<void> _browseDir(
    TextEditingController ctrl,
    String description,
  ) async {
    final picked = await pickFolder(description: description);
    if (picked != null && mounted) {
      ctrl.text = picked;
      _dirty.add(ctrl);
    }
  }

  Future<void> _saveNickname() async {
    var nick = _nickController.text.trim();
    // eMule resets to the default nick when the field is cleared.
    if (nick.isEmpty) {
      nick = 'eMule-Rust';
      _nickController.text = nick;
    }
    final ok = await ref
        .read(settingsControllerProvider)
        .updateSettings({'nickname': nick});
    if (ok && mounted) {
      _dirty.remove(_nickController);
      _syncFromSettings();
    }
  }

  Future<void> _saveTempDir() async {
    final ok = await ref.read(settingsControllerProvider).updateSettings(
      {'tempDir': _tempDirController.text.trim()},
    );
    if (ok && mounted) {
      _dirty.remove(_tempDirController);
      _syncFromSettings();
    }
  }

  Future<void> _saveDefaultDir() async {
    final l10n = context.l10n;
    setState(() {
      _isSaving = true;
      _inlineError = null;
      _inlineSuccess = null;
    });

    final newDir = _dirController.text.trim();
    if (newDir.isEmpty) {
      setState(() {
        _isSaving = false;
        _inlineError = l10n.settingsDirEmptyError;
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
          _dirty.remove(_dirController);
          _inlineSuccess = l10n.settingsDirSaved;
        } else {
          _inlineError = ref.read(shellControllerProvider).errorMessage ??
              l10n.settingsDirProbeFailed;
        }
      });
      if (success) _syncFromSettings();
    }
  }

  Future<void> _resetDefaultDir() async {
    final l10n = context.l10n;
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
          _dirty.remove(_dirController);
          _dirController.text =
              ref.read(defaultDownloadDirectoryProvider);
          _inlineSuccess = l10n.settingsDirResetDone;
        } else {
          _inlineError = ref.read(shellControllerProvider).errorMessage ??
              l10n.settingsDirResetFailed;
        }
      });
    }
  }
}
