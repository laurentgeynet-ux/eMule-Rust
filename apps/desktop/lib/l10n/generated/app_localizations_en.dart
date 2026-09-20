// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appName => 'Emule-Sion';

  @override
  String get appTagline => 'Modern eD2K Client';

  @override
  String get loopbackLabMode => 'Loopback Lab Mode';

  @override
  String get tooltipLoopbackLabMode =>
      'Laboratory mode.\nConnections are restricted to 127.0.0.1.\n\nNo public eD2K servers, Kad nodes,\nor external peers are contacted.';

  @override
  String get devModeBadge => 'DEV';

  @override
  String get sectionDownloads => 'DOWNLOADS';

  @override
  String get sectionCategories => 'CATEGORIES';

  @override
  String get sectionAuditActivity => 'AUDIT & ACTIVITY';

  @override
  String get navHistoryStats => 'History & Statistics';

  @override
  String get navServers => 'eD2K Servers';

  @override
  String get navSearch => 'eD2K Search';

  @override
  String get navKad => 'Kad DHT';

  @override
  String get filterAll => 'All Downloads';

  @override
  String get filterActive => 'Active';

  @override
  String get filterPaused => 'Paused';

  @override
  String get filterCompleted => 'Completed';

  @override
  String get filterFailed => 'Failed';

  @override
  String get uncategorized => 'Uncategorized';

  @override
  String get searchPlaceholder => 'Search downloads...';

  @override
  String get btnAddLink => 'Add Link';

  @override
  String get btnAddEd2kLink => 'Add eD2K Link';

  @override
  String get btnAddDownload => 'Add Download';

  @override
  String get btnCancel => 'Cancel';

  @override
  String get btnClose => 'Close';

  @override
  String get btnSave => 'Save';

  @override
  String get btnDelete => 'Delete';

  @override
  String get btnRetry => 'Retry';

  @override
  String get btnCopy => 'Copy';

  @override
  String get btnCopied => 'Copied';

  @override
  String get actionSettings => 'Settings';

  @override
  String get actionImportPartMet => 'Import .part.met';

  @override
  String get actionGuidedLabTest => 'Guided Local Test';

  @override
  String get tooltipAddCategory => 'Add Category';

  @override
  String get tooltipRefresh => 'Refresh Downloads';

  @override
  String get tooltipDevTools => 'Toggle Simulation & Debug Tools';

  @override
  String get tooltipResume => 'Resume';

  @override
  String get tooltipPause => 'Pause';

  @override
  String get tooltipDelete => 'Delete';

  @override
  String get emptyStateTitle => 'No downloads in this view';

  @override
  String get emptyStateSubtitle =>
      'Add an eD2K link to create a local download entry.\nThis lab build operates in loopback-only mode.';

  @override
  String get zeroSpeedDual => '↓ 0 B/s   ↑ 0 B/s';

  @override
  String get daemonConnected => 'Daemon Connected';

  @override
  String get daemonConnecting => 'Connecting to daemon...';

  @override
  String get daemonReconnecting => 'Reconnecting...';

  @override
  String get daemonError => 'Daemon Error';

  @override
  String get daemonDisconnected => 'Daemon Disconnected';

  @override
  String daemonVersionLine(String version, String protocol) {
    return 'Daemon v$version (proto v$protocol)';
  }

  @override
  String get dialogAddTitle => 'Add eD2K Download';

  @override
  String get dialogFieldLabel => 'eD2K Link URL';

  @override
  String get dialogFieldHint => 'ed2k://|file|name|size|hash|/';

  @override
  String get dialogValidationPrefix => 'Must start with \"ed2k://|file|\"';

  @override
  String get dialogValidationParts => 'Must contain: name, size, and hash';

  @override
  String get diagnosticsBadge => 'Diagnostics';

  @override
  String get tooltipDiagnostics =>
      'Click to inspect system & workdir diagnostics.';

  @override
  String get tooltipSystemDiagnostics =>
      'System Diagnostics & Confinement (127.0.0.1)';

  @override
  String get tooltipOpenDrawer => 'Open navigation menu';

  @override
  String get tooltipLabGuide => 'Guided Local Test Flow';

  @override
  String get killSwitchBadgeDisabled => 'Disabled';

  @override
  String get tooltipPolicyKill =>
      'Network Subsystem is DISABLED (Kill Switch engaged).\nClick to view policy and re-enable.';

  @override
  String tooltipPolicyInfo(String profile, String status) {
    return 'Network Policy: $profile\nStatus: $status\nClick to inspect policy, quotas, and audit log.';
  }

  @override
  String get tooltipKillSwitchEnable => 'Re-enable Network Subsystem';

  @override
  String get tooltipKillSwitchDisable =>
      'Kill Switch: Disconnect All Sockets Immediately';

  @override
  String get tooltipIpFilter => 'IP Filter & Rate Limits';
}
