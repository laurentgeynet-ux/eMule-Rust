import 'package:flutter/material.dart';

import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import '../../../../core/system_actions.dart';
import '../../settings/widgets/network_mode_dialog.dart';
import 'settings/settings_dialog_footer.dart';
import 'settings/settings_dialog_header.dart';
import 'settings/settings_directory_section.dart';
import 'settings/settings_inline_message.dart';
import 'settings/settings_language_section.dart';
import 'settings/settings_network_card.dart';
import 'settings/settings_network_mode_row.dart';
import 'settings/settings_friends_section.dart';
import 'settings/settings_retention_section.dart';

/// Download settings: default directory, history retention, network
/// profile summary and network mode shortcut.
class SettingsDialog extends ConsumerStatefulWidget {
    const SettingsDialog({super.key, });

  @override
  ConsumerState<SettingsDialog> createState() => _SettingsDialogState();
}

class _SettingsDialogState extends ConsumerState<SettingsDialog> {
  late TextEditingController _dirController;
  bool _isSaving = false;
  String? _inlineError;
  String? _inlineSuccess;

  @override
  void initState() {
    super.initState();
    final currentDir = ref.read(defaultDownloadDirectoryProvider);
    _dirController = TextEditingController(text: currentDir);
    Future.microtask(() => ref.read(settingsControllerProvider).fetch());
  }

  @override
  void dispose() {
    _dirController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(settingsControllerProvider).settings;
    final isCustom = settings['isCustom'] == true ||
        (settings['defaultDownloadDir'] != null &&
            settings['defaultDownloadDir'] != settings['portableDownloadDir']);

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
                              () => systemActions.openDirectory(ref.read(defaultDownloadDirectoryProvider)),
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
                              ref.watch(historyControllerProvider).retentionDays,
                          onChanged:
                              ref.read(historyControllerProvider).setRetentionDays,
                        ),
                        const SizedBox(height: 20),
                        const Divider(color: AppTheme.darkBorder),
                        const SizedBox(height: 12),
                        const SettingsLanguageSection(),
                        const SizedBox(height: 20),
                        const Divider(color: AppTheme.darkBorder),
                        const SizedBox(height: 12),
                        SettingsNetworkCard(
                          profile:
                              ref.watch(networkPolicyControllerProvider).currentProfile,
                        ),
                        const SizedBox(height: 16),
                        SettingsNetworkModeRow(
                          onConfigure: () => showDialog(
                            context: context,
                            builder: (_) => const NetworkModeDialog(),
                          ),
                        ),
                        const SizedBox(height: 20),
                        const Divider(color: AppTheme.darkBorder),
                        const SizedBox(height: 12),
                        const SettingsFriendsSection(),
                      ],
                    ),
                  ),
                ),
                const SettingsDialogFooter(),
              ],
            ),
          ),
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
        await ref.read(settingsControllerProvider).setDefaultDownloadDirectory(newDir);
    if (mounted) {
      setState(() {
        _isSaving = false;
        if (success) {
          _inlineSuccess =
              'Default download directory updated successfully.';
          _inlineError = null;
        } else {
          _inlineError = ref.read(shellControllerProvider).errorMessage ??
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
        await ref.read(settingsControllerProvider).resetDefaultDownloadDirectory();
    if (mounted) {
      setState(() {
        _isSaving = false;
        if (success) {
          _dirController.text =
              ref.read(defaultDownloadDirectoryProvider);
          _inlineSuccess = 'Reset to portable default directory.';
          _inlineError = null;
        } else {
          _inlineError = ref.read(shellControllerProvider).errorMessage ??
              'Failed to reset download directory.';
          _inlineSuccess = null;
        }
      });
    }
  }
}
