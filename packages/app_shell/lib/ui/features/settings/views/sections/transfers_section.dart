import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../../app/providers.dart';
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

  void _syncFromSettings() {
    final s = ref.read(settingsControllerProvider).settings;
    _maxGlobalCtrl.text = '${s['maxGlobalDownloadPeers'] ?? ''}';
    _maxPerFileCtrl.text = '${s['maxPeersPerDownload'] ?? ''}';
    _minPerFileCtrl.text = '${s['minPeersPerDownload'] ?? ''}';
    _maxQueueCtrl.text = '${s['maxUploadQueue'] ?? ''}';
    _minUploadCtrl.text = '${s['minUploadKbs'] ?? '1'}';
    _dailyQuotaCtrl.text = '${s['dailyUploadQuotaGb'] ?? '0'}';
    if (mounted) setState(() => _loaded = true);
  }

  @override
  Widget build(BuildContext context) {
    if (!_loaded) {
      return const Center(child: CircularProgressIndicator());
    }

    return SettingsSectionScaffold(
      title: 'Transferts',
      subtitle: 'Limites de connexions aux sources du swarm',
      icon: Icons.swap_vert_rounded,
      children: [
        _NumberField(
          controller: _maxGlobalCtrl,
          label: 'Sources maximum (global)',
          hint: 'Nombre total de sources eD2K/Kad simultanées, tous fichiers confondus',
        ),
        const SizedBox(height: 12),
        _NumberField(
          controller: _maxPerFileCtrl,
          label: 'Sources maximum par fichier',
          hint: 'Plafond par défaut — surchargeable par fichier (FT_MAXSOURCES)',
        ),
        const SizedBox(height: 12),
        _NumberField(
          controller: _minPerFileCtrl,
          label: 'Sources minimum par fichier',
          hint: 'Quota de sources garanti par fichier même si le swarm global est saturé',
        ),
        const SizedBox(height: 12),
        _NumberField(
          controller: _maxQueueCtrl,
          label: 'File d\'attente upload max',
          hint: 'Clients en attente d\'un slot upload (eMule QueueSize, défaut 5000)',
        ),
        const SizedBox(height: 12),
        _NumberField(
          controller: _minUploadCtrl,
          label: 'Débit upload min par slot (Ko/s)',
          hint: 'N\'ouvre pas de nouveau slot sous ce seuil (eMule MinUpload, défaut 1)',
        ),
        const SizedBox(height: 12),
        _NumberField(
          controller: _dailyQuotaCtrl,
          label: 'Quota upload quotidien (Gio, 0 = illimité)',
          hint:
              'Option Emule-Sion : au-delà du quota, plus aucun slot '
              'd\'upload n\'est accordé. Remise à zéro chaque jour (UTC).',
        ),
        const SizedBox(height: 8),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: const Text(
            'Priorité auto des nouveaux downloads',
            style: TextStyle(fontSize: 12, color: AppColors.textPrimary),
          ),
          subtitle: const Text(
            'Les fichiers ≥ 1 part eD2K (9,28 Mio) démarrent en priorité « high »',
            style: TextStyle(fontSize: 10, color: AppColors.textMuted),
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
          title: const Text(
            'Sauvetage de swarm (swarm healing)',
            style: TextStyle(fontSize: 12, color: AppColors.textPrimary),
          ),
          subtitle: const Text(
            'Option Emule-Sion : télécharge en priorité les parts les plus '
            'rares du swarm (peu de sources restantes) pour devenir une '
            'source de secours avant leur disparition',
            style: TextStyle(fontSize: 10, color: AppColors.textMuted),
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
          label: Text(_isSaving ? 'Enregistrement…' : 'Appliquer'),
          onPressed: _isSaving ? null : _save,
        ),
      ],
    );
  }

  Future<void> _save() async {
    setState(() => _isSaving = true);
    final patch = <String, dynamic>{};
    final g = int.tryParse(_maxGlobalCtrl.text.trim());
    final f = int.tryParse(_maxPerFileCtrl.text.trim());
    final m = int.tryParse(_minPerFileCtrl.text.trim());
    if (g != null && g > 0) patch['maxGlobalDownloadPeers'] = g;
    if (f != null && f > 0) patch['maxPeersPerDownload'] = f;
    if (m != null && m >= 0) patch['minPeersPerDownload'] = m;
    final q = int.tryParse(_maxQueueCtrl.text.trim());
    if (q != null && q >= 10) patch['maxUploadQueue'] = q;
    final mu = int.tryParse(_minUploadCtrl.text.trim());
    if (mu != null && mu >= 0) patch['minUploadKbs'] = mu;
    final quota = int.tryParse(_dailyQuotaCtrl.text.trim());
    if (quota != null && quota >= 0) patch['dailyUploadQuotaGb'] = quota;
    await ref.read(settingsControllerProvider).updateSettings(patch);
    if (mounted) {
      setState(() => _isSaving = false);
      _syncFromSettings();
    }
  }
}

class _NumberField extends StatelessWidget {
  final TextEditingController controller;
  final String label;
  final String hint;

  const _NumberField({
    required this.controller,
    required this.label,
    required this.hint,
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
    );
  }
}
