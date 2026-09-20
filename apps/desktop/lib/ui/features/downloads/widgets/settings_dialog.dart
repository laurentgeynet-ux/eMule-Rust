import 'package:flutter/material.dart';

import 'package:ui_kit/ui_kit.dart';
import 'package:mule_client/mule_client.dart';
import '../view_models/downloads_view_model.dart';
import '../../settings/widgets/network_mode_dialog.dart';

class SettingsDialog extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const SettingsDialog({super.key, required this.viewModel});

  @override
  State<SettingsDialog> createState() => _SettingsDialogState();
}

class _SettingsDialogState extends State<SettingsDialog> {
  late TextEditingController _dirController;
  bool _isSaving = false;
  String? _inlineError;
  String? _inlineSuccess;

  @override
  void initState() {
    super.initState();
    final currentDir = widget.viewModel.defaultDownloadDirectory.path;
    _dirController = TextEditingController(text: currentDir);
    widget.viewModel.fetchSettings();
  }

  @override
  void dispose() {
    _dirController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.viewModel,
      builder: (context, _) {
        final isCustom = widget.viewModel.settings['isCustom'] == true ||
            (widget.viewModel.settings['defaultDownloadDir'] != null &&
                widget.viewModel.settings['defaultDownloadDir'] !=
                    widget.viewModel.settings['portableDownloadDir']);

        return Dialog(
          backgroundColor: AppTheme.darkSurface,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
            side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
          ),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 680),
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
                    border: Border(
                      bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5),
                    ),
                  ),
                  child: Row(
                    children: [
                      Container(
                        padding: const EdgeInsets.all(8),
                        decoration: BoxDecoration(
                          color: AppTheme.primary.withValues(alpha: 0.2),
                          borderRadius: BorderRadius.circular(8),
                        ),
                        child: const Icon(Icons.settings_outlined, color: AppTheme.primary, size: 20),
                      ),
                      const SizedBox(width: 12),
                      const Expanded(
                        child: Text(
                          'Download Settings & Storage',
                          style: TextStyle(
                            fontSize: 16,
                            fontWeight: FontWeight.bold,
                            color: Colors.white,
                          ),
                        ),
                      ),
                      IconButton(
                        icon: const Icon(Icons.close, size: 20, color: Colors.white54),
                        onPressed: () => Navigator.of(context).pop(),
                      ),
                    ],
                  ),
                ),

                // Body
                Flexible(
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.all(20),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                      // Section 1: Default Download Directory
                      const Text(
                        'Global Default Download Directory',
                        style: TextStyle(fontSize: 14, fontWeight: FontWeight.w600, color: Colors.white),
                      ),
                      const SizedBox(height: 6),
                      const Text(
                        'This path is used as the default target destination when adding new downloads. '
                        'Temporary .part files remain strictly confined in data/temp/ until completed.',
                        style: TextStyle(fontSize: 12, color: Colors.white60, height: 1.4),
                      ),
                      const SizedBox(height: 14),

                      // Input Field & Save Button
                      Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Expanded(
                            child: TextField(
                              controller: _dirController,
                              style: const TextStyle(fontSize: 13),
                              decoration: InputDecoration(
                                isDense: true,
                                labelText: 'Destination Directory Path',
                                hintText: 'e.g. C:\\Downloads or downloads',
                                filled: true,
                                fillColor: AppTheme.darkCard,
                                border: OutlineInputBorder(
                                  borderRadius: BorderRadius.circular(8),
                                  borderSide: const BorderSide(color: AppTheme.darkBorder),
                                ),
                                helperText: isCustom
                                    ? 'Using customized directory (stored in database)'
                                    : 'Using default portable directory (<bundleRoot>/downloads)',
                                helperStyle: TextStyle(
                                  color: isCustom ? Colors.tealAccent : Colors.white38,
                                  fontSize: 11,
                                ),
                              ),
                            ),
                          ),
                          const SizedBox(width: 10),
                          FilledButton(
                            style: FilledButton.styleFrom(
                              backgroundColor: AppTheme.primary,
                              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
                            ),
                            onPressed: _isSaving ? null : () => _saveDefaultDir(context),
                            child: _isSaving
                                ? const SizedBox(
                                    width: 16,
                                    height: 16,
                                    child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white),
                                  )
                                : const Text('Save'),
                          ),
                        ],
                      ),

                      const SizedBox(height: 10),

                      // Reset to Portable Default Button
                      Row(
                        children: [
                          OutlinedButton.icon(
                            icon: const Icon(Icons.restart_alt_rounded, size: 16),
                            label: const Text('Reset to Portable Default'),
                            style: OutlinedButton.styleFrom(
                              foregroundColor: Colors.white70,
                              side: const BorderSide(color: AppTheme.darkBorder),
                            ),
                            onPressed: _isSaving ? null : () => _resetDefaultDir(context),
                          ),
                          const SizedBox(width: 12),
                          TextButton.icon(
                            icon: const Icon(Icons.folder_open_rounded, size: 16),
                            label: const Text('Open Folder'),
                            onPressed: () => widget.viewModel.openDownloadsFolder(),
                          ),
                        ],
                      ),

                      if (_inlineError != null) ...[
                        const SizedBox(height: 12),
                        Container(
                          padding: const EdgeInsets.all(10),
                          decoration: BoxDecoration(
                            color: AppTheme.statusFailed.withValues(alpha: 0.15),
                            borderRadius: BorderRadius.circular(6),
                            border: Border.all(color: AppTheme.statusFailed.withValues(alpha: 0.3)),
                          ),
                          child: Row(
                            children: [
                              const Icon(Icons.error_outline, color: AppTheme.statusFailed, size: 16),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Text(
                                  _inlineError!,
                                  style: const TextStyle(fontSize: 12, color: AppTheme.statusFailed),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ],

                      if (_inlineSuccess != null) ...[
                        const SizedBox(height: 12),
                        Container(
                          padding: const EdgeInsets.all(10),
                          decoration: BoxDecoration(
                            color: Colors.teal.withValues(alpha: 0.15),
                            borderRadius: BorderRadius.circular(6),
                            border: Border.all(color: Colors.teal.withValues(alpha: 0.3)),
                          ),
                          child: Row(
                            children: [
                              const Icon(Icons.check_circle_outline, color: Colors.tealAccent, size: 16),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Text(
                                  _inlineSuccess!,
                                  style: const TextStyle(fontSize: 12, color: Colors.tealAccent),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ],

                      const SizedBox(height: 16),
                      const Divider(color: AppTheme.darkBorder),
                      const SizedBox(height: 12),

                      // Section 2: History Retention Policy
                      Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Container(
                            padding: const EdgeInsets.all(8),
                            decoration: BoxDecoration(
                              color: AppTheme.primary.withValues(alpha: 0.15),
                              borderRadius: BorderRadius.circular(6),
                            ),
                            child: const Icon(Icons.history_rounded, size: 18, color: AppTheme.primary),
                          ),
                          const SizedBox(width: 12),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                const Text(
                                  'History Retention Policy',
                                  style: TextStyle(
                                    fontSize: 14,
                                    fontWeight: FontWeight.bold,
                                    color: Colors.white,
                                  ),
                                ),
                                const SizedBox(height: 4),
                                const Text(
                                  'Historical transition events older than the retention period are automatically pruned on daemon startup.',
                                  style: TextStyle(fontSize: 12, color: Colors.white54, height: 1.3),
                                ),
                                const SizedBox(height: 12),
                                Wrap(
                                  crossAxisAlignment: WrapCrossAlignment.center,
                                  spacing: 8,
                                  runSpacing: 8,
                                  children: [
                                    const Text('Keep history for: ', style: TextStyle(fontSize: 13, color: Colors.white70)),
                                    Container(
                                      padding: const EdgeInsets.symmetric(horizontal: 10),
                                      decoration: BoxDecoration(
                                        color: AppTheme.darkCard,
                                        borderRadius: BorderRadius.circular(6),
                                        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
                                      ),
                                      child: DropdownButtonHideUnderline(
                                        child: DropdownButton<int>(
                                          value: widget.viewModel.historyRetentionDays,
                                          dropdownColor: AppTheme.darkCard,
                                          style: const TextStyle(fontSize: 13, color: Colors.white),
                                          items: const [
                                            DropdownMenuItem(value: 7, child: Text('7 Days')),
                                            DropdownMenuItem(value: 30, child: Text('30 Days (Default)')),
                                            DropdownMenuItem(value: 90, child: Text('90 Days')),
                                            DropdownMenuItem(value: 0, child: Text('Indefinitely (No pruning)')),
                                          ],
                                          onChanged: (days) {
                                            if (days != null) {
                                              widget.viewModel.setHistoryRetentionDays(days);
                                            }
                                          },
                                        ),
                                      ),
                                    ),
                                  ],
                                ),
                              ],
                            ),
                          ),
                        ],
                      ),

                      const SizedBox(height: 20),
                      const Divider(color: AppTheme.darkBorder),
                      const SizedBox(height: 12),

                      // Section 3: Network Architecture & Policy Status
                      Container(
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: AppTheme.darkCard,
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(color: AppTheme.darkBorder, width: 0.5),
                        ),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Icon(Icons.shield_outlined, color: Colors.tealAccent, size: 18),
                            const SizedBox(width: 10),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(
                                    widget.viewModel.currentNetworkProfile != NetworkProfile.localLab
                                        ? 'Mode Réseau Public Ouvert (WAN & Kad)'
                                        : 'Mode Laboratoire Confiné (127.0.0.1)',
                                    style: const TextStyle(
                                      fontSize: 12,
                                      fontWeight: FontWeight.bold,
                                      color: Colors.white,
                                    ),
                                  ),
                                  const SizedBox(height: 4),
                                  Text(
                                    widget.viewModel.currentNetworkProfile != NetworkProfile.localLab
                                        ? 'Emule-Sion est connecté aux réseaux eD2K et Kademlia mondiaux avec ports ouverts et protection Anti-SSRF active. Les transferts actifs sont sécurisés dans data/temp/.'
                                        : 'Emule-Sion fonctionne exclusivement sur la boucle locale (127.0.0.1). Aucune connexion publique entrante ou sortante n\'est effectuée.',
                                    style: const TextStyle(fontSize: 11, color: Colors.white54, height: 1.4),
                                  ),
                                ],
                              ),
                            ),
                          ],
                        ),
                      ),

                      const SizedBox(height: 16),

                      // Section 4: Network Mode & UPnP
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: const [
                                Text(
                                  'Mode Réseau & Traversée Box (UPnP)',
                                  style: TextStyle(
                                    fontSize: 13,
                                    fontWeight: FontWeight.bold,
                                    color: Colors.white,
                                  ),
                                ),
                                SizedBox(height: 4),
                                Text(
                                  'Bascule entre laboratoire local et réseau public mondial.',
                                  style: TextStyle(fontSize: 11, color: Colors.white54),
                                ),
                              ],
                            ),
                          ),
                          const SizedBox(width: 10),
                          FilledButton.icon(
                            icon: const Icon(Icons.public, size: 16),
                            label: const Text('Configurer'),
                            style: FilledButton.styleFrom(backgroundColor: Colors.amber[800]),
                            onPressed: () {
                              showDialog(
                                context: context,
                                builder: (_) => NetworkModeDialog(viewModel: widget.viewModel),
                              );
                            },
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ),

                // Footer
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
                  decoration: const BoxDecoration(
                    color: AppTheme.darkCard,
                    borderRadius: BorderRadius.vertical(bottom: Radius.circular(12)),
                    border: Border(
                      top: BorderSide(color: AppTheme.darkBorder, width: 0.5),
                    ),
                  ),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      FilledButton(
                        style: FilledButton.styleFrom(
                          backgroundColor: Colors.white12,
                          foregroundColor: Colors.white,
                        ),
                        onPressed: () => Navigator.of(context).pop(),
                        child: const Text('Close'),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }

  Future<void> _saveDefaultDir(BuildContext context) async {
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

    final success = await widget.viewModel.setDefaultDownloadDirectory(newDir);
    if (mounted) {
      setState(() {
        _isSaving = false;
        if (success) {
          _inlineSuccess = 'Default download directory updated successfully.';
          _inlineError = null;
        } else {
          _inlineError = widget.viewModel.errorMessage ?? 'Directory write probe failed.';
          _inlineSuccess = null;
        }
      });
    }
  }

  Future<void> _resetDefaultDir(BuildContext context) async {
    setState(() {
      _isSaving = true;
      _inlineError = null;
      _inlineSuccess = null;
    });

    final success = await widget.viewModel.resetDefaultDownloadDirectory();
    if (mounted) {
      setState(() {
        _isSaving = false;
        if (success) {
          _dirController.text = widget.viewModel.defaultDownloadDirectory.path;
          _inlineSuccess = 'Reset to portable default directory.';
          _inlineError = null;
        } else {
          _inlineError = widget.viewModel.errorMessage ?? 'Failed to reset download directory.';
          _inlineSuccess = null;
        }
      });
    }
  }
}
