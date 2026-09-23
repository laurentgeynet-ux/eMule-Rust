// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../app/providers.dart';
import '../../../../../l10n/l10n.dart';
import '../../../downloads/widgets/network/ip_filter_tab.dart';
import '../../../downloads/widgets/network/rate_limits_tab.dart';
import '../../widgets/anti_ssrf_notice.dart';
import 'settings_section_scaffold.dart';

/// « Sécurité » : filtre IP défensif (ipfilter.dat), limites de débit et
/// rappel de la politique anti-SSRF.
class SettingsSecuritySection extends ConsumerStatefulWidget {
  const SettingsSecuritySection({super.key});

  @override
  ConsumerState<SettingsSecuritySection> createState() =>
      _SettingsSecuritySectionState();
}

class _SettingsSecuritySectionState
    extends ConsumerState<SettingsSecuritySection> {
  final _filePathController = TextEditingController();
  final _downloadLimitController = TextEditingController();
  final _uploadLimitController = TextEditingController();
  final _connLimitController = TextEditingController();
  final _identityRotationController = TextEditingController();

  /// Champs en cours d'édition : jamais écrasés par une resynchronisation
  /// externe (`settings.changed`, `network.policyChanged`, autre client).
  final Set<TextEditingController> _dirty = {};

  /// Chargement initial (spinner plein cadre).
  bool _isLoading = true;

  /// Action RPC en cours (toggle filtre, import, limites) — barre fine,
  /// le contenu reste affiché au lieu d'être remplacé par un spinner.
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    Future.microtask(_loadData);
  }

  @override
  void dispose() {
    _filePathController.dispose();
    _downloadLimitController.dispose();
    _uploadLimitController.dispose();
    _connLimitController.dispose();
    _identityRotationController.dispose();
    super.dispose();
  }

  void _syncText(TextEditingController ctrl, Object? value) {
    final v = value?.toString() ?? '';
    if (!_dirty.contains(ctrl) && ctrl.text != v) ctrl.text = v;
  }

  void _syncFromLimits(RateLimitsModel? limits) {
    if (limits == null) return;
    // On affiche la limite *configurée* (persistée), pas l'effectif du
    // bucket : celui-ci peut être borné par le ratio eD2K ou piloté par
    // USS — la différence est expliquée en helper dans RateLimitsTab.
    _syncText(
      _downloadLimitController,
      limits.downloadConfiguredBytesPerSec ~/ 1024,
    );
    _syncText(
      _uploadLimitController,
      limits.uploadConfiguredBytesPerSec ~/ 1024,
    );
    _syncText(_connLimitController, limits.connectionsPerMinute);
  }

  void _syncFromSettings() {
    final s = ref.read(settingsControllerProvider).settings;
    _syncText(_identityRotationController, s['identityRotationHours'] ?? 0);
  }

  Future<void> _loadData() async {
    final ctl = ref.read(networkPolicyControllerProvider);
    await ctl.fetchFilterStatus();
    await ctl.fetchRateLimits();

    if (mounted) {
      _syncFromLimits(ref.read(networkPolicyControllerProvider).rateLimits);
    }
    await ref.read(settingsControllerProvider).fetch();
    if (mounted) {
      _syncFromSettings();
      setState(() => _isLoading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(networkPolicyControllerProvider, (_, next) {
      _syncFromLimits(next.rateLimits);
    });
    ref.listen(settingsControllerProvider, (_, __) => _syncFromSettings());
    final ctl = ref.watch(networkPolicyControllerProvider);
    final l10n = context.l10n;

    return SettingsSectionScaffold(
      title: l10n.settingsSectionSecurity,
      subtitle: l10n.settingsSecuritySubtitle,
      icon: Icons.security_rounded,
      children: [
        const AntiSsrfNotice(),
        const SizedBox(height: 16),
        if (_isLoading)
          const Center(child: CircularProgressIndicator())
        else ...[
          if (_busy)
            const Padding(
              padding: EdgeInsets.only(bottom: 8),
              child: LinearProgressIndicator(minHeight: 2),
            ),
          IpFilterTab(
            status: ctl.filterStatus,
            filePathController: _filePathController,
            onToggle: (enabled) async {
              setState(() => _busy = true);
              await ctl.setFilterEnabled(enabled);
              if (mounted) setState(() => _busy = false);
            },
            onImport: (path) async {
              setState(() => _busy = true);
              final ok = await ctl.importFilterFile(path);
              if (mounted) {
                setState(() => _busy = false);
                if (ok) _filePathController.clear();
              }
            },
            onClear: () async {
              setState(() => _busy = true);
              await ctl.clearFilter();
              if (mounted) setState(() => _busy = false);
            },
          ),
          const SizedBox(height: 20),
          RateLimitsTab(
            downloadLimitController: _downloadLimitController,
            uploadLimitController: _uploadLimitController,
            connLimitController: _connLimitController,
            limits: ctl.rateLimits,
            onSave: _saveLimits,
            onChanged: (ctrl) => _dirty.add(ctrl),
          ),
          const SizedBox(height: 20),
          const Divider(color: AppTheme.darkBorder),
          const SizedBox(height: 8),
          Text(
            l10n.settingsExclusiveTitle,
            style: const TextStyle(
              fontSize: 12,
              fontWeight: FontWeight.w600,
              color: AppColors.textPrimary,
            ),
          ),
          const SizedBox(height: 8),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            dense: true,
            title: Text(
              l10n.settingsParanoidTitle,
              style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
            ),
            subtitle: Text(
              l10n.settingsParanoidSub,
              style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
            ),
            value: ref
                    .watch(settingsControllerProvider)
                    .settings['paranoidMode'] as bool? ??
                false,
            onChanged: (v) => ref
                .read(settingsControllerProvider)
                .updateSettings({'paranoidMode': v}),
          ),
          const SizedBox(height: 8),
          TextField(
            controller: _identityRotationController,
            keyboardType: TextInputType.number,
            style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
            decoration: InputDecoration(
              labelText: l10n.settingsIdentityRotationLabel,
              helperText: l10n.settingsIdentityRotationHelper,
              helperStyle:
                  const TextStyle(fontSize: 11, color: AppColors.textMuted),
              isDense: true,
              border: const OutlineInputBorder(),
              suffixIcon: IconButton(
                icon: const Icon(Icons.save_rounded, size: 18),
                tooltip: l10n.settingsApply,
                onPressed: _saveIdentityRotation,
              ),
            ),
            onChanged: (_) => _dirty.add(_identityRotationController),
            onSubmitted: (_) => _saveIdentityRotation(),
          ),
        ],
      ],
    );
  }

  Future<void> _saveLimits() async {
    final l10n = context.l10n;
    final dlKb = int.tryParse(_downloadLimitController.text.trim());
    final ulKb = int.tryParse(_uploadLimitController.text.trim());
    final conn = int.tryParse(_connLimitController.text.trim());
    final invalid = <String>[
      if (dlKb == null || dlKb < 0) l10n.settingsLimitDlField,
      if (ulKb == null || ulKb < 0) l10n.settingsLimitUlField,
      if (conn == null || conn <= 0) l10n.settingsLimitConnField,
    ];
    if (invalid.isNotEmpty) {
      ref
          .read(shellControllerProvider)
          .setError(l10n.settingsInvalidLimits(invalid.join(', ')));
      return;
    }

    final current = ref.read(networkPolicyControllerProvider).rateLimits;
    final limits = RateLimitsModel(
      downloadBytesPerSec: dlKb! * 1024,
      uploadBytesPerSec: ulKb! * 1024,
      downloadConfiguredBytesPerSec: dlKb * 1024,
      uploadConfiguredBytesPerSec: ulKb * 1024,
      connectionsPerMinute: conn!,
      serverRequestsPerSec: current?.serverRequestsPerSec ?? 10,
      kadPacketsPerSec: current?.kadPacketsPerSec ?? 50,
    );

    setState(() => _busy = true);
    await ref.read(networkPolicyControllerProvider).setRateLimits(limits);
    // Le daemon peut clamper les limites à la capacité de ligne déclarée
    // (eMule : limite ≤ 90 %/80 % de la capacité) — on reflète les valeurs
    // effectivement appliquées.
    if (mounted) {
      _dirty
        ..remove(_downloadLimitController)
        ..remove(_uploadLimitController)
        ..remove(_connLimitController);
      _syncFromLimits(ref.read(networkPolicyControllerProvider).rateLimits);
      setState(() => _busy = false);
    }
  }

  Future<void> _saveIdentityRotation() async {
    final l10n = context.l10n;
    final raw = _identityRotationController.text.trim();
    final hours = int.tryParse(raw);
    if (hours == null || hours < 0 || hours > 8760) {
      ref
          .read(shellControllerProvider)
          .setError(l10n.settingsIdentityRotationInvalid(raw));
      return;
    }
    final ok = await ref
        .read(settingsControllerProvider)
        .updateSettings({'identityRotationHours': hours});
    if (ok && mounted) {
      _dirty.remove(_identityRotationController);
      _syncFromSettings();
    }
  }
}
