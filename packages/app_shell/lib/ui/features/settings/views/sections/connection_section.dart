// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../../app/providers.dart';
import '../../../../../l10n/l10n.dart';
import '../../../downloads/widgets/network/network_quotas_card.dart';
import '../../../downloads/widgets/network/network_state_banner.dart';
import '../../widgets/anti_ssrf_notice.dart';
import '../../widgets/network_toggles_card.dart';
import '../../widgets/upnp_status_card.dart';
import 'settings_section_scaffold.dart';

/// « Connexion » : profil réseau (lab/WAN), kill switch, budgets de
/// sockets et statut UPnP/mappings de ports.
class SettingsConnectionSection extends ConsumerStatefulWidget {
  const SettingsConnectionSection({super.key});

  @override
  ConsumerState<SettingsConnectionSection> createState() =>
      _SettingsConnectionSectionState();
}

class _SettingsConnectionSectionState
    extends ConsumerState<SettingsConnectionSection> {
  UpnpStatusModel? _upnpStatus;
  bool _upnpLoading = false;
  String? _upnpMessage;
  bool _upnpIsError = false;
  StreamSubscription<UpnpStatusModel>? _upnpSub;
  final _maxConnCtrl = TextEditingController();
  final _connPerMinCtrl = TextEditingController();
  final _tcpPortCtrl = TextEditingController();
  final _udpPortCtrl = TextEditingController();
  final _capDlCtrl = TextEditingController();
  final _capUlCtrl = TextEditingController();
  final _serverMetPathCtrl = TextEditingController();

  /// Champs en cours d'édition : jamais écrasés par une resynchronisation
  /// externe (`settings.changed`, autre client, MCP).
  final Set<TextEditingController> _dirty = {};

  @override
  void initState() {
    super.initState();
    // `upnp.statusChanged` pousse le snapshot complet (enable/map/unmap,
    // retries en arrière-plan) — la carte se rafraîchit sans action.
    _upnpSub = ref
        .read(muleDaemonClientProvider)
        .upnpStatusEvents
        .listen((s) {
          if (mounted) setState(() => _upnpStatus = s);
        });
    Future.microtask(() async {
      await _loadData();
      await ref.read(settingsControllerProvider).fetch();
      if (mounted) _syncFromSettings();
    });
  }

  @override
  void dispose() {
    _upnpSub?.cancel();
    _maxConnCtrl.dispose();
    _connPerMinCtrl.dispose();
    _tcpPortCtrl.dispose();
    _udpPortCtrl.dispose();
    _capDlCtrl.dispose();
    _capUlCtrl.dispose();
    _serverMetPathCtrl.dispose();
    super.dispose();
  }

  void _syncText(TextEditingController ctrl, Object? value) {
    final v = value?.toString() ?? '';
    if (!_dirty.contains(ctrl) && ctrl.text != v) ctrl.text = v;
  }

  /// Réécrit les champs depuis la map de settings, sauf ceux en cours
  /// d'édition par l'utilisateur.
  void _syncFromSettings() {
    final s = ref.read(settingsControllerProvider).settings;
    _syncText(_maxConnCtrl, s['maxConnections']);
    _syncText(_connPerMinCtrl, s['maxConnPerMin']);
    _syncText(_tcpPortCtrl, s['ed2kPort']);
    _syncText(_udpPortCtrl, s['kadPort']);
    _syncText(_capDlCtrl, s['capacityDownloadKbs']);
    _syncText(_capUlCtrl, s['capacityUploadKbs']);
    _syncText(_serverMetPathCtrl, s['serverAutoupdatePath']);
  }

  Future<void> _loadData() async {
    await ref.read(networkPolicyControllerProvider).fetchPolicy();
    await _loadUpnp();
  }

  Future<void> _loadUpnp() async {
    try {
      final upnp = await ref.read(muleDaemonClientProvider).getUpnpStatus();
      if (mounted) setState(() => _upnpStatus = upnp);
    } catch (_) {
      // UPnP indisponible (daemon ancien ou profil lab) — carte masquée.
    }
  }

  Future<void> _switchProfile(NetworkProfile target) async {
    final ctl = ref.read(networkPolicyControllerProvider);
    await ctl.setProfile(target);
    try {
      if (target == NetworkProfile.publicEd2kKadPreview) {
        await ref.read(muleDaemonClientProvider).mapUpnpPorts();
      } else {
        await ref.read(muleDaemonClientProvider).unmapUpnpPorts();
      }
    } catch (_) {}
    await ctl.fetchPolicy();
    await _loadUpnp();
  }

  Future<void> _refreshUpnp() async {
    final l10n = context.l10n;
    setState(() {
      _upnpLoading = true;
      _upnpMessage = null;
    });
    try {
      final upnp = await ref.read(muleDaemonClientProvider).mapUpnpPorts();
      if (mounted) {
        setState(() {
          _upnpStatus = upnp;
          _upnpLoading = false;
          _upnpIsError = !upnp.available;
          _upnpMessage = upnp.available
              ? l10n.upnpConfiguredMsg(upnp.externalIp ?? l10n.upnpIpUnknown)
              : l10n.upnpNoGateway;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _upnpLoading = false;
          _upnpIsError = true;
          _upnpMessage = l10n.upnpRefreshFailed('$e');
        });
      }
    }
  }

  /// Le kill switch coupe immédiatement toutes les sockets réseau —
  /// confirmation explicite avant de couper (action destructive pour les
  /// transferts en cours).
  Future<void> _confirmDisconnectAll() async {
    final l10n = context.l10n;
    final confirm = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.darkSurface,
        title: Text(l10n.killSwitchConfirmTitle),
        content: Text(l10n.killSwitchConfirmBody),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: Text(l10n.btnCancel),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: AppColors.dangerShade700,
            ),
            onPressed: () => Navigator.of(ctx).pop(true),
            child: Text(l10n.killSwitchConfirmAction),
          ),
        ],
      ),
    );
    if (confirm == true) {
      final ctl = ref.read(networkPolicyControllerProvider);
      await ctl.disconnectAll();
      await ctl.fetchPolicy();
    }
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(settingsControllerProvider, (_, __) => _syncFromSettings());
    final ctl = ref.watch(networkPolicyControllerProvider);
    final l10n = context.l10n;

    return SettingsSectionScaffold(
      title: l10n.settingsSectionConnection,
      subtitle: l10n.settingsConnectionSubtitle,
      icon: Icons.settings_ethernet_rounded,
      children: [
        NetworkStateBanner(
          state: ctl.currentState,
          isKillSwitchActive: ctl.isKillSwitchActive,
          onDisconnectAll: _confirmDisconnectAll,
          onReenable: () async {
            await ctl.enableNetwork();
            await ctl.fetchPolicy();
          },
        ),
        const SizedBox(height: 16),
        NetworkTogglesCard(
          active: ctl.currentProfile,
          onSelected: _switchProfile,
        ),
        const SizedBox(height: 16),
        NetworkQuotasCard(policy: ctl.policy),
        const SizedBox(height: 16),
        _buildPortsCard(context),
        const SizedBox(height: 16),
        _buildProtocolCard(context),
        const SizedBox(height: 16),
        _buildServersCard(context),
        const SizedBox(height: 16),
        if (_upnpStatus != null)
          UpnpStatusCard(
            status: _upnpStatus!,
            isLoading: _upnpLoading,
            onRefresh: _refreshUpnp,
          ),
        if (_upnpMessage != null) ...[
          const SizedBox(height: 8),
          Text(
            _upnpMessage!,
            style: TextStyle(
              fontSize: 12,
              color: _upnpIsError
                  ? AppTheme.statusFailed
                  : AppColors.positive,
            ),
          ),
        ],
        const SizedBox(height: 16),
        const AntiSsrfNotice(),
      ],
    );
  }

  /// Capacités & ports clients — eMule Preferences → Connexion
  /// (débits de ligne, ports TCP/UDP, toggle UDP, UPnP).
  Widget _buildPortsCard(BuildContext context) {
    final l10n = context.l10n;
    final settings = ref.watch(settingsControllerProvider).settings;
    final udpEnabled = settings['udpEnabled'] as bool? ?? true;
    final upnpEnabled = settings['upnpEnabled'] as bool? ?? false;

    void set(String key, dynamic value) =>
        ref.read(settingsControllerProvider).updateSettings({key: value});

    return Card(
      color: AppTheme.darkSurface,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              l10n.settingsCapPortsTitle,
              style: const TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: AppColors.textPrimary,
              ),
            ),
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(
                  child: _numField(
                    _capDlCtrl,
                    l10n.settingsCapDlLabel,
                    l10n.settingsCapDlHint,
                    (v) => set('capacityDownloadKbs', v),
                    min: 0,
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: _numField(
                    _capUlCtrl,
                    l10n.settingsCapUlLabel,
                    l10n.settingsCapUlHint,
                    (v) => set('capacityUploadKbs', v),
                    min: 0,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: _numField(
                    _tcpPortCtrl,
                    l10n.settingsTcpPortLabel,
                    l10n.settingsTcpPortHint,
                    (v) => set('ed2kPort', v),
                    max: 65535,
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: _numField(
                    _udpPortCtrl,
                    l10n.settingsUdpPortLabel,
                    l10n.settingsUdpPortHint,
                    (v) => set('kadPort', v),
                    max: 65535,
                  ),
                ),
              ],
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: Text(
                l10n.settingsUdpEnabledTitle,
                style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
              ),
              subtitle: Text(
                l10n.settingsUdpEnabledSub,
                style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
              ),
              value: udpEnabled,
              onChanged: (v) => set('udpEnabled', v),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: Text(
                l10n.settingsUpnpToggleTitle,
                style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
              ),
              subtitle: Text(
                l10n.settingsUpnpToggleSub,
                style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
              ),
              value: upnpEnabled,
              onChanged: (v) => set('upnpEnabled', v),
            ),
            const SizedBox(height: 8),
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton.icon(
                icon: const Icon(Icons.save_rounded, size: 16),
                label: Text(l10n.settingsApply),
                onPressed: () => _applyNumeric([
                  (ctrl: _capDlCtrl, label: l10n.settingsCapDlLabel, key: 'capacityDownloadKbs', min: 0, max: null),
                  (ctrl: _capUlCtrl, label: l10n.settingsCapUlLabel, key: 'capacityUploadKbs', min: 0, max: null),
                  (ctrl: _tcpPortCtrl, label: l10n.settingsTcpPortLabel, key: 'ed2kPort', min: 1, max: 65535),
                  (ctrl: _udpPortCtrl, label: l10n.settingsUdpPortLabel, key: 'kadPort', min: 1, max: 65535),
                ]),
              ),
            ),
          ],
        ),
      ),
    );
  }

  /// Obfuscation (protocol encryption), connection counters and USS —
  /// eMule Preferences → Connection / Extended.
  Widget _buildProtocolCard(BuildContext context) {
    final l10n = context.l10n;
    final settings = ref.watch(settingsControllerProvider).settings;
    final obfEnabled = settings['obfuscationEnabled'] as bool? ?? true;
    final obfRequired = settings['obfuscationRequired'] as bool? ?? false;
    final ussEnabled = settings['ussEnabled'] as bool? ?? false;

    void set(String key, dynamic value) =>
        ref.read(settingsControllerProvider).updateSettings({key: value});

    return Card(
      color: AppTheme.darkSurface,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              l10n.settingsProtocolTitle,
              style: const TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: AppColors.textPrimary,
              ),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: Text(
                l10n.settingsObfEnabledTitle,
                style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
              ),
              subtitle: Text(
                l10n.settingsObfEnabledSub,
                style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
              ),
              value: obfEnabled,
              onChanged: (v) => set('obfuscationEnabled', v),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: Text(
                l10n.settingsObfRequiredTitle,
                style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
              ),
              subtitle: Text(
                l10n.settingsObfRequiredSub,
                style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
              ),
              value: obfRequired,
              onChanged: obfEnabled
                  ? (v) => set('obfuscationRequired', v)
                  : null,
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: Text(
                l10n.settingsUssTitle,
                style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
              ),
              subtitle: Text(
                l10n.settingsUssSub,
                style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
              ),
              value: ussEnabled,
              onChanged: (v) => set('ussEnabled', v),
            ),
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(
                  child: _numField(
                    _maxConnCtrl,
                    l10n.settingsMaxConnLabel,
                    l10n.settingsMaxConnHint,
                    (v) => set('maxConnections', v),
                    max: 65000,
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: _numField(
                    _connPerMinCtrl,
                    l10n.settingsConnPerMinLabel,
                    l10n.settingsConnPerMinHint,
                    (v) => set('maxConnPerMin', v),
                    max: 1200,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 8),
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton.icon(
                icon: const Icon(Icons.save_rounded, size: 16),
                label: Text(l10n.settingsApply),
                onPressed: () => _applyNumeric([
                  (ctrl: _maxConnCtrl, label: l10n.settingsMaxConnLabel, key: 'maxConnections', min: 1, max: 65000),
                  (ctrl: _connPerMinCtrl, label: l10n.settingsConnPerMinLabel, key: 'maxConnPerMin', min: 1, max: 1200),
                ]),
              ),
            ),
          ],
        ),
      ),
    );
  }

  /// Serveurs eD2K — eMule Preferences → Server : auto-reconnect,
  /// autoconnect au démarrage, suppression des serveurs morts, mise à jour
  /// de la liste depuis un fichier `server.met` local, connexion sûre.
  Widget _buildServersCard(BuildContext context) {
    final l10n = context.l10n;
    final settings = ref.watch(settingsControllerProvider).settings;
    bool b(String key, bool def) => settings[key] as bool? ?? def;

    void set(String key, dynamic value) =>
        ref.read(settingsControllerProvider).updateSettings({key: value});

    Widget tile(String key, bool def, String title, String sub) =>
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: Text(
            title,
            style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
          ),
          subtitle: Text(
            sub,
            style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
          ),
          value: b(key, def),
          onChanged: (v) => set(key, v),
        );

    return Card(
      color: AppTheme.darkSurface,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              l10n.settingsServersTitle,
              style: const TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: AppColors.textPrimary,
              ),
            ),
            tile(
              'serverReconnect',
              true,
              l10n.settingsServerReconnectTitle,
              l10n.settingsServerReconnectSub,
            ),
            tile(
              'serverAutoconnect',
              false,
              l10n.settingsServerAutoconnectTitle,
              l10n.settingsServerAutoconnectSub,
            ),
            tile(
              'serverAutoremoveDead',
              true,
              l10n.settingsServerAutoremoveTitle,
              l10n.settingsServerAutoremoveSub,
            ),
            tile(
              'safeConnect',
              false,
              l10n.settingsSafeConnectTitle,
              l10n.settingsSafeConnectSub,
            ),
            tile(
              'serverAutoupdateList',
              false,
              l10n.settingsServerAutoupdateTitle,
              l10n.settingsServerAutoupdateSub,
            ),
            const SizedBox(height: 8),
            TextField(
              controller: _serverMetPathCtrl,
              style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
              decoration: InputDecoration(
                labelText: l10n.settingsServerMetLabel,
                helperText: l10n.settingsServerMetHint,
                helperStyle:
                    const TextStyle(fontSize: 10, color: AppColors.textMuted),
                isDense: true,
                border: const OutlineInputBorder(),
                suffixIcon: IconButton(
                  icon: const Icon(Icons.save_rounded, size: 18),
                  tooltip: l10n.settingsApply,
                  onPressed: _saveServerMetPath,
                ),
              ),
              onChanged: (_) => _dirty.add(_serverMetPathCtrl),
              onSubmitted: (_) => _saveServerMetPath(),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _saveServerMetPath() async {
    final ok = await ref.read(settingsControllerProvider).updateSettings(
      {'serverAutoupdatePath': _serverMetPathCtrl.text.trim()},
    );
    if (ok && mounted) {
      _dirty.remove(_serverMetPathCtrl);
      _syncFromSettings();
    }
  }

  /// Parse + validation d'un champ numérique. Champ vide = valeur inchangée
  /// (`invalid: false`, `value: null`) ; saisie hors bornes = erreur
  /// remontée via la bannière globale (`invalid: true`).
  ({int? value, bool invalid}) _parseNum(
    String label,
    String raw, {
    required int min,
    int? max,
  }) {
    if (raw.isEmpty) return (value: null, invalid: false);
    final n = int.tryParse(raw);
    if (n == null || n < min || (max != null && n > max)) {
      final l10n = context.l10n;
      ref.read(shellControllerProvider).setError(
        max != null
            ? l10n.settingsInvalidNumRange(label, raw, min, max)
            : l10n.settingsInvalidNumMin(label, raw, min),
      );
      return (value: null, invalid: true);
    }
    return (value: n, invalid: false);
  }

  /// Bouton « Appliquer » : valide tous les champs puis envoie un patch
  /// `settings.set` unique — aucune application partielle si un champ est
  /// invalide.
  Future<void> _applyNumeric(
    List<({TextEditingController ctrl, String label, String key, int min, int? max})>
    fields,
  ) async {
    final patch = <String, dynamic>{};
    var invalid = false;
    for (final f in fields) {
      final r = _parseNum(f.label, f.ctrl.text.trim(), min: f.min, max: f.max);
      if (r.invalid) {
        invalid = true;
      } else if (r.value != null) {
        patch[f.key] = r.value;
      }
    }
    if (invalid || patch.isEmpty) return;
    final ok =
        await ref.read(settingsControllerProvider).updateSettings(patch);
    if (ok && mounted) {
      for (final f in fields) {
        if (patch.containsKey(f.key)) _dirty.remove(f.ctrl);
      }
      _syncFromSettings();
    }
  }

  Widget _numField(
    TextEditingController ctrl,
    String label,
    String hint,
    void Function(int) onSubmit, {
    int min = 1,
    int? max,
  }) {
    return TextField(
      controller: ctrl,
      keyboardType: TextInputType.number,
      style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
      decoration: InputDecoration(
        labelText: label,
        helperText: hint,
        helperStyle: const TextStyle(fontSize: 10, color: AppColors.textMuted),
        isDense: true,
        border: const OutlineInputBorder(),
      ),
      onChanged: (_) => _dirty.add(ctrl),
      onSubmitted: (v) {
        final r = _parseNum(label, v.trim(), min: min, max: max);
        if (r.value != null) {
          _dirty.remove(ctrl);
          onSubmit(r.value!);
        }
      },
    );
  }
}
