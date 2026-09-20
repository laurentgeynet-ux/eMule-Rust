import 'package:flutter/material.dart';

import 'package:ui_kit/ui_kit.dart';
import 'package:mule_client/mule_client.dart';
import '../../../../app/app_controller.dart';
import 'anti_ssrf_notice.dart';
import 'network_mode_status_card.dart';
import 'upnp_status_card.dart';

/// Network mode switch (confined lab ↔ public WAN/Kad) with UPnP
/// traversal status and refresh.
class NetworkModeDialog extends StatefulWidget {
  final AppController viewModel;

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
          _success = targetProfile ==
                  NetworkProfile.publicEd2kKadPreview
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
    final currentProfile =
        _policy?.profile ?? NetworkProfile.publicEd2kKadPreview;
    final isPublic =
        currentProfile == NetworkProfile.publicEd2kKadPreview;
    final accent =
        isPublic ? AppColors.warning : AppColors.accentBlue;

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
            Container(
              padding: const EdgeInsets.symmetric(
                horizontal: 20,
                vertical: 16,
              ),
              decoration: const BoxDecoration(
                color: AppTheme.darkCard,
                borderRadius:
                    BorderRadius.vertical(top: Radius.circular(12)),
                border: Border(
                  bottom: BorderSide(
                    color: AppTheme.darkBorder,
                    width: 0.5,
                  ),
                ),
              ),
              child: Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: accent.withValues(alpha: 0.2),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: Icon(
                      isPublic
                          ? Icons.public
                          : Icons.science_outlined,
                      color: accent,
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
                    icon: const Icon(
                      Icons.close,
                      size: 20,
                      color: AppColors.textMuted,
                    ),
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                ],
              ),
            ),
            Flexible(
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    NetworkModeStatusCard(profile: currentProfile),
                    const SizedBox(height: 16),
                    if (_upnpStatus != null) ...[
                      UpnpStatusCard(
                        status: _upnpStatus!,
                        isLoading: _isLoading,
                        onRefresh: _refreshUpnp,
                      ),
                    ],
                    const SizedBox(height: 16),
                    const AntiSsrfNotice(),
                    if (_error != null) ...[
                      const SizedBox(height: 12),
                      Text(
                        _error!,
                        style: const TextStyle(
                          color: AppTheme.statusFailed,
                          fontSize: 12,
                        ),
                      ),
                    ],
                    if (_success != null) ...[
                      const SizedBox(height: 12),
                      Text(
                        _success!,
                        style: const TextStyle(
                          color: AppColors.positive,
                          fontSize: 12,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            ),
            Container(
              padding: const EdgeInsets.symmetric(
                horizontal: 20,
                vertical: 14,
              ),
              decoration: const BoxDecoration(
                color: AppTheme.darkCard,
                borderRadius: BorderRadius.vertical(
                  bottom: Radius.circular(12),
                ),
                border: Border(
                  top: BorderSide(
                    color: AppTheme.darkBorder,
                    width: 0.5,
                  ),
                ),
              ),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  TextButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: const Text(
                      'Fermer',
                      style:
                          TextStyle(color: AppColors.textSecondary),
                    ),
                  ),
                  if (isPublic)
                    FilledButton.icon(
                      icon: const Icon(Icons.lock_outline, size: 16),
                      label: const Text(
                        'Revenir en Mode Confiné (Lab 127.0.0.1)',
                      ),
                      style: FilledButton.styleFrom(
                        backgroundColor: AppColors.labDim,
                      ),
                      onPressed: _isLoading
                          ? null
                          : () => _switchProfile(
                              NetworkProfile.localLab),
                    )
                  else
                    FilledButton.icon(
                      icon: const Icon(Icons.public, size: 16),
                      label: const Text(
                        'Activer le Mode Réseau Public (WAN)',
                      ),
                      style: FilledButton.styleFrom(
                        backgroundColor: AppColors.warningShade800,
                      ),
                      onPressed: _isLoading
                          ? null
                          : () => _switchProfile(
                              NetworkProfile.publicEd2kKadPreview),
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
