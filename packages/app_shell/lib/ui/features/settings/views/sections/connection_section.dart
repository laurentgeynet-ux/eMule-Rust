import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../../app/providers.dart';
import '../../../downloads/widgets/network/network_profile_selector.dart';
import '../../../downloads/widgets/network/network_quotas_card.dart';
import '../../../downloads/widgets/network/network_state_banner.dart';
import '../../widgets/anti_ssrf_notice.dart';
import '../../widgets/network_mode_status_card.dart';
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
  final _maxConnCtrl = TextEditingController();
  final _connPerMinCtrl = TextEditingController();
  final _tcpPortCtrl = TextEditingController();
  final _udpPortCtrl = TextEditingController();
  final _capDlCtrl = TextEditingController();
  final _capUlCtrl = TextEditingController();
  final _serverMetPathCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    Future.microtask(() async {
      await _loadData();
      await ref.read(settingsControllerProvider).fetch();
      if (mounted) {
        final s = ref.read(settingsControllerProvider).settings;
        _maxConnCtrl.text = '${s['maxConnections'] ?? ''}';
        _connPerMinCtrl.text = '${s['maxConnPerMin'] ?? ''}';
        _tcpPortCtrl.text = '${s['ed2kPort'] ?? ''}';
        _udpPortCtrl.text = '${s['kadPort'] ?? ''}';
        _capDlCtrl.text = '${s['capacityDownloadKbs'] ?? ''}';
        _capUlCtrl.text = '${s['capacityUploadKbs'] ?? ''}';
        _serverMetPathCtrl.text = '${s['serverAutoupdatePath'] ?? ''}';
      }
    });
  }

  @override
  void dispose() {
    _maxConnCtrl.dispose();
    _connPerMinCtrl.dispose();
    _tcpPortCtrl.dispose();
    _udpPortCtrl.dispose();
    _capDlCtrl.dispose();
    _capUlCtrl.dispose();
    _serverMetPathCtrl.dispose();
    super.dispose();
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
              ? 'UPnP configuré (IP WAN : ${upnp.externalIp ?? "inconnue"}).'
              : 'Passerelle UPnP non détectée ou désactivée sur votre box.';
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _upnpLoading = false;
          _upnpIsError = true;
          _upnpMessage = 'Échec de rafraîchissement UPnP : $e';
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final ctl = ref.watch(networkPolicyControllerProvider);

    return SettingsSectionScaffold(
      title: 'Connexion',
      subtitle: 'Profil réseau, isolation, budgets de sockets et UPnP',
      icon: Icons.settings_ethernet_rounded,
      children: [
        NetworkStateBanner(
          state: ctl.currentState,
          isKillSwitchActive: ctl.isKillSwitchActive,
          onDisconnectAll: () async {
            await ctl.disconnectAll();
            await ctl.fetchPolicy();
          },
          onReenable: () async {
            await ctl.enableNetwork();
            await ctl.fetchPolicy();
          },
        ),
        const SizedBox(height: 16),
        NetworkModeStatusCard(profile: ctl.currentProfile),
        const SizedBox(height: 16),
        NetworkProfileSelector(
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
            const Text(
              'Capacités & ports',
              style: TextStyle(
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
                    'Capacité réception (Ko/s)',
                    'Vitesse de ligne descendante (0 = illimité)',
                    (v) => set('capacityDownloadKbs', v),
                    min: 0,
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: _numField(
                    _capUlCtrl,
                    'Capacité émission (Ko/s)',
                    'Vitesse de ligne montante (référence USS, 0 = illimité)',
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
                    'Port TCP',
                    'Port client eD2K — rebind immédiat du listener',
                    (v) => set('ed2kPort', v),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: _numField(
                    _udpPortCtrl,
                    'Port UDP',
                    'Port Kad/UDP — socket rebind à chaud',
                    (v) => set('kadPort', v),
                  ),
                ),
              ],
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: const Text(
                'UDP activé',
                style: TextStyle(fontSize: 12, color: AppColors.textPrimary),
              ),
              subtitle: const Text(
                'Kad UDP, requêtes serveur UDP et re-ask UDP (eMule « UDP désactivé »)',
                style: TextStyle(fontSize: 10, color: AppColors.textMuted),
              ),
              value: udpEnabled,
              onChanged: (v) => set('udpEnabled', v),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: const Text(
                'Configurer les ports grâce à l\'UPnP',
                style: TextStyle(fontSize: 12, color: AppColors.textPrimary),
              ),
              subtitle: const Text(
                'Mappe/démappe automatiquement TCP+UDP sur la box',
                style: TextStyle(fontSize: 10, color: AppColors.textMuted),
              ),
              value: upnpEnabled,
              onChanged: (v) => set('upnpEnabled', v),
            ),
            const SizedBox(height: 8),
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton.icon(
                icon: const Icon(Icons.save_rounded, size: 16),
                label: const Text('Appliquer'),
                onPressed: () {
                  _submitNum(_capDlCtrl, 0, (v) => set('capacityDownloadKbs', v));
                  _submitNum(_capUlCtrl, 0, (v) => set('capacityUploadKbs', v));
                  _submitNum(_tcpPortCtrl, 1, (v) => set('ed2kPort', v));
                  _submitNum(_udpPortCtrl, 1, (v) => set('kadPort', v));
                },
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
            const Text(
              'Protocole & limites',
              style: TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: AppColors.textPrimary,
              ),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: const Text(
                'Connexions obfusquées supportées',
                style: TextStyle(fontSize: 12, color: AppColors.textPrimary),
              ),
              subtitle: const Text(
                'Protocol encryption eD2K/Kad (eMule « Support obfuscated connections »)',
                style: TextStyle(fontSize: 10, color: AppColors.textMuted),
              ),
              value: obfEnabled,
              onChanged: (v) => set('obfuscationEnabled', v),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: const Text(
                'Obfuscation requise',
                style: TextStyle(fontSize: 12, color: AppColors.textPrimary),
              ),
              subtitle: const Text(
                'Rejette les pairs sans chiffrement (eMule « Require obfuscated connections »)',
                style: TextStyle(fontSize: 10, color: AppColors.textMuted),
              ),
              value: obfRequired,
              onChanged: obfEnabled
                  ? (v) => set('obfuscationRequired', v)
                  : null,
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: const Text(
                'USS — Upload Speed Sense',
                style: TextStyle(fontSize: 12, color: AppColors.textPrimary),
              ),
              subtitle: const Text(
                'Ajuste dynamiquement la limite d\'upload selon la latence mesurée',
                style: TextStyle(fontSize: 10, color: AppColors.textMuted),
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
                    'Connexions max',
                    'Budget global de sockets pairs (eMule MaxConnections)',
                    (v) => set('maxConnections', v),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: _numField(
                    _connPerMinCtrl,
                    'Connexions / min',
                    'Nouvelles connexions sortantes par minute (eMule ≈ 240)',
                    (v) => set('maxConnPerMin', v),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 8),
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton.icon(
                icon: const Icon(Icons.save_rounded, size: 16),
                label: const Text('Appliquer'),
                onPressed: () {
                  _submitNum(_maxConnCtrl, 1, (v) => set('maxConnections', v));
                  _submitNum(_connPerMinCtrl, 1, (v) => set('maxConnPerMin', v));
                },
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
            const Text(
              'Serveurs eD2K',
              style: TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: AppColors.textPrimary,
              ),
            ),
            tile(
              'serverReconnect',
              true,
              'Reconnexion automatique',
              'Reconnecte au dernier serveur après une perte de connexion (eMule « Reconnect »)',
            ),
            tile(
              'serverAutoconnect',
              false,
              'Connexion au démarrage',
              'Se connecte automatiquement au meilleur serveur connu au lancement (eMule « Autoconnect »)',
            ),
            tile(
              'serverAutoremoveDead',
              true,
              'Supprimer les serveurs morts',
              'Retire un serveur de la liste dès le premier échec de reconnexion (eMule « DeadServerRetry » = 1)',
            ),
            tile(
              'safeConnect',
              false,
              'Connexion sûre',
              'Bloque la reprise des downloads tant que ni le serveur eD2K ni Kad ne sont disponibles',
            ),
            tile(
              'serverAutoupdateList',
              false,
              'Mettre à jour la liste au démarrage',
              'Importe le fichier server.met ci-dessous au lancement du daemon',
            ),
            const SizedBox(height: 8),
            TextField(
              controller: _serverMetPathCtrl,
              style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
              decoration: InputDecoration(
                labelText: 'Fichier server.met',
                helperText:
                    'Chemin local du fichier .met importé au démarrage (aucun fetch HTTP)',
                helperStyle:
                    const TextStyle(fontSize: 10, color: AppColors.textMuted),
                isDense: true,
                border: const OutlineInputBorder(),
                suffixIcon: IconButton(
                  icon: const Icon(Icons.save_rounded, size: 18),
                  tooltip: 'Appliquer',
                  onPressed: () => set(
                    'serverAutoupdatePath',
                    _serverMetPathCtrl.text.trim(),
                  ),
                ),
              ),
              onSubmitted: (v) => set('serverAutoupdatePath', v.trim()),
            ),
          ],
        ),
      ),
    );
  }

  void _submitNum(TextEditingController ctrl, int min, void Function(int) onSubmit) {
    final n = int.tryParse(ctrl.text.trim());
    if (n != null && n >= min) onSubmit(n);
  }

  Widget _numField(
    TextEditingController ctrl,
    String label,
    String hint,
    void Function(int) onSubmit, {
    int min = 1,
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
      onSubmitted: (v) {
        final n = int.tryParse(v.trim());
        if (n != null && n >= min) onSubmit(n);
      },
    );
  }
}
