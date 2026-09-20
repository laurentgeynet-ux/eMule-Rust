import 'package:flutter/material.dart';

import 'package:ui_kit/ui_kit.dart';
import 'package:mule_client/mule_client.dart';
import '../../downloads/view_models/downloads_view_model.dart';

class NetworkModeDialog extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const NetworkModeDialog({super.key, required this.viewModel});

  @override
  State<NetworkModeDialog> createState() => _NetworkModeDialogState();
}

class _NetworkModeDialogState extends State<NetworkModeDialog> {
  bool _isLoading = false;
  UpnpStatusModel? _upnpStatus;
  NetworkPolicySnapshot? _policy;
  String? _error;
  String? _success;

  @override
  void initState() {
    super.initState();
    _loadStatus();
  }

  Future<void> _loadStatus() async {
    setState(() => _isLoading = true);
    try {
      final policy = await widget.viewModel.client.getNetworkPolicy();
      final upnp = await widget.viewModel.client.getUpnpStatus();
      if (mounted) {
        setState(() {
          _policy = policy;
          _upnpStatus = upnp;
          _isLoading = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _error = 'Failed to load network status: $e';
          _isLoading = false;
        });
      }
    }
  }

  Future<void> _switchProfile(NetworkProfile targetProfile) async {
    setState(() {
      _isLoading = true;
      _error = null;
      _success = null;
    });
    try {
      await widget.viewModel.client.setNetworkProfile(targetProfile);
      if (targetProfile == NetworkProfile.publicEd2kKadPreview) {
        try {
          await widget.viewModel.client.mapUpnpPorts();
        } catch (_) {}
      } else {
        try {
          await widget.viewModel.client.unmapUpnpPorts();
        } catch (_) {}
      }
      await _loadStatus();
      if (mounted) {
        setState(() {
          _success = targetProfile == NetworkProfile.publicEd2kKadPreview
              ? 'Mode Réseau Public activé avec succès.'
              : 'Mode Laboratoire Local (127.0.0.1) rétabli.';
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _error = 'Erreur lors du changement de mode : $e';
          _isLoading = false;
        });
      }
    }
  }

  Future<void> _refreshUpnp() async {
    setState(() {
      _isLoading = true;
      _error = null;
      _success = null;
    });
    try {
      final upnp = await widget.viewModel.client.mapUpnpPorts();
      if (mounted) {
        setState(() {
          _upnpStatus = upnp;
          _isLoading = false;
          _success = upnp.available
              ? 'UPnP configuré avec succès (IP WAN: ${upnp.externalIp ?? "inconnue"}).'
              : 'Passerelle UPnP non détectée ou désactivée sur votre box.';
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _error = 'Échec de rafraîchissement UPnP : $e';
          _isLoading = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final currentProfile = _policy?.profile ?? NetworkProfile.publicEd2kKadPreview;
    final isPublic = currentProfile == NetworkProfile.publicEd2kKadPreview;

    return Dialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 640),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Header
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
              decoration: const BoxDecoration(
                color: AppTheme.darkCard,
                borderRadius: BorderRadius.vertical(top: Radius.circular(12)),
                border: Border(bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5)),
              ),
              child: Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: (isPublic ? AppColors.warning : AppColors.accentBlue).withValues(alpha: 0.2),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: Icon(
                      isPublic ? Icons.public : Icons.science_outlined,
                      color: isPublic ? AppColors.warning : AppColors.accentBlue,
                      size: 20,
                    ),
                  ),
                  const SizedBox(width: 12),
                  const Expanded(
                    child: Text(
                      'Mode Réseau & Traversée Box (UPnP)',
                      style: TextStyle(
                        fontSize: 16,
                        fontWeight: FontWeight.bold,
                        color: AppColors.textPrimary,
                      ),
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.close, size: 20, color: AppColors.textMuted),
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                ],
              ),
            ),

            // Content
            Flexible(
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    // Status Card
                    Container(
                      padding: const EdgeInsets.all(16),
                      decoration: BoxDecoration(
                        color: AppTheme.darkCard,
                        borderRadius: BorderRadius.circular(8),
                        border: Border.all(color: AppTheme.darkBorder),
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              Text(
                                'Profil Réseau Actuel : ',
                                style: TextStyle(color: AppColors.textPrimary.withValues(alpha: 0.7), fontSize: 13),
                              ),
                              Chip(
                                label: Text(
                                  isPublic ? 'RÉSEAU PUBLIC (WAN)' : 'LABORATOIRE (127.0.0.1)',
                                  style: TextStyle(
                                    fontSize: 11,
                                    fontWeight: FontWeight.bold,
                                    color: isPublic ? AppColors.warning : AppColors.accentBlue,
                                  ),
                                ),
                                backgroundColor: (isPublic ? AppColors.warning : AppColors.accentBlue).withValues(alpha: 0.15),
                                side: BorderSide(color: (isPublic ? AppColors.warning : AppColors.accentBlue).withValues(alpha: 0.4)),
                                padding: EdgeInsets.zero,
                              ),
                            ],
                          ),
                          const SizedBox(height: 8),
                          Text(
                            isPublic
                                ? 'Votre client peut contacter des serveurs eD2K publics, des pairs externes et la DHT Kademlia mondiale.'
                                : 'Toutes les sockets sont strictement confinées à la boucle locale 127.0.0.1. Zéro paquet sur Internet.',
                            style: const TextStyle(fontSize: 12, color: AppColors.textSoft, height: 1.4),
                          ),
                        ],
                      ),
                    ),

                    const SizedBox(height: 16),

                    // UPnP Box Status
                    if (_upnpStatus != null) ...[
                      const Text(
                        'Statut UPnP IGD (Box / Routeur NAT)',
                        style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold, color: AppColors.textPrimary),
                      ),
                      const SizedBox(height: 8),
                      Container(
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: AppTheme.darkCard,
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(color: AppTheme.darkBorder),
                        ),
                        child: Column(
                          children: [
                            Row(
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              children: [
                                const Text('Passerelle / Box détectée :', style: TextStyle(fontSize: 12, color: AppColors.textSecondary)),
                                Text(
                                  _upnpStatus!.available
                                      ? (_upnpStatus!.gatewayAddr ?? 'Connectée')
                                      : 'Non détectée / Désactivée',
                                  style: TextStyle(
                                    fontSize: 12,
                                    fontWeight: FontWeight.w600,
                                    color: _upnpStatus!.available ? AppColors.positive : AppColors.accentOrange,
                                  ),
                                ),
                              ],
                            ),
                            const Divider(color: AppTheme.darkBorder, height: 16),
                            Row(
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              children: [
                                const Text('Adresse IP Externe (WAN) :', style: TextStyle(fontSize: 12, color: AppColors.textSecondary)),
                                Text(
                                  _upnpStatus!.externalIp ?? 'Inconnue',
                                  style: const TextStyle(fontSize: 12, fontWeight: FontWeight.bold, color: AppColors.accentTeal),
                                ),
                              ],
                            ),
                            const Divider(color: AppTheme.darkBorder, height: 16),
                            Row(
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              children: [
                                Text('Port TCP eD2K (${_upnpStatus!.tcpPort}) :', style: const TextStyle(fontSize: 12, color: AppColors.textSecondary)),
                                Row(
                                  children: [
                                    Icon(
                                      _upnpStatus!.tcpMapped ? Icons.check_circle : Icons.cancel_outlined,
                                      size: 14,
                                      color: _upnpStatus!.tcpMapped ? AppColors.positive : AppColors.textFaint,
                                    ),
                                    const SizedBox(width: 4),
                                    Text(
                                      _upnpStatus!.tcpMapped ? 'Mappé (HighID)' : 'Non mappé',
                                      style: TextStyle(
                                        fontSize: 12,
                                        color: _upnpStatus!.tcpMapped ? AppColors.positive : AppColors.textSoft,
                                      ),
                                    ),
                                  ],
                                ),
                              ],
                            ),
                            const SizedBox(height: 8),
                            Row(
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              children: [
                                Text('Port UDP Kad (${_upnpStatus!.udpPort}) :', style: const TextStyle(fontSize: 12, color: AppColors.textSecondary)),
                                Row(
                                  children: [
                                    Icon(
                                      _upnpStatus!.udpMapped ? Icons.check_circle : Icons.cancel_outlined,
                                      size: 14,
                                      color: _upnpStatus!.udpMapped ? AppColors.positive : AppColors.textFaint,
                                    ),
                                    const SizedBox(width: 4),
                                    Text(
                                      _upnpStatus!.udpMapped ? 'Mappé (Ouvert)' : 'Non mappé',
                                      style: TextStyle(
                                        fontSize: 12,
                                        color: _upnpStatus!.udpMapped ? AppColors.positive : AppColors.textSoft,
                                      ),
                                    ),
                                  ],
                                ),
                              ],
                            ),
                          ],
                        ),
                      ),
                      const SizedBox(height: 10),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          OutlinedButton.icon(
                            icon: const Icon(Icons.refresh, size: 14),
                            label: const Text('Tester / Rafraîchir UPnP'),
                            style: OutlinedButton.styleFrom(
                              foregroundColor: AppColors.textSecondary,
                              side: const BorderSide(color: AppTheme.darkBorder),
                              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                            ),
                            onPressed: _isLoading ? null : _refreshUpnp,
                          ),
                        ],
                      ),
                    ],

                    const SizedBox(height: 16),

                    // Avertissement de Sécurité
                    Container(
                      padding: const EdgeInsets.all(12),
                      decoration: BoxDecoration(
                        color: AppColors.warning.withValues(alpha: 0.1),
                        borderRadius: BorderRadius.circular(8),
                        border: Border.all(color: AppColors.warning.withValues(alpha: 0.3)),
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: const [
                          Row(
                            children: [
                              Icon(Icons.security, color: AppColors.warning, size: 16),
                              SizedBox(width: 8),
                              Text(
                                'Garantie de Sécurité Anti-SSRF',
                                style: TextStyle(color: AppColors.warning, fontWeight: FontWeight.bold, fontSize: 12),
                              ),
                            ],
                          ),
                          SizedBox(height: 6),
                          Text(
                            'Même en mode public, la politique réseau d\'Emule-Sion bloque rigoureusement tout trafic sortant vers les adresses de votre réseau local privé (192.168.x.x, 10.x.x.x, 172.16-31.x.x) pour protéger vos équipements domestiques.',
                            style: TextStyle(fontSize: 11, color: AppColors.textSecondary, height: 1.4),
                          ),
                        ],
                      ),
                    ),

                    if (_error != null) ...[
                      const SizedBox(height: 12),
                      Text(_error!, style: const TextStyle(color: AppTheme.statusFailed, fontSize: 12)),
                    ],
                    if (_success != null) ...[
                      const SizedBox(height: 12),
                      Text(_success!, style: const TextStyle(color: AppColors.positive, fontSize: 12)),
                    ],
                  ],
                ),
              ),
            ),

            // Footer actions
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
              decoration: const BoxDecoration(
                color: AppTheme.darkCard,
                borderRadius: BorderRadius.vertical(bottom: Radius.circular(12)),
                border: Border(top: BorderSide(color: AppTheme.darkBorder, width: 0.5)),
              ),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  TextButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: const Text('Fermer', style: TextStyle(color: AppColors.textSecondary)),
                  ),
                  if (isPublic)
                    FilledButton.icon(
                      icon: const Icon(Icons.lock_outline, size: 16),
                      label: const Text('Revenir en Mode Confiné (Lab 127.0.0.1)'),
                      style: FilledButton.styleFrom(backgroundColor: AppColors.labDim),
                      onPressed: _isLoading ? null : () => _switchProfile(NetworkProfile.localLab),
                    )
                  else
                    FilledButton.icon(
                      icon: const Icon(Icons.public, size: 16),
                      label: const Text('Activer le Mode Réseau Public (WAN)'),
                      style: FilledButton.styleFrom(backgroundColor: AppColors.warningShade800),
                      onPressed: _isLoading ? null : () => _switchProfile(NetworkProfile.publicEd2kKadPreview),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
