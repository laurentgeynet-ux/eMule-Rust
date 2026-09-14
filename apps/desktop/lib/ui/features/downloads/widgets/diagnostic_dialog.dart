import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../core/theme.dart';
import '../view_models/downloads_view_model.dart';
import 'lab_guide_dialog.dart';

class DiagnosticDialog extends StatelessWidget {
  final DownloadsViewModel viewModel;

  const DiagnosticDialog({super.key, required this.viewModel});

  @override
  Widget build(BuildContext context) {
    final info = viewModel.daemonInfo;
    final supervisor = viewModel.supervisor;

    final workdirMode = info?.workdirMode ?? 'Unknown';
    final workdirPath = info?.workdirPath ?? 'Not available';
    final isPortable = workdirMode == 'Portable';
    final isInstalled = workdirMode == 'InstalledDefault';

    final pidText = supervisor?.activePid != null
        ? '${supervisor!.activePid}'
        : (viewModel.connectionState.label);

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
                              const SelectableText(
                                'ws://127.0.0.1:4660/rpc',
                                style: TextStyle(fontSize: 12, fontFamily: 'monospace'),
                              ),
                            ),
                            const Divider(height: 16, color: AppTheme.darkBorder),
                            _buildInfoRow(
                              'Daemon Version',
                              Text(
                                info != null ? 'v${info.version} (proto v${info.protocolVersion})' : 'N/A',
                                style: const TextStyle(fontSize: 12),
                              ),
                            ),
                            const Divider(height: 16, color: AppTheme.darkBorder),
                            _buildInfoRow(
                              'Instance ID',
                              SelectableText(
                                info?.instanceId ?? 'N/A',
                                style: const TextStyle(fontSize: 11, fontFamily: 'monospace', color: Colors.white70),
                              ),
                            ),
                            const Divider(height: 16, color: AppTheme.darkBorder),
                            _buildInfoRow(
                              'Session ID',
                              SelectableText(
                                info?.sessionId ?? 'N/A',
                                style: const TextStyle(fontSize: 11, fontFamily: 'monospace', color: Colors.white70),
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
                          info,
                          workdirMode,
                          isPortable,
                          isInstalled,
                          viewModel.countAll,
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
                          info,
                          workdirMode,
                          workdirPath,
                          pidText,
                          viewModel.countAll,
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

  String _formatShareableDiagnostics(
    dynamic info,
    String workdirMode,
    bool isPortable,
    bool isInstalled,
    int countAll,
  ) {
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
    sb.writeln('Daemon Version: ${info?.version ?? 'N/A'}');
    sb.writeln('Protocol Version: ${info?.protocolVersion ?? 'N/A'}');
    sb.writeln('Instance ID: ${maskId(info?.instanceId)}');
    sb.writeln('Session ID: ${maskId(info?.sessionId)}');
    sb.writeln('Active Downloads: $countAll');
    return sb.toString();
  }

  String _formatFullDiagnostics(
    dynamic info,
    String workdirMode,
    String workdirPath,
    String pidText,
    int countAll,
  ) {
    final sb = StringBuffer();
    sb.writeln('=== Emule-Sion Diagnostics (Full Local) ===');
    sb.writeln('Workdir Mode: $workdirMode');
    sb.writeln('Workdir Path: $workdirPath');
    sb.writeln('Confinement: 127.0.0.1 (Loopback only)');
    sb.writeln('Process PID: $pidText');
    sb.writeln('RPC Endpoint: ws://127.0.0.1:4660/rpc');
    sb.writeln('Daemon Version: ${info?.version ?? 'N/A'}');
    sb.writeln('Protocol Version: ${info?.protocolVersion ?? 'N/A'}');
    sb.writeln('Capabilities: ${(info?.capabilities as List?)?.join(', ') ?? 'N/A'}');
    sb.writeln('Instance ID: ${info?.instanceId ?? 'N/A'}');
    sb.writeln('Session ID: ${info?.sessionId ?? 'N/A'}');
    sb.writeln('Active Downloads: $countAll');
    sb.writeln('Security: Auth token strictly redacted (never exported)');
    return sb.toString();
  }
}

