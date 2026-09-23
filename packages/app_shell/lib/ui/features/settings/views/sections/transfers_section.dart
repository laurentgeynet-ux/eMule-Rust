// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../../app/providers.dart';
import '../../../../../l10n/l10n.dart';
import 'settings_section_scaffold.dart';

/// « Transferts » : limites de sources du swarm (persistées via
/// `settings.set` — équivalent des options eD2K d'eMule).
class SettingsTransfersSection extends ConsumerStatefulWidget {
  const SettingsTransfersSection({super.key});

  @override
  ConsumerState<SettingsTransfersSection> createState() =>
      _SettingsTransfersSectionState();
}

class _SettingsTransfersSectionState
    extends ConsumerState<SettingsTransfersSection> {
  final _maxGlobalCtrl = TextEditingController();
  final _maxPerFileCtrl = TextEditingController();
  final _minPerFileCtrl = TextEditingController();
  final _maxQueueCtrl = TextEditingController();
  final _minUploadCtrl = TextEditingController();
  final _dailyQuotaCtrl = TextEditingController();

  /// Champs en cours d'édition : jamais écrasés par une resynchronisation
  /// externe (`settings.changed`, autre client, MCP).
  final Set<TextEditingController> _dirty = {};

  bool _isSaving = false;
  bool _loaded = false;

  @override
  void initState() {
    super.initState();
    Future.microtask(() async {
      await ref.read(settingsControllerProvider).fetch();
      _syncFromSettings();
    });
  }

  @override
  void dispose() {
    _maxGlobalCtrl.dispose();
    _maxPerFileCtrl.dispose();
    _minPerFileCtrl.dispose();
    _maxQueueCtrl.dispose();
    _minUploadCtrl.dispose();
    _dailyQuotaCtrl.dispose();
    super.dispose();
  }

  void _syncText(TextEditingController ctrl, Object? value) {
    final v = value?.toString() ?? '';
    if (!_dirty.contains(ctrl) && ctrl.text != v) ctrl.text = v;
  }

  void _syncFromSettings() {
    final s = ref.read(settingsControllerProvider).settings;
    _syncText(_maxGlobalCtrl, s['maxGlobalDownloadPeers']);
    _syncText(_maxPerFileCtrl, s['maxPeersPerDownload']);
    _syncText(_minPerFileCtrl, s['minPeersPerDownload']);
    _syncText(_maxQueueCtrl, s['maxUploadQueue']);
    _syncText(_minUploadCtrl, s['minUploadKbs'] ?? 1);
    _syncText(_dailyQuotaCtrl, s['dailyUploadQuotaGb'] ?? 0);
    if (mounted && !_loaded) setState(() => _loaded = true);
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(settingsControllerProvider, (_, __) => _syncFromSettings());
    final l10n = context.l10n;

    if (!_loaded) {
      return const Center(child: CircularProgressIndicator());
    }

    return SettingsSectionScaffold(
      title: l10n.settingsSectionTransfers,
      subtitle: l10n.settingsTransfersSubtitle,
      icon: Icons.swap_vert_rounded,
      children: [
        _NumberField(
          controller: _maxGlobalCtrl,
          label: l10n.settingsMaxGlobalLabel,
          hint: l10n.settingsMaxGlobalHint,
          onDirty: () => _dirty.add(_maxGlobalCtrl),
          onSubmit: _save,
        ),
        const SizedBox(height: 12),
        _NumberField(
          controller: _maxPerFileCtrl,
          label: l10n.settingsMaxPerFileLabel,
          hint: l10n.settingsMaxPerFileHint,
          onDirty: () => _dirty.add(_maxPerFileCtrl),
          onSubmit: _save,
        ),
        const SizedBox(height: 12),
        _NumberField(
          controller: _minPerFileCtrl,
          label: l10n.settingsMinPerFileLabel,
          hint: l10n.settingsMinPerFileHint,
          onDirty: () => _dirty.add(_minPerFileCtrl),
          onSubmit: _save,
        ),
        const SizedBox(height: 12),
        _NumberField(
          controller: _maxQueueCtrl,
          label: l10n.settingsMaxQueueLabel,
          hint: l10n.settingsMaxQueueHint,
          onDirty: () => _dirty.add(_maxQueueCtrl),
          onSubmit: _save,
        ),
        const SizedBox(height: 12),
        _NumberField(
          controller: _minUploadCtrl,
          label: l10n.settingsMinUploadLabel,
          hint: l10n.settingsMinUploadHint,
          onDirty: () => _dirty.add(_minUploadCtrl),
          onSubmit: _save,
        ),
        const SizedBox(height: 12),
        _NumberField(
          controller: _dailyQuotaCtrl,
          label: l10n.settingsDailyQuotaLabel,
          hint: l10n.settingsDailyQuotaHint,
          onDirty: () => _dirty.add(_dailyQuotaCtrl),
          onSubmit: _save,
        ),
        const SizedBox(height: 8),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: Text(
            l10n.settingsAutoPriorityTitle,
            style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
          ),
          subtitle: Text(
            l10n.settingsAutoPrioritySub,
            style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
          ),
          value:
              ref.watch(settingsControllerProvider).settings['autoPriorityNewDownloads']
                  as bool? ??
              false,
          onChanged: (v) => ref
              .read(settingsControllerProvider)
              .updateSettings({'autoPriorityNewDownloads': v}),
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: Text(
            l10n.settingsSwarmHealingTitle,
            style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
          ),
          subtitle: Text(
            l10n.settingsSwarmHealingSub,
            style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
          ),
          value:
              ref.watch(settingsControllerProvider).settings['swarmHealing']
                  as bool? ??
              false,
          onChanged: (v) => ref
              .read(settingsControllerProvider)
              .updateSettings({'swarmHealing': v}),
        ),
        const SizedBox(height: 16),
        FilledButton.icon(
          icon: _isSaving
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.save_rounded, size: 16),
          label: Text(_isSaving ? l10n.settingsSaving : l10n.settingsApply),
          onPressed: _isSaving ? null : _save,
        ),
      ],
    );
  }

  Future<void> _save() async {
    final l10n = context.l10n;
    final patch = <String, dynamic>{};
    final applied = <TextEditingController>[];
    var invalid = false;
    void add(
      TextEditingController ctrl,
      String label,
      int min,
      String key, [
      int? max,
    ]) {
      final raw = ctrl.text.trim();
      if (raw.isEmpty) return; // champ vide = valeur inchangée
      final n = int.tryParse(raw);
      if (n == null || n < min || (max != null && n > max)) {
        invalid = true;
        ref
            .read(shellControllerProvider)
            .setError(
              max != null
                  ? l10n.settingsInvalidNumRange(label, raw, min, max)
                  : l10n.settingsInvalidNumMin(label, raw, min),
            );
      } else {
        patch[key] = n;
        applied.add(ctrl);
      }
    }

    add(_maxGlobalCtrl, l10n.settingsMaxGlobalLabel, 1, 'maxGlobalDownloadPeers');
    add(_maxPerFileCtrl, l10n.settingsMaxPerFileLabel, 1, 'maxPeersPerDownload');
    add(_minPerFileCtrl, l10n.settingsMinPerFileLabel, 0, 'minPeersPerDownload');
    add(_maxQueueCtrl, l10n.settingsMaxQueueLabel, 10, 'maxUploadQueue', 5000);
    add(_minUploadCtrl, l10n.settingsMinUploadLabel, 0, 'minUploadKbs');
    add(
      _dailyQuotaCtrl,
      l10n.settingsDailyQuotaLabel,
      0,
      'dailyUploadQuotaGb',
      1048576,
    );

    if (invalid || patch.isEmpty) return;

    setState(() => _isSaving = true);
    final ok =
        await ref.read(settingsControllerProvider).updateSettings(patch);
    if (mounted) {
      setState(() => _isSaving = false);
      if (ok) {
        for (final c in applied) {
          _dirty.remove(c);
        }
        _syncFromSettings();
      }
    }
  }
}

class _NumberField extends StatelessWidget {
  final TextEditingController controller;
  final String label;
  final String hint;
  final VoidCallback onDirty;
  final VoidCallback onSubmit;

  const _NumberField({
    required this.controller,
    required this.label,
    required this.hint,
    required this.onDirty,
    required this.onSubmit,
  });

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: controller,
      keyboardType: TextInputType.number,
      style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
      decoration: InputDecoration(
        labelText: label,
        helperText: hint,
        helperStyle: const TextStyle(fontSize: 11, color: AppColors.textMuted),
        isDense: true,
        border: const OutlineInputBorder(),
      ),
      onChanged: (_) => onDirty(),
      onSubmitted: (_) => onSubmit(),
    );
  }
}
