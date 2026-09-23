import 'package:mule_client/mule_client.dart';

import '../../../../../core/work_dirs.dart';

/// Display label for a [WorkDirMode].
String formatWorkDirMode(WorkDirMode mode) {
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

/// Obfuscates user profile segments and truncates long paths so a path can
/// be shown in the UI without exposing the full local layout.
String formatPathForDisplay(String path) {
  if (path == 'Not available' || path.isEmpty) return path;

  // Obfuscate Windows user profile: C:\Users\<username>\... -> C:\Users\…\...
  final winUserRegex =
      RegExp(r'^([a-zA-Z]:\\Users)\\[^\\]+\\(.*)$', caseSensitive: false);
  final winMatch = winUserRegex.firstMatch(path);
  if (winMatch != null) {
    final prefix = winMatch.group(1)!;
    final rest = winMatch.group(2)!;
    final parts = rest.split('\\');
    if (parts.length > 2) {
      return '$prefix\\…\\${parts.first}\\…\\${parts.last}';
    } else if (parts.isNotEmpty) {
      return '$prefix\\…\\$rest';
    }
    return '$prefix\\…';
  }

  // Obfuscate Unix user profile: /home/<username>/... -> /home/…/...
  final unixUserRegex = RegExp(r'^(/home)/[^/]+/(.*)$');
  final unixMatch = unixUserRegex.firstMatch(path);
  if (unixMatch != null) {
    final prefix = unixMatch.group(1)!;
    final rest = unixMatch.group(2)!;
    final parts = rest.split('/');
    if (parts.length > 2) {
      return '$prefix/…/${parts.first}/…/${parts.last}';
    }
    return '$prefix/…/$rest';
  }

  // Truncate generic long paths
  if (path.length > 45) {
    final sep = path.contains('/') ? '/' : '\\';
    final parts = path.split(sep);
    if (parts.length > 3) {
      return '${parts.first}$sep…$sep${parts[parts.length - 2]}$sep${parts.last}';
    }
  }

  return path;
}

/// Sanitized one-shot diagnostic report: masked IDs, no real paths, no PID.
String formatShareableDiagnostics({
  required String daemonVersion,
  required String protocolVersion,
  required String instanceId,
  required String sessionId,
  required String workdirMode,
  required bool isPortable,
  required bool isInstalled,
  required int countAll,
  required NetworkProfile networkProfile,
}) {
  String maskId(String? id) {
    if (id == null || id.isEmpty || id == 'N/A') return 'N/A';
    if (id.length <= 8) return id;
    return '${id.substring(0, 4)}…${id.substring(id.length - 4)}';
  }

  final maskedWorkdir = isPortable
      ? '<portable-data>'
      : (isInstalled ? '<installed-data>' : '<custom-data>');

  final confinement = networkProfile == NetworkProfile.localLab
      ? '127.0.0.1 (Loopback only)'
      : 'Public (${networkProfile.toKebabCase()})';

  final sb = StringBuffer();
  sb.writeln('=== EMule-Rust Diagnostics (Sanitized) ===');
  sb.writeln('Workdir Mode: $workdirMode');
  sb.writeln('Workdir Path: $maskedWorkdir');
  sb.writeln('Confinement: $confinement');
  sb.writeln('Process PID: [local-process]');
  sb.writeln('RPC Endpoint: ws://loopback:4660/rpc');
  sb.writeln('Daemon Version: $daemonVersion');
  sb.writeln('Protocol Version: $protocolVersion');
  sb.writeln('Instance ID: ${maskId(instanceId)}');
  sb.writeln('Session ID: ${maskId(sessionId)}');
  sb.writeln('Active Downloads: $countAll');
  return sb.toString();
}

/// Full local diagnostic report — for developer use only, never shared.
String formatFullDiagnostics({
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
  required NetworkProfile networkProfile,
}) {
  final confinement = networkProfile == NetworkProfile.localLab
      ? '127.0.0.1 (Loopback only)'
      : 'Public (${networkProfile.toKebabCase()})';

  final sb = StringBuffer();
  sb.writeln('=== EMule-Rust Diagnostics (Full Local) ===');
  sb.writeln('Workdir Mode: $workdirMode');
  sb.writeln('Workdir Path: $workdirPath');
  sb.writeln('Confinement: $confinement');
  sb.writeln('Process PID: $pidText');
  sb.writeln('RPC Endpoint: $rpcEndpoint');
  sb.writeln('Daemon Version: $daemonVersion');
  sb.writeln('Protocol Version: $protocolVersion');
  sb.writeln(
      'Capabilities: ${capabilities.isNotEmpty ? capabilities.join(', ') : 'N/A'}');
  sb.writeln('Instance ID: $instanceId');
  sb.writeln('Session ID: $sessionId');
  sb.writeln('Active Downloads: $countAll');
  sb.writeln('Security: Auth token strictly redacted (never exported)');
  return sb.toString();
}
