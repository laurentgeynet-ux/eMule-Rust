import 'package:flutter/material.dart';

import 'package:ui_kit/ui_kit.dart';
import '../view_models/downloads_view_model.dart';
import '../../settings/widgets/network_mode_dialog.dart';
import 'settings/settings_dialog_footer.dart';
import 'settings/settings_dialog_header.dart';
import 'settings/settings_directory_section.dart';
import 'settings/settings_inline_message.dart';
import 'settings/settings_network_card.dart';
import 'settings/settings_network_mode_row.dart';
import 'settings/settings_retention_section.dart';

/// Download settings: default directory, history retention, network
/// profile summary and network mode shortcut.
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
            side:
                const BorderSide(color: AppTheme.darkBorder, width: 0.5),
          ),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 680),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                const SettingsDialogHeader(),
                Flexible(
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.all(20),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        SettingsDirectorySection(
                          dirController: _dirController,
                          isCustom: isCustom,
                          isSaving: _isSaving,
                          onSave: () => _saveDefaultDir(context),
                          onReset: () => _resetDefaultDir(context),
                          onOpenFolder:
                              widget.viewModel.openDownloadsFolder,
                        ),
                        if (_inlineError != null)
                          SettingsInlineMessage(
                            message: _inlineError!,
                            isError: true,
                          ),
                        if (_inlineSuccess != null)
                          SettingsInlineMessage(
                            message: _inlineSuccess!,
                            isError: false,
                          ),
                        const SizedBox(height: 16),
                        const Divider(color: AppTheme.darkBorder),
                        const SizedBox(height: 12),
                        SettingsRetentionSection(
                          retentionDays:
                              widget.viewModel.historyRetentionDays,
                          onChanged:
                              widget.viewModel.setHistoryRetentionDays,
                        ),
                        const SizedBox(height: 20),
                        const Divider(color: AppTheme.darkBorder),
                        const SizedBox(height: 12),
                        SettingsNetworkCard(
                          profile:
                              widget.viewModel.currentNetworkProfile,
                        ),
                        const SizedBox(height: 16),
                        SettingsNetworkModeRow(
                          onConfigure: () => showDialog(
                            context: context,
                            builder: (_) => NetworkModeDialog(
                              viewModel: widget.viewModel,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
                const SettingsDialogFooter(),
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

    final success =
        await widget.viewModel.setDefaultDownloadDirectory(newDir);
    if (mounted) {
      setState(() {
        _isSaving = false;
        if (success) {
          _inlineSuccess =
              'Default download directory updated successfully.';
          _inlineError = null;
        } else {
          _inlineError = widget.viewModel.errorMessage ??
              'Directory write probe failed.';
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

    final success =
        await widget.viewModel.resetDefaultDownloadDirectory();
    if (mounted) {
      setState(() {
        _isSaving = false;
        if (success) {
          _dirController.text =
              widget.viewModel.defaultDownloadDirectory.path;
          _inlineSuccess = 'Reset to portable default directory.';
          _inlineError = null;
        } else {
          _inlineError = widget.viewModel.errorMessage ??
              'Failed to reset download directory.';
          _inlineSuccess = null;
        }
      });
    }
  }
}
