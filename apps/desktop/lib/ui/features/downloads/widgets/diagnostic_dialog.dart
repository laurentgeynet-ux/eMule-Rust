import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:ui_kit/ui_kit.dart';
import '../../../../app/app_controller.dart';
import 'diagnostic/capability_chip.dart';
import 'diagnostic/diagnostic_card.dart';
import 'diagnostic/diagnostic_copy_row.dart';
import 'diagnostic/diagnostic_format.dart';
import 'diagnostic/diagnostic_info_row.dart';
import 'diagnostic/diagnostic_row_divider.dart';
import 'diagnostic/diagnostic_section_title.dart';
import 'diagnostic/network_profile_banner.dart';
import 'lab_guide_dialog.dart';
import '../../../../l10n/l10n.dart';

/// System & lab diagnostics: workdir mode/paths, daemon process info,
/// capabilities, and sanitized/full copy actions.
class DiagnosticDialog extends StatelessWidget {
  final AppController viewModel;

  const DiagnosticDialog({super.key, required this.viewModel});

  @override
  Widget build(BuildContext context) {
    final info = viewModel.daemonInfo;
    final supervisor = viewModel.supervisor;
    final resolvedLocal = supervisor?.resolveWorkDir();

    final workdirMode = info?.workdirMode ??
        (resolvedLocal != null
            ? formatWorkDirMode(resolvedLocal.mode)
            : 'Unknown');
    final workdirPath = info?.workdirPath ??
        resolvedLocal?.directory.path ??
        'Not available';
    final downloadsPath = info?.defaultDownloadDir ??
        resolvedLocal?.defaultDownloadDirectory.path ??
        viewModel.defaultDownloadDirectory.path;
    final isPortable = workdirMode.toLowerCase() == 'portable';
    final isInstalled = workdirMode.toLowerCase() == 'installeddefault';

    final pidText = supervisor?.activeSession?.pid != null
        ? '${supervisor!.activeSession!.pid}'
        : (supervisor?.activePid != null
            ? '${supervisor!.activePid}'
            : (viewModel.connectionState.label));

    final rpcEndpoint = supervisor?.activeSession?.endpoint ??
        'ws://127.0.0.1:4660/rpc';
    final instanceId = (info?.instanceId.isNotEmpty == true)
        ? info!.instanceId
        : (supervisor?.activeSession?.instanceId ?? 'N/A');
    final sessionId =
        (info?.sessionId.isNotEmpty == true) ? info!.sessionId : 'N/A';
    final daemonVersion = info != null
        ? 'v${info.version} (proto v${info.protocolVersion})'
        : (supervisor?.activeSession?.protocolVersion != null
            ? 'proto v${supervisor!.activeSession!.protocolVersion}'
            : 'N/A');
    final protocolVersionStr = info != null
        ? '${info.protocolVersion}'
        : (supervisor?.activeSession != null
            ? '${supervisor!.activeSession!.protocolVersion}'
            : '1');
    final capabilities = info?.capabilities ?? const <String>[];

    return Dialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 580, maxHeight: 680),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const Icon(
                    Icons.analytics_outlined,
                    color: AppTheme.primary,
                    size: 24,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(context.l10n.systemLabDiagnostics,
                      style: TextStyle(
                        fontSize: 18,
                        fontWeight: FontWeight.bold,
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
              const SizedBox(height: 16),
              Expanded(
                child: SingleChildScrollView(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      NetworkProfileBanner(
                        profile: viewModel.currentNetworkProfile,
                      ),
                      const SizedBox(height: 16),
                      DiagnosticSectionTitle(
                        title: context.l10n.workingDirectoryMode,
                      ),
                      const SizedBox(height: 8),
                      DiagnosticCard(
                        children: [
                          DiagnosticInfoRow(
                            label: context.l10n.mode,
                            content: Row(
                              children: [
                                _WorkdirModeBadge(
                                  mode: workdirMode,
                                  isPortable: isPortable,
                                  isInstalled: isInstalled,
                                ),
                                if (isPortable) ...[
                                  const SizedBox(width: 8),
                                  Text(context.l10n.isolatedDataFolder,
                                    style: TextStyle(
                                      color: AppColors.textMuted,
                                      fontSize: 11,
                                    ),
                                  ),
                                ],
                              ],
                            ),
                          ),
                          const DiagnosticRowDivider(),
                          DiagnosticCopyRow(
                            label: context.l10n.internalData,
                            path: workdirPath,
                            copyTooltip: 'Copy full path',
                            copiedMessage:
                                'Internal data path copied to clipboard',
                          ),
                          const DiagnosticRowDivider(),
                          DiagnosticCopyRow(
                            label: context.l10n.downloads,
                            path: downloadsPath,
                            copyTooltip:
                                'Copy downloads directory path',
                            copiedMessage:
                                'Downloads path copied to clipboard',
                          ),
                        ],
                      ),
                      const SizedBox(height: 16),
                      DiagnosticSectionTitle(
                        title: context.l10n.daemonProcess,
                      ),
                      const SizedBox(height: 8),
                      DiagnosticCard(
                        children: [
                          DiagnosticInfoRow(
                            label: context.l10n.processPid,
                            content: Text(
                              pidText,
                              style: const TextStyle(
                                fontSize: 12,
                                fontFamily: 'monospace',
                              ),
                            ),
                          ),
                          const DiagnosticRowDivider(),
                          DiagnosticInfoRow(
                            label: context.l10n.rpcEndpoint,
                            content: SelectableText(
                              rpcEndpoint,
                              style: const TextStyle(
                                fontSize: 12,
                                fontFamily: 'monospace',
                              ),
                            ),
                          ),
                          const DiagnosticRowDivider(),
                          DiagnosticInfoRow(
                            label: context.l10n.daemonVersion,
                            content: Text(
                              daemonVersion,
                              style: const TextStyle(fontSize: 12),
                            ),
                          ),
                          const DiagnosticRowDivider(),
                          DiagnosticInfoRow(
                            label: context.l10n.instanceId,
                            content: SelectableText(
                              instanceId,
                              style: const TextStyle(
                                fontSize: 11,
                                fontFamily: 'monospace',
                                color: AppColors.textSecondary,
                              ),
                            ),
                          ),
                          const DiagnosticRowDivider(),
                          DiagnosticInfoRow(
                            label: context.l10n.sessionId,
                            content: SelectableText(
                              sessionId,
                              style: const TextStyle(
                                fontSize: 11,
                                fontFamily: 'monospace',
                                color: AppColors.textSecondary,
                              ),
                            ),
                          ),
                          const DiagnosticRowDivider(),
                          DiagnosticCopyRow(
                            label: context.l10n.daemonLog,
                            path: '$workdirPath\\daemon.log',
                            copyTooltip: 'Copy daemon log path',
                            copiedMessage:
                                'Daemon log path copied to clipboard',
                          ),
                          const DiagnosticRowDivider(),
                          DiagnosticInfoRow(
                            label: context.l10n.capabilities,
                            content: Wrap(
                              spacing: 4,
                              runSpacing: 4,
                              children: capabilities.isEmpty
                                  ? [
                                      Text(context.l10n.none,
                                        style: TextStyle(
                                          fontSize: 12,
                                          color: AppColors.textMuted,
                                        ),
                                      ),
                                    ]
                                  : capabilities
                                      .map((c) =>
                                          CapabilityChip(capability: c))
                                      .toList(),
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ),
              const SizedBox(height: 16),
              Align(
                alignment: Alignment.centerRight,
                child: Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  alignment: WrapAlignment.end,
                  children: [
                    OutlinedButton.icon(
                      icon: const Icon(Icons.share_outlined, size: 16),
                      label: Text(context.l10n.copyShareableSanitized),
                      onPressed: () {
                        final sanitized = formatShareableDiagnostics(
                          daemonVersion: daemonVersion,
                          protocolVersion: protocolVersionStr,
                          instanceId: instanceId,
                          sessionId: sessionId,
                          workdirMode: workdirMode,
                          isPortable: isPortable,
                          isInstalled: isInstalled,
                          countAll: viewModel.countAll,
                          networkProfile:
                              viewModel.currentNetworkProfile,
                        );
                        Clipboard.setData(
                          ClipboardData(text: sanitized),
                        );
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(
                            content: Text(context.l10n.sanitizedShareableDiagnosticsCopied,
                            ),
                            duration: Duration(seconds: 2),
                          ),
                        );
                      },
                    ),
                    OutlinedButton.icon(
                      icon: const Icon(Icons.code_rounded, size: 16),
                      label: Text(context.l10n.copyFullLocalDev),
                      onPressed: () {
                        final full = formatFullDiagnostics(
                          daemonVersion: daemonVersion,
                          protocolVersion: protocolVersionStr,
                          instanceId: instanceId,
                          sessionId: sessionId,
                          capabilities: capabilities,
                          workdirMode: workdirMode,
                          workdirPath: workdirPath,
                          pidText: pidText,
                          rpcEndpoint: rpcEndpoint,
                          countAll: viewModel.countAll,
                          networkProfile:
                              viewModel.currentNetworkProfile,
                        );
                        Clipboard.setData(ClipboardData(text: full));
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(
                            content: Text(context.l10n.fullDiagnosticsCopiedToClipboard,
                            ),
                            duration: Duration(seconds: 2),
                          ),
                        );
                      },
                    ),
                    if (viewModel.isLabModeAvailable)
                      FilledButton.tonalIcon(
                        icon: const Icon(Icons.science, size: 16),
                        label: Text(context.l10n.openLabGuide),
                        style: FilledButton.styleFrom(
                          backgroundColor:
                              AppColors.lab.withValues(alpha: 0.2),
                          foregroundColor: AppColors.accentTeal,
                        ),
                        onPressed: () {
                          Navigator.of(context).pop();
                          showDialog(
                            context: context,
                            builder: (ctx) =>
                                LabGuideDialog(viewModel: viewModel),
                          );
                        },
                      ),
                    FilledButton(
                      onPressed: () => Navigator.of(context).pop(),
                      child: Text(context.l10n.close),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _WorkdirModeBadge extends StatelessWidget {
  final String mode;
  final bool isPortable;
  final bool isInstalled;

  const _WorkdirModeBadge({
    required this.mode,
    required this.isPortable,
    required this.isInstalled,
  });

  @override
  Widget build(BuildContext context) {
    final bg = isPortable
        ? AppTheme.primary.withValues(alpha: 0.2)
        : (isInstalled
            ? AppColors.warning.withValues(alpha: 0.2)
            : AppColors.labDim.withValues(alpha: 0.2));
    final border = isPortable
        ? AppTheme.primary
        : (isInstalled ? AppColors.warning : AppColors.labDim);
    final fg = isPortable
        ? AppTheme.primary
        : (isInstalled ? AppColors.accentAmber : AppColors.textSecondary);

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(4),
        border: Border.all(color: border, width: 0.5),
      ),
      child: Text(
        mode,
        style: TextStyle(
          color: fg,
          fontWeight: FontWeight.bold,
          fontSize: 11,
        ),
      ),
    );
  }
}
