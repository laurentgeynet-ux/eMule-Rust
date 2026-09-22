import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../app/providers.dart';
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
  bool _isLoading = false;

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

  Future<void> _loadData() async {
    setState(() => _isLoading = true);
    final ctl = ref.read(networkPolicyControllerProvider);
    await ctl.fetchFilterStatus();
    await ctl.fetchRateLimits();

    final limits = ref.read(networkPolicyControllerProvider).rateLimits;
    if (limits != null) {
      _downloadLimitController.text =
          (limits.downloadBytesPerSec ~/ 1024).toString();
      _uploadLimitController.text =
          (limits.uploadBytesPerSec ~/ 1024).toString();
      _connLimitController.text =
          limits.connectionsPerMinute.toString();
    }
    await ref.read(settingsControllerProvider).fetch();
    if (mounted) {
      final s = ref.read(settingsControllerProvider).settings;
      _identityRotationController.text =
          '${s['identityRotationHours'] ?? 0}';
    }
    if (mounted) setState(() => _isLoading = false);
  }

  @override
  Widget build(BuildContext context) {
    final ctl = ref.watch(networkPolicyControllerProvider);

    return SettingsSectionScaffold(
      title: 'Sécurité',
      subtitle: 'Filtre IP, limites de débit et politique anti-SSRF',
      icon: Icons.security_rounded,
      children: [
        const AntiSsrfNotice(),
        const SizedBox(height: 16),
        if (_isLoading)
          const Center(child: CircularProgressIndicator())
        else ...[
          IpFilterTab(
            status: ctl.filterStatus,
            filePathController: _filePathController,
            onToggle: (enabled) async {
              setState(() => _isLoading = true);
              await ctl.setFilterEnabled(enabled);
              if (mounted) setState(() => _isLoading = false);
            },
            onImport: (path) async {
              setState(() => _isLoading = true);
              final ok = await ctl.importFilterFile(path);
              if (mounted) {
                setState(() => _isLoading = false);
                if (ok) _filePathController.clear();
              }
            },
            onClear: () async {
              setState(() => _isLoading = true);
              await ctl.clearFilter();
              if (mounted) setState(() => _isLoading = false);
            },
          ),
          const SizedBox(height: 20),
          RateLimitsTab(
            downloadLimitController: _downloadLimitController,
            uploadLimitController: _uploadLimitController,
            connLimitController: _connLimitController,
            onSave: _saveLimits,
          ),
          const SizedBox(height: 20),
          const Divider(color: AppTheme.darkBorder),
          const SizedBox(height: 8),
          const Text(
            'Options exclusives Emule-Sion',
            style: TextStyle(
              fontSize: 12,
              fontWeight: FontWeight.w600,
              color: AppColors.textPrimary,
            ),
          ),
          const SizedBox(height: 8),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            dense: true,
            title: const Text(
              'Mode paranoid',
              style: TextStyle(fontSize: 12, color: AppColors.textPrimary),
            ),
            subtitle: const Text(
              'Ne sert que les pairs obfusqués dont la signature Secure '
              'Ident a été vérifiée — les clients eMule classiques sont '
              'refusés (réduit fortement la compatibilité)',
              style: TextStyle(fontSize: 10, color: AppColors.textMuted),
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
              labelText: 'Rotation d\'identité (heures, 0 = désactivée)',
              helperText:
                  'Régénère la clé RSA, le user hash eD2K et le KadID '
                  'périodiquement. Invalide les crédits accumulés — identité '
                  'volontairement éphémère.',
              helperStyle:
                  const TextStyle(fontSize: 11, color: AppColors.textMuted),
              isDense: true,
              border: const OutlineInputBorder(),
              suffixIcon: IconButton(
                icon: const Icon(Icons.save_rounded, size: 18),
                onPressed: _saveIdentityRotation,
              ),
            ),
            onSubmitted: (_) => _saveIdentityRotation(),
          ),
        ],
      ],
    );
  }

  Future<void> _saveLimits() async {
    final dlKb = int.tryParse(_downloadLimitController.text.trim()) ?? 0;
    final ulKb = int.tryParse(_uploadLimitController.text.trim()) ?? 0;
    final current = ref.read(networkPolicyControllerProvider).rateLimits;
    final conn = int.tryParse(_connLimitController.text.trim()) ??
        current?.connectionsPerMinute ??
        240;

    final limits = RateLimitsModel(
      downloadBytesPerSec: dlKb * 1024,
      uploadBytesPerSec: ulKb * 1024,
      connectionsPerMinute: conn,
      serverRequestsPerSec: current?.serverRequestsPerSec ?? 10,
      kadPacketsPerSec: current?.kadPacketsPerSec ?? 50,
    );

    setState(() => _isLoading = true);
    await ref.read(networkPolicyControllerProvider).setRateLimits(limits);
    if (mounted) setState(() => _isLoading = false);
  }

  Future<void> _saveIdentityRotation() async {
    final hours = int.tryParse(_identityRotationController.text.trim()) ?? 0;
    if (hours < 0 || hours > 8760) return;
    await ref
        .read(settingsControllerProvider)
        .updateSettings({'identityRotationHours': hours});
  }
}
