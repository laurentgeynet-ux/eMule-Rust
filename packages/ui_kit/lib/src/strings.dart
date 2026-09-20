/// Centralized UI strings for the Emule-Sion desktop application.
/// All user-facing strings are strictly in English to ensure international consistency
/// and provide a clean foundation for future i18n/localization files.
@Deprecated(
  'Use AppLocalizations (gen_l10n) via `context.l10n` in apps instead. '
  'Kept for reference until all consumers migrate.',
)
abstract final class AppStrings {
  // Application branding
  static const String appName = 'Emule-Sion';
  static const String appTagline = 'Modern eD2K Client';

  // Network & Mode Badges
  static const String loopbackLabMode = 'Loopback Lab Mode';
  static const String tooltipLoopbackLabMode =
      'Laboratory mode.\n'
      'Connections are restricted to 127.0.0.1.\n\n'
      'No public eD2K servers, Kad nodes,\n'
      'or external peers are contacted.';
  static const String devModeBadge = 'DEV';

  // Navigation & Sections
  static const String sectionDownloads = 'DOWNLOADS';
  static const String filterAll = 'All Downloads';
  static const String filterActive = 'Active';
  static const String filterPaused = 'Paused';
  static const String filterCompleted = 'Completed';
  static const String filterFailed = 'Failed';

  // Search & Actions
  static const String searchPlaceholder = 'Search downloads...';
  static const String btnAddLink = 'Add Link';
  static const String btnAddEd2kLink = 'Add eD2K Link';
  static const String btnAddDownload = 'Add Download';
  static const String btnCancel = 'Cancel';
  static const String tooltipRefresh = 'Refresh Downloads';
  static const String tooltipDevTools = 'Toggle Simulation & Debug Tools';
  static const String tooltipResume = 'Resume';
  static const String tooltipPause = 'Pause';
  static const String tooltipDelete = 'Delete';

  // Empty State
  static const String emptyStateTitle = 'No downloads in this view';
  static const String emptyStateSubtitle =
      'Add an eD2K link to create a local download entry.\nThis lab build operates in loopback-only mode.';

  // Speed Indicators
  static const String zeroSpeedDual = '↓ 0 B/s   ↑ 0 B/s';

  // Daemon Connection Statuses
  static const String daemonConnected = 'Daemon Connected';
  static const String daemonConnecting = 'Connecting to daemon...';
  static const String daemonReconnecting = 'Reconnecting...';
  static const String daemonError = 'Daemon Error';
  static const String daemonDisconnected = 'Daemon Disconnected';

  // Dialogs & Validation
  static const String dialogAddTitle = 'Add eD2K Download';
  static const String dialogFieldLabel = 'eD2K Link URL';
  static const String dialogFieldHint = 'ed2k://|file|name|size|hash|/';
  static const String dialogValidationPrefix = 'Must start with "ed2k://|file|"';
  static const String dialogValidationParts = 'Must contain: name, size, and hash';
}
