import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../core/theme.dart';
import '../../../../services/work_directory_resolver.dart';
import '../view_models/downloads_view_model.dart';
import 'lab_guide_dialog.dart';

class DiagnosticDialog extends StatelessWidget {
  final DownloadsViewModel viewModel;

  const DiagnosticDialog({super.key, required this.viewModel});

  static String _formatWorkDirMode(WorkDirMode mode) {
    switch (mode) {
      case WorkDirMode.portable:
        return 'Portable';
      case WorkDirMode.installedDefault:
        return 'InstalledDefault';
      case WorkDirMode.environment:
        return 'Environment';
      case WorkDirMode.explicitCli:
        return 'ExplicitCli';
    }
  }

  @override
  Widget build(BuildContext context) {
    final info = viewModel.daemonInfo;
    final supervisor = viewModel.supervisor;
    final resolvedLocal = supervisor?.resolveWorkDir();

    final workdirMode = info?.workdirMode ??
        (resolvedLocal != null ? _formatWorkDirMode(resolvedLocal.mode) : 'Unknown');
    final workdirPath = info?.workdirPath ??
        resolvedLocal?.directory.path ??
        'Not available';
    final isPortable = workdirMode.toLowerCase() == 'portable';
    final isInstalled = workdirMode.toLowerCase() == 'installeddefault';

    final pidText = supervisor?.activeSession?.pid != null
        ? '${supervisor!.activeSession!.pid}'
        : (supervisor?.activePid != null
            ? '${supervisor!.activePid}'
            : (viewModel.connectionState.label));

    final rpcEndpoint = supervisor?.activeSession?.endpoint ?? 'ws://127.0.0.1:4660/rpc';
    final instanceId = (info?.instanceId.isNotEmpty == true)
        ? info!.instanceId
        : (supervisor?.activeSession?.instanceId ?? 'N/A');
    final sessionId = (info?.sessionId.isNotEmpty == true)
        ? info!.sessionId
        : 'N/A';
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
              // Header
              Row(
                children: [
                  const Icon(Icons.analytics_outlined, color: AppTheme.primary, size: 24),
                  const SizedBox(width: 10),
                  const Expanded(
                    child: Text(
                      'System & Lab Diagnostics',
                      style: TextStyle(
                        fontSize: 18,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.close, size: 20, color: Colors.white54),
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
                      // Network Isolation Card (Key Guarantee)
                      Container(
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: const Color(0x2210B981),
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(color: const Color(0x6610B981), width: 0.5),
                        ),
                        child: const Row(
                          children: [
                            Icon(Icons.shield_outlined, color: Color(0xFF10B981), size: 22),
                            SizedBox(width: 10),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(
                                    'Strict Loopback Confinement (127.0.0.1)',
                                    style: TextStyle(
                                      color: Color(0xFF10B981),
                                      fontWeight: FontWeight.bold,
                                      fontSize: 13,
                                    ),
                                  ),
                                  SizedBox(height: 2),
                                  Text(
                                    'Zero WAN traffic. No public eD2K servers, Kad nodes, or external peers.',
                                    style: TextStyle(color: Colors.white70, fontSize: 12),
                                  ),
                                ],
                              ),
                            ),
                          ],
                        ),
                      ),

                      const SizedBox(height: 16),

                      // Workdir Section
                      _buildSectionTitle('Working Directory & Mode'),
                      const SizedBox(height: 8),
                      Container(
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: AppTheme.darkCard,
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(color: AppTheme.darkBorder, width: 0.5),
                        ),
                        child: Column(
                          children: [
                            _buildInfoRow(
                              'Mode',
                              Row(
                                children: [
                                  Container(
                                    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                                    decoration: BoxDecoration(
                                      color: isPortable
                                          ? AppTheme.primary.withValues(alpha: 0.2)
                                          : (isInstalled
                                              ? Colors.amber.withValues(alpha: 0.2)
                                              : Colors.blueGrey.withValues(alpha: 0.2)),
                                      borderRadius: BorderRadius.circular(4),
                                      border: Border.all(
                                        color: isPortable
                                            ? AppTheme.primary
                                            : (isInstalled ? Colors.amber : Colors.blueGrey),
                                        width: 0.5,
                                      ),
                                    ),
                                    child: Text(
                                      workdirMode,
                                      style: TextStyle(
                                        color: isPortable
                                            ? AppTheme.primary
                                            : (isInstalled ? Colors.amberAccent : Colors.white70),
                                        fontWeight: FontWeight.bold,
                                        fontSize: 11,
                                      ),
                                    ),
                                  ),
                                  if (isPortable) ...[
                                    const SizedBox(width: 8),
                                    const Text(
                                      '(Isolated data folder)',
                                      style: TextStyle(color: Colors.white54, fontSize: 11),
                                    ),
                                  ],
                                ],
                              ),
                            ),
                            const Divider(height: 16, color: AppTheme.darkBorder),
                            _buildInfoRow(
                              'Path',
                              SelectableText(
                                workdirPath,
                                style: const TextStyle(
                                  fontSize: 12,
                                  fontFamily: 'monospace',
                                  color: Colors.white,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),

                      const SizedBox(height: 16),

                      // Daemon & Process Section
                      _buildSectionTitle('Daemon & Process'),
                      const SizedBox(height: 8),
                      Container(
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: AppTheme.darkCard,
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(color: AppTheme.darkBorder, width: 0.5),
                        ),
                        child: Column(
                          children: [
                            _buildInfoRow(
                              'Process PID',
                              Text(
                                pidText,
                                style: const TextStyle(fontSize: 12, fontFamily: 'monospace'),
                              ),
                            ),
                            const Divider(height: 16, color: AppTheme.darkBorder),
                            _buildInfoRow(
                              'RPC Endpoint',
                              SelectableText(
                                rpcEndpoint,
                                style: const TextStyle(fontSize: 12, fontFamily: 'monospace'),
                              ),
                            ),
                            const Divider(height: 16, color: AppTheme.darkBorder),
                            _buildInfoRow(
                              'Daemon Version',
                              Text(
                                daemonVersion,
                                style: const TextStyle(fontSize: 12),
                              ),
                            ),
                            const Divider(height: 16, color: AppTheme.darkBorder),
                            _buildInfoRow(
                              'Instance ID',
                              SelectableText(
                                instanceId,
                                style: const TextStyle(fontSize: 11, fontFamily: 'monospace', color: Colors.white70),
                              ),
                            ),
                            const Divider(height: 16, color: AppTheme.darkBorder),
                            _buildInfoRow(
                              'Session ID',
                              SelectableText(
                                sessionId,
                                style: const TextStyle(fontSize: 11, fontFamily: 'monospace', color: Colors.white70),
                              ),
                            ),
                            const Divider(height: 16, color: AppTheme.darkBorder),
                            _buildInfoRow(
                              'Capabilities',
                              Wrap(
                                spacing: 4,
                                runSpacing: 4,
                                children: capabilities.isEmpty
                                    ? [
                                        const Text('None', style: TextStyle(fontSize: 12, color: Colors.white54)),
                                      ]
                                    : capabilities.map((c) {
                                        final isLab = c == 'lab-tools';
                                        return Container(
                                          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                                          decoration: BoxDecoration(
                                            color: isLab ? Colors.teal.withValues(alpha: 0.2) : Colors.white10,
                                            borderRadius: BorderRadius.circular(4),
                                            border: Border.all(
                                              color: isLab ? Colors.teal : Colors.white24,
                                              width: 0.5,
                                            ),
                                          ),
                                          child: Text(
                                            c,
                                            style: TextStyle(
                                              fontSize: 10,
                                              fontFamily: 'monospace',
                                              color: isLab ? Colors.tealAccent : Colors.white70,
                                              fontWeight: isLab ? FontWeight.bold : FontWeight.normal,
                                            ),
                                          ),
                                        );
                                      }).toList(),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),

              const SizedBox(height: 16),

              // Footer
              Align(
                alignment: Alignment.centerRight,
                child: Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  alignment: WrapAlignment.end,
                  children: [
                    OutlinedButton.icon(
                      icon: const Icon(Icons.share_outlined, size: 16),
                      label: const Text('Copy Shareable (Sanitized)'),
                      onPressed: () {
                        final sanitized = _formatShareableDiagnostics(
                          daemonVersion: daemonVersion,
                          protocolVersion: protocolVersionStr,
                          instanceId: instanceId,
                          sessionId: sessionId,
                          workdirMode: workdirMode,
                          isPortable: isPortable,
                          isInstalled: isInstalled,
                          countAll: viewModel.countAll,
                        );
                        Clipboard.setData(ClipboardData(text: sanitized));
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text('Sanitized shareable diagnostics copied'),
                            duration: Duration(seconds: 2),
                          ),
                        );
                      },
                    ),
                    OutlinedButton.icon(
                      icon: const Icon(Icons.code_rounded, size: 16),
                      label: const Text('Copy Full (Local Dev)'),
                      onPressed: () {
                        final full = _formatFullDiagnostics(
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
                        );
                        Clipboard.setData(ClipboardData(text: full));
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text('Full diagnostics copied to clipboard'),
                            duration: Duration(seconds: 2),
                          ),
                        );
                      },
                    ),
                    if (viewModel.isLabModeAvailable)
                      FilledButton.tonalIcon(
                        icon: const Icon(Icons.science, size: 16),
                        label: const Text('Open Lab Guide'),
                        style: FilledButton.styleFrom(
                          backgroundColor: Colors.teal.withValues(alpha: 0.2),
                          foregroundColor: Colors.tealAccent,
                        ),
                        onPressed: () {
                          Navigator.of(context).pop();
                          showDialog(
                            context: context,
                            builder: (ctx) => LabGuideDialog(viewModel: viewModel),
                          );
                        },
                      ),
                    FilledButton(
                      onPressed: () => Navigator.of(context).pop(),
                      child: const Text('Close'),
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

  Widget _buildSectionTitle(String title) {
    return Text(
      title,
      style: const TextStyle(
        fontSize: 13,
        fontWeight: FontWeight.w600,
        color: Colors.white70,
        letterSpacing: 0.5,
      ),
    );
  }

  Widget _buildInfoRow(String label, Widget content) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: 120,
          child: Text(
            label,
            style: const TextStyle(
              fontSize: 12,
              color: Colors.white54,
              fontWeight: FontWeight.w500,
            ),
          ),
        ),
        Expanded(child: content),
      ],
    );
  }

  String _formatShareableDiagnostics({
    required String daemonVersion,
    required String protocolVersion,
    required String instanceId,
    required String sessionId,
    required String workdirMode,
    required bool isPortable,
    required bool isInstalled,
    required int countAll,
  }) {
    String maskId(String? id) {
      if (id == null || id.isEmpty || id == 'N/A') return 'N/A';
      if (id.length <= 8) return id;
      return '${id.substring(0, 4)}…${id.substring(id.length - 4)}';
    }

    final maskedWorkdir = isPortable
        ? '<portable-data>'
        : (isInstalled ? '<installed-data>' : '<custom-data>');

    final sb = StringBuffer();
    sb.writeln('=== Emule-Sion Diagnostics (Sanitized) ===');
    sb.writeln('Workdir Mode: $workdirMode');
    sb.writeln('Workdir Path: $maskedWorkdir');
    sb.writeln('Confinement: 127.0.0.1 (Loopback only)');
    sb.writeln('Process PID: [local-process]');
    sb.writeln('RPC Endpoint: ws://loopback:4660/rpc');
    sb.writeln('Daemon Version: $daemonVersion');
    sb.writeln('Protocol Version: $protocolVersion');
    sb.writeln('Instance ID: ${maskId(instanceId)}');
    sb.writeln('Session ID: ${maskId(sessionId)}');
    sb.writeln('Active Downloads: $countAll');
    return sb.toString();
  }

  String _formatFullDiagnostics({
    required String daemonVersion,
    required String protocolVersion,
    required String instanceId,
    required String sessionId,
    required List<String> capabilities,
    required String workdirMode,
    required String workdirPath,
    required String pidText,
    required String rpcEndpoint,
    required int countAll,
  }) {
    final sb = StringBuffer();
    sb.writeln('=== Emule-Sion Diagnostics (Full Local) ===');
    sb.writeln('Workdir Mode: $workdirMode');
    sb.writeln('Workdir Path: $workdirPath');
    sb.writeln('Confinement: 127.0.0.1 (Loopback only)');
    sb.writeln('Process PID: $pidText');
    sb.writeln('RPC Endpoint: $rpcEndpoint');
    sb.writeln('Daemon Version: $daemonVersion');
    sb.writeln('Protocol Version: $protocolVersion');
    sb.writeln('Capabilities: ${capabilities.isNotEmpty ? capabilities.join(', ') : 'N/A'}');
    sb.writeln('Instance ID: $instanceId');
    sb.writeln('Session ID: $sessionId');
    sb.writeln('Active Downloads: $countAll');
    sb.writeln('Security: Auth token strictly redacted (never exported)');
    return sb.toString();
  }
}

