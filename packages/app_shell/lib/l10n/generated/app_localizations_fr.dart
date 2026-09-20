// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for French (`fr`).
class AppLocalizationsFr extends AppLocalizations {
  AppLocalizationsFr([String locale = 'fr']) : super(locale);

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
  String get navServers => 'Serveurs eD2K';

  @override
  String get navSearch => 'Recherche eD2K';

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
  String get uncategorized => 'Sans catégorie';

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
  String get btnClose => 'Fermer';

  @override
  String get btnSave => 'Enregistrer';

  @override
  String get btnDelete => 'Supprimer';

  @override
  String get btnRetry => 'Réessayer';

  @override
  String get btnCopy => 'Copier';

  @override
  String get btnCopied => 'Copié';

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
  String get tooltipOpenDrawer => 'Ouvrir le menu de navigation';

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

  @override
  String get addEd2kDownload => 'Add eD2K Download';

  @override
  String get ed2kLink => 'eD2K Link';

  @override
  String get category => 'Category';

  @override
  String get targetDirectory => 'Target Directory';

  @override
  String get defaultDownloadsFolder => 'Default downloads folder';

  @override
  String get browse => 'Browse...';

  @override
  String get diskAllocation => 'Disk Allocation';

  @override
  String get cancel => 'Cancel';

  @override
  String get addDownload => 'Add Download';

  @override
  String get eGMoviesMusicLinux => 'e.g. Movies, Music, Linux ISOs';

  @override
  String get inheritGlobalDestination => 'Inherit global destination';

  @override
  String get devSimulationTools => 'Dev Simulation Tools:';

  @override
  String selected(String fileName) {
    return 'Selected: \"$fileName\"';
  }

  @override
  String get inject256KbChunk => 'Inject 256 KB Chunk';

  @override
  String get inject1MbChunk => 'Inject 1 MB Chunk';

  @override
  String get clickAnyDownloadRowAbove =>
      'Click any download row above to select it for data injection tests.';

  @override
  String get closeDebugBar => 'Close Debug Bar';

  @override
  String get systemLabDiagnostics => 'System & Lab Diagnostics';

  @override
  String get workingDirectoryMode => 'Working Directory & Mode';

  @override
  String get mode => 'Mode';

  @override
  String get isolatedDataFolder => '(Isolated data folder)';

  @override
  String get internalData => 'Internal Data';

  @override
  String get downloads => 'Downloads';

  @override
  String get daemonProcess => 'Daemon & Process';

  @override
  String get processPid => 'Process PID';

  @override
  String get rpcEndpoint => 'RPC Endpoint';

  @override
  String get daemonVersion => 'Daemon Version';

  @override
  String get instanceId => 'Instance ID';

  @override
  String get sessionId => 'Session ID';

  @override
  String get daemonLog => 'Daemon Log';

  @override
  String get capabilities => 'Capabilities';

  @override
  String get none => 'None';

  @override
  String get copyShareableSanitized => 'Copy Shareable (Sanitized)';

  @override
  String get sanitizedShareableDiagnosticsCopied =>
      'Sanitized shareable diagnostics copied';

  @override
  String get copyFullLocalDev => 'Copy Full (Local Dev)';

  @override
  String get fullDiagnosticsCopiedToClipboard =>
      'Full diagnostics copied to clipboard';

  @override
  String get openLabGuide => 'Open Lab Guide';

  @override
  String get close => 'Close';

  @override
  String details(String fileName) {
    return 'Details: $fileName';
  }

  @override
  String get inspectFullDetails => 'Inspect Full Details';

  @override
  String get downloadId => 'Download ID';

  @override
  String get totalSize => 'Total Size';

  @override
  String get verifiedBytes => 'Verified Bytes';

  @override
  String get status => 'Status';

  @override
  String get sources => 'Sources';

  @override
  String get downloadNotFound => 'Download Not Found';

  @override
  String get theRequestedDownloadIsNo =>
      'The requested download is no longer available.';

  @override
  String get importPartMetExperimental => 'Import .part.met (Experimental)';

  @override
  String get importAnExistingPartialOr =>
      'Import an existing partial or completed eMule download safely.';

  @override
  String get pathToPartMetFile => 'Path to .part.met file';

  @override
  String get inspect => 'Inspect';

  @override
  String get associatedPartFilePathOptional =>
      'Associated .part file path (optional, auto-detected)';

  @override
  String get autoDetectedNextToMet => 'Auto-detected next to .met file';

  @override
  String get targetDownloadDirectory => 'Target Download Directory';

  @override
  String get targetDirectory2 => 'Target directory';

  @override
  String get categoryOptional => 'Category (Optional)';

  @override
  String get importDownload => 'Import Download';

  @override
  String get compatibilityAlert => 'Compatibility Alert';

  @override
  String get retry => 'Retry';

  @override
  String get ipFilterRateLimiters => 'IP Filter & Rate Limiters';

  @override
  String get localDefensiveIpFilterIpfilter =>
      'Local defensive IP filter (ipfilter.dat) and Token Bucket bandwidth quotas';

  @override
  String connectedDownloadToLoopbackSource(String peerAddr) {
    return 'Connected download to loopback source ($peerAddr)';
  }

  @override
  String cleanupCompletedFileSDeleted(
    String deletedFilesCount,
    String folders,
    String freedBytes,
  ) {
    return 'Cleanup completed: $deletedFilesCount file(s) deleted across $folders ($freedBytes freed).';
  }

  @override
  String get guidedLocalLoopbackTestFlow => 'Guided Local Loopback Test Flow';

  @override
  String get fullEndToEndEd2k =>
      'Full end-to-end eD2K testing on 127.0.0.1 without touching a terminal.';

  @override
  String get networkPolicyDefenseGuard => 'Network Policy & Defense Guard';

  @override
  String get defensiveSocketAuthorizationAntiSsrf =>
      'Defensive socket authorization, anti-SSRF isolation, and resource quotas';

  @override
  String get redactedNetworkAuditLogRecent =>
      'Redacted Network Audit Log (Recent Decisions)';

  @override
  String get sparseInstantGrowsDynamically =>
      'Sparse (Instant, grows dynamically)';

  @override
  String get preallocateFullSizeAvoidsFragmentation =>
      'Preallocate (Full size, avoids fragmentation)';

  @override
  String get noneMinimalInitialDiskUse => 'None (Minimal initial disk use)';

  @override
  String get sansCategorieNone => 'Sans catégorie (None)';

  @override
  String hash(String hash) {
    return 'Hash: $hash';
  }

  @override
  String get rrggbb => '#RRGGBB';

  @override
  String get normalDefault => 'Normal (Default)';

  @override
  String get low => 'Low';

  @override
  String get normal => 'Normal';

  @override
  String get high => 'High';

  @override
  String get changeTargetDestination => 'Change Target Destination';

  @override
  String selectTargetDestinationFolderFor(String fileName) {
    return 'Select target destination folder for \"$fileName\":';
  }

  @override
  String get targetDirectoryPath => 'Target Directory Path';

  @override
  String get eGDownloadsOrC => 'e.g. downloads or C:\\TargetDir';

  @override
  String get theFolderMustExistAnd =>
      'The folder must exist and be writable. Active .part data remains in data/temp/ until completed.';

  @override
  String get saveDestination => 'Save Destination';

  @override
  String copiedToClipboard(String label) {
    return '$label copied to clipboard';
  }

  @override
  String get resume => 'Resume';

  @override
  String get pause => 'Pause';

  @override
  String get retryLoopbackSource => 'Retry Loopback Source';

  @override
  String get openCompletedFile => 'Open Completed File';

  @override
  String get remove => 'Remove...';

  @override
  String get diagnosticsRedactedExport => 'Diagnostics & Redacted Export';

  @override
  String get exportSanitizedDiagnosticsZeroAuth =>
      'Export sanitized diagnostics (zero auth token secrets, loopback 127.0.0.1 confinement verification, redacted user directories).';

  @override
  String get copyRedactedDiagnostic => 'Copy Redacted Diagnostic';

  @override
  String get downloadDiagnosticCopiedToClipboard =>
      'Download diagnostic copied to clipboard (sanitized: no secrets)';

  @override
  String get identityEd2kLinks => 'Identity & eD2K Links';

  @override
  String get downloadUuid => 'Download UUID';

  @override
  String get ed2kHashMd4 => 'eD2K Hash (MD4)';

  @override
  String get canonicalEd2kLink => 'Canonical eD2K Link';

  @override
  String get transferMetricsSpeed => 'Transfer Metrics & Speed';

  @override
  String get received => 'Received';

  @override
  String get remaining => 'Remaining';

  @override
  String get verified => 'Verified';

  @override
  String get currentRate => 'Current Rate';

  @override
  String get averageRate => 'Average Rate';

  @override
  String get eta => 'ETA';

  @override
  String get attempts => 'Attempts';

  @override
  String lastError(String lastError) {
    return 'Last Error: $lastError';
  }

  @override
  String ed2kPartMapPartsOf(String length) {
    return 'eD2K Part Map ($length parts of 9.28 MB)';
  }

  @override
  String get partBreakdownIsInitializing => 'Part breakdown is initializing...';

  @override
  String part(
    String partIndex,
    String startOffset,
    String endOffset,
    String size,
    String receivedBytes,
    String status,
    String availableSourceCount,
  ) {
    return 'Part #$partIndex\nRange: $startOffset - $endOffset\nSize: $size\nReceived: $receivedBytes\nStatus: $status\nSources: $availableSourceCount';
  }

  @override
  String verified2(String verifiedCount) {
    return 'Verified ($verifiedCount)';
  }

  @override
  String completed(String completedCount) {
    return 'Completed ($completedCount)';
  }

  @override
  String downloading(String downloadingCount) {
    return 'Downloading ($downloadingCount)';
  }

  @override
  String missing(String missingCount) {
    return 'Missing ($missingCount)';
  }

  @override
  String noSource(String unavailableCount) {
    return 'No source ($unavailableCount)';
  }

  @override
  String get storageTargetDestination => 'Storage & Target Destination';

  @override
  String get temporaryPartPathConfined => 'Temporary Part Path (Confined)';

  @override
  String get finalTargetDestination => 'Final Target Destination';

  @override
  String get changeDestination => 'Change Destination';

  @override
  String get noteChangingDestinationRedirectsWhere =>
      'Note: Changing destination redirects where the completed file is assembled. Ongoing .part chunks remain safely confined to data/temp/ until final verification.';

  @override
  String get removeDownload => 'Remove Download?';

  @override
  String removeFromTheDownloadsManager(String fileName) {
    return 'Remove \"$fileName\" from the downloads manager?';
  }

  @override
  String get theDownloadEntryInSqlite =>
      '• The download entry in SQLite will be permanently deleted.\n• Temporary files (.part and .part.met) in data/temp/ will be removed if selected below.\n• The final completed file in destination (if completed) will NOT be deleted.';

  @override
  String get deleteTemporaryPartFileFrom =>
      'Delete temporary .part file from data/temp/';

  @override
  String get confirmRemove => 'Confirm Remove';

  @override
  String get sourcesEd2kSessionPeer => 'SOURCES ED2K & SESSION PEER';

  @override
  String get ajouterSourceIpPortEx =>
      'Ajouter source IP:port (ex: 127.0.0.1:39735)';

  @override
  String get ajouter => 'Ajouter';

  @override
  String get demanderAuServeur => 'Demander au serveur';

  @override
  String get demanderAuReseauKad => 'Demander au réseau Kad';

  @override
  String get actualiserLaListe => 'Actualiser la liste';

  @override
  String sessionActive(String peerAddress) {
    return 'Session active : $peerAddress';
  }

  @override
  String recu(String transferredBytes, String transferRateBps) {
    return 'Reçu : $transferredBytes ($transferRateBps)';
  }

  @override
  String get deconnecter => 'Déconnecter';

  @override
  String cooldownS(String cooldownSecondsRemaining) {
    return 'Cooldown ${cooldownSecondsRemaining}s';
  }

  @override
  String get forcer => 'Forcer';

  @override
  String get noCategory => 'No Category';

  @override
  String get totalSize2 => 'Total Size:';

  @override
  String get estAcquiredGapsComplement => 'Est. Acquired / Gaps Complement:';

  @override
  String get partCount => 'Part Count:';

  @override
  String get ed2kHash => 'eD2K Hash';

  @override
  String get importSuccessfulTheDownloadIs =>
      'Import successful! The download is paused and verified parts have been staged.';

  @override
  String get auditReport => 'Audit Report';

  @override
  String get auditReportCopiedToClipboard => 'Audit report copied to clipboard';

  @override
  String get copyReport => 'Copy Report';

  @override
  String get done => 'Done';

  @override
  String get theDownloadWillBeStaged =>
      'The download will be staged into data/temp/ and initialized in Paused state. L’import ne contacte aucun peer et le téléchargement restera en pause.';

  @override
  String get confirmCleanup => 'Confirm Cleanup';

  @override
  String thisWillRemoveFixtureFile(String fixtureCount) {
    return 'This will remove $fixtureCount fixture file(s) in <workdir>/fixtures/ and clean temporary download part files. Incoming and finished downloads will never be touched.\n\nAre you sure you want to proceed?';
  }

  @override
  String get deleteTestData => 'Delete Test Data';

  @override
  String get storageDataInternalStateVs =>
      'Storage: data/ (Internal State) vs downloads/ (Completed Files)';

  @override
  String get openFolder => 'Open Folder';

  @override
  String internalDataDbTempPart(String workdir) {
    return '• Internal data (DB, temp .part, fixtures): $workdir';
  }

  @override
  String completedDownloads(String downloadsDir) {
    return '• Completed downloads: $downloadsDir';
  }

  @override
  String get cleanTestDataFixturesIncomplete =>
      'Clean Test Data (Fixtures & Incomplete Parts)';

  @override
  String get cleanupTestData => 'Cleanup Test Data';

  @override
  String get connectDownloadToLoopbackSource =>
      'Connect Download to Loopback Source';

  @override
  String get noDownloadsInQueueAdd =>
      'No downloads in queue. Add an eD2K link in Step 2.';

  @override
  String get targetDownloadTask => 'Target Download Task';

  @override
  String warningDownloadSize(String totalBytes, String fixtureSize) {
    return 'Warning: Download size ($totalBytes) does not match fixture ($fixtureSize).';
  }

  @override
  String get startLoopbackPeerInStep =>
      'Start loopback peer in Step 3 before connecting.';

  @override
  String finalFile(String fileName, String downloadsDir) {
    return 'Final file: $downloadsDir\\$fileName';
  }

  @override
  String get generateLocalTestFixture => 'Generate Local Test Fixture';

  @override
  String get obtainCanonicalEd2kLinkAdd =>
      'Obtain Canonical eD2K Link & Add to Downloads';

  @override
  String get copyLink => 'Copy Link';

  @override
  String get ed2kLinkCopiedToClipboard => 'eD2K Link copied to clipboard';

  @override
  String get downloadTaskCreatedInUi => 'Download task created in UI';

  @override
  String get completeStep1ToGenerate =>
      'Complete Step 1 to generate an eD2K fixture first.';

  @override
  String get loopbackTestPeer1270 => 'Loopback Test Peer (127.0.0.1)';

  @override
  String get lastImportReport => 'Last Import Report';

  @override
  String invalidLinesSkipped(String invalidLines) {
    return 'Invalid lines skipped: $invalidLines';
  }

  @override
  String rangesLoaded(String rangeCount) {
    return '$rangeCount ranges loaded';
  }

  @override
  String totalBlockedConnectionAttempts(String hitsCount) {
    return 'Total blocked connection attempts: $hitsCount';
  }

  @override
  String get importIpfilterDat => 'Import ipfilter.dat';

  @override
  String get loadALocalPeerguardianEmule =>
      'Load a local PeerGuardian / eMule formatted ipfilter.dat file. Remote URL download is disallowed for safety.';

  @override
  String get import => 'Import';

  @override
  String get clear => 'Clear';

  @override
  String get noNetworkEventsRecordedYet => 'No network events recorded yet.';

  @override
  String get activeConnectivityProfile => 'Active Connectivity Profile';

  @override
  String get activeConnectionsLimits => 'Active Connections & Limits';

  @override
  String get peerSockets => 'Peer Sockets';

  @override
  String get serverSockets => 'Server Sockets';

  @override
  String get listeners => 'Listeners';

  @override
  String get connectTimeout => 'Connect Timeout';

  @override
  String get maxFrameSize => 'Max Frame Size';

  @override
  String get maxBufferMemory => 'Max Buffer Memory';

  @override
  String status2(String label) {
    return 'Status: $label';
  }

  @override
  String get reEnable => 'Re-enable';

  @override
  String get disconnectAll => 'Disconnect All';

  @override
  String get bandwidthTokenBucketQuotas => 'Bandwidth & Token Bucket Quotas';

  @override
  String get limitsAreEnforcedAsynchronouslyUsing =>
      'Limits are enforced asynchronously using Token Buckets. 0 means unlimited.';

  @override
  String get downloadLimitKbS0 => 'Download Limit (KB/s) [0 = unlimited]';

  @override
  String get uploadLimitKbS0 => 'Upload Limit (KB/s) [0 = unlimited]';

  @override
  String get maxNewConnectionsMinute => 'Max New Connections / Minute';

  @override
  String get saveLimits => 'Save Limits';

  @override
  String get inspectDetailsPartMap => 'Inspect Details & Part Map';

  @override
  String get remove2 => 'Remove';

  @override
  String removeFromDownloads(String fileName) {
    return 'Remove \"$fileName\" from downloads?';
  }

  @override
  String get deleteDownloadedPartTempFiles =>
      'Delete downloaded part/temp files from disk';

  @override
  String id(String id) {
    return 'ID: $id';
  }

  @override
  String src(String sourceCount) {
    return '$sourceCount src';
  }

  @override
  String fullId(String id) {
    return 'Full ID: $id';
  }

  @override
  String get downloadIdCopiedToClipboard => 'Download ID copied to clipboard';

  @override
  String sources2(String sourceCount) {
    return '$sourceCount sources';
  }

  @override
  String get downloadSettingsStorage => 'Download Settings & Storage';

  @override
  String get globalDefaultDownloadDirectory =>
      'Global Default Download Directory';

  @override
  String get thisPathIsUsedAs =>
      'This path is used as the default target destination when adding new downloads. Temporary .part files remain strictly confined in data/temp/ until completed.';

  @override
  String get destinationDirectoryPath => 'Destination Directory Path';

  @override
  String get eGCDownloadsOr => 'e.g. C:\\Downloads or downloads';

  @override
  String get save => 'Save';

  @override
  String get resetToPortableDefault => 'Reset to Portable Default';

  @override
  String get modeReseauTraverseeBoxUpnp => 'Mode Réseau & Traversée Box (UPnP)';

  @override
  String get basculeEntreLaboratoireLocalEt =>
      'Bascule entre laboratoire local et réseau public mondial.';

  @override
  String get configurer => 'Configurer';

  @override
  String get historyRetentionPolicy => 'History Retention Policy';

  @override
  String get historicalTransitionEventsOlderThan =>
      'Historical transition events older than the retention period are automatically pruned on daemon startup.';

  @override
  String get keepHistoryFor => 'Keep history for: ';

  @override
  String get indefinitelyNoPruning => 'Indefinitely (No pruning)';

  @override
  String delete(String name) {
    return 'Delete \"$name\"?';
  }

  @override
  String get deletingThisCategoryWillMove =>
      'Deleting this category will move all associated downloads to \"Sans catégorie\" (Uncategorized).\n\nNo downloads or files will be deleted.';

  @override
  String get deleteCategory => 'Delete Category';

  @override
  String get categoryOptions => 'Category Options';

  @override
  String get editCategory => 'Edit Category';

  @override
  String get clearDownloadHistory => 'Clear Download History?';

  @override
  String get areYouSureYouWant =>
      'Are you sure you want to permanently delete all download history and statistics records?\n\nThis action is irreversible.';

  @override
  String get clearAllHistory => 'Clear All History';

  @override
  String get noDownloadHistoryRecordedYet => 'No download history recorded yet';

  @override
  String get completedResumedFailedAndRemoved =>
      'Completed, resumed, failed, and removed transfer events will appear here.';

  @override
  String eventDetail(String fileName) {
    return 'Event Detail: $fileName';
  }

  @override
  String get viewDetails => 'View details';

  @override
  String of2(String transferredBytes, String totalBytes) {
    return '$transferredBytes of $totalBytes';
  }

  @override
  String hash2(String ed2kHash) {
    return 'Hash: $ed2kHash';
  }

  @override
  String of3(String totalBytes) {
    return 'of $totalBytes';
  }

  @override
  String res(String resumeCount, String connectionAttempts) {
    return '$resumeCount res. / $connectionAttempts att.';
  }

  @override
  String exportHistory(String toUpperCase) {
    return 'Export History ($toUpperCase)';
  }

  @override
  String get reportIsSanitizedAllLocal =>
      'Report is sanitized: all local usernames and filesystem paths are redacted.';

  @override
  String get copyToClipboard => 'Copy to Clipboard';

  @override
  String get exportCopiedToClipboard => 'Export copied to clipboard';

  @override
  String get filterByEvent => 'Filter by Event:';

  @override
  String get allEvents => 'All Events';

  @override
  String get completed2 => 'Completed';

  @override
  String get failed => 'Failed';

  @override
  String get resumed => 'Resumed';

  @override
  String get verifying => 'Verifying';

  @override
  String get removed => 'Removed';

  @override
  String recordS(String length) {
    return '$length record(s)';
  }

  @override
  String get totalCompleted => 'Total Completed';

  @override
  String get activityWindows => 'Activity Windows';

  @override
  String get resumesRetries => 'Resumes & Retries';

  @override
  String failuresLogged(String totalFailedCount) {
    return '$totalFailedCount failures logged';
  }

  @override
  String get integrityHealth => 'Integrity Health';

  @override
  String get historyLocalStatistics => 'History & Local Statistics';

  @override
  String get exportHistoryReport => 'Export history report';

  @override
  String get exportAsJsonSanitized => 'Export as JSON (Sanitized)';

  @override
  String get exportAsCsvSanitized => 'Export as CSV (Sanitized)';

  @override
  String get exportReport => 'Export Report';

  @override
  String get clearHistory => 'Clear History';

  @override
  String get refresh => 'Refresh';

  @override
  String get amorcerLeReseauKadBootstrap => 'Amorcer le réseau Kad (Bootstrap)';

  @override
  String get adresseIp => 'Adresse IP';

  @override
  String get portUdp => 'Port UDP';

  @override
  String get annuler => 'Annuler';

  @override
  String get lancerLAmorcage => 'Lancer l\'amorçage';

  @override
  String get amorcerKadDepuisUneUrl => 'Amorcer Kad depuis une URL';

  @override
  String get telechargezUnFichierNodesDat =>
      'Téléchargez un fichier nodes.dat distant pour amorcer votre table de routage Kad Kademlia (ex: https://upd.emule-security.org/nodes.dat) :';

  @override
  String get urlDuNodesDat => 'URL du nodes.dat';

  @override
  String get noteSeulsLesProtocolesHttp =>
      'Note : Seuls les protocoles HTTP/HTTPS publics sont acceptés. Le trafic vers les réseaux locaux (RFC1918) et loopback est rejeté par sécurité (anti-SSRF).';

  @override
  String get telechargerEtAmorcer => 'Télécharger et Amorcer';

  @override
  String get effacerLaTableDeRoutage => 'Effacer la table de routage Kad ?';

  @override
  String get tousLesContactsKadEn =>
      'Tous les contacts Kad en mémoire et dans la base SQLite locale seront supprimés.\nAucun paquet réseau ne sera envoyé.';

  @override
  String get effacer => 'Effacer';

  @override
  String get kadId128BitXor => 'Kad ID (128-bit XOR)';

  @override
  String get adresseIpPorts => 'Adresse IP & Ports';

  @override
  String get version => 'Version';

  @override
  String get statut => 'Statut';

  @override
  String get echecs => 'Échecs';

  @override
  String get derniereVue => 'Dernière vue';

  @override
  String get actions => 'Actions';

  @override
  String v(String version) {
    return 'v$version';
  }

  @override
  String get envoyerKadPing0x60 => 'Envoyer Kad Ping (0x60)';

  @override
  String get aucunContactKad => 'Aucun contact Kad';

  @override
  String get importezUnFichierNodesDat =>
      'Importez un fichier nodes.dat pour amorcer la table de routage K-buckets.';

  @override
  String get importerNodesDat => 'Importer nodes.dat';

  @override
  String get filtrerParKadIdIp => 'Filtrer par Kad ID, IP ou Port...';

  @override
  String get effacerLaTable => 'Effacer la table';

  @override
  String get importerUnFichierNodesDat => 'Importer un fichier nodes.dat';

  @override
  String get specifiezLeCheminAbsoluVers =>
      'Spécifiez le chemin absolu vers un fichier nodes.dat (v0, v1 ou v2).';

  @override
  String get inspecter => 'Inspecter';

  @override
  String formatNodesDatVDetecte(String version) {
    return 'Format nodes.dat v$version détecté';
  }

  @override
  String contactsDeclares(String declaredCount) {
    return '• Contacts déclarés : $declaredCount';
  }

  @override
  String contactsValidesAdmissibles(String validContactsCount) {
    return '• Contacts valides & admissibles : $validContactsCount';
  }

  @override
  String rejetesParLaPolitiqueReseau(String rejectedByPolicyCount) {
    return '• Rejetés par la politique réseau : $rejectedByPolicyCount';
  }

  @override
  String rejetesParLeFiltreIp(String rejectedByFilterCount) {
    return '• Rejetés par le filtre IP : $rejectedByFilterCount';
  }

  @override
  String get importerLesContacts => 'Importer les contacts';

  @override
  String get idKadLocal => 'ID Kad Local :';

  @override
  String get arreterUdp => 'Arrêter UDP';

  @override
  String get socketUdp => 'Socket UDP';

  @override
  String get paquetsTxRx => 'Paquets TX / RX';

  @override
  String get contactsActifs => 'Contacts actifs';

  @override
  String get bucketsActifs => 'Buckets actifs';

  @override
  String get contactsEnBase => 'Contacts en base';

  @override
  String get profil => 'Profil';

  @override
  String get reseauKadDhtKademlia => 'Réseau Kad DHT (Kademlia)';

  @override
  String get actualiser => 'Actualiser';

  @override
  String get amorcerBootstrap => 'Amorcer (Bootstrap)';

  @override
  String get amorcerDepuisUrl => 'Amorcer depuis URL';

  @override
  String get allerAKad => 'Aller à Kad';

  @override
  String get allerAuxServeurs => 'Aller aux Serveurs';

  @override
  String get nonConnecteAUnServeur => 'Non connecté à un serveur eD2K.';

  @override
  String get connectezVousDAbordPour =>
      'Connectez-vous d\'abord pour effectuer une recherche.';

  @override
  String get vousNEtesActuellementPas =>
      'Vous n\'êtes actuellement pas connecté à un serveur eD2K. Connectez-vous d\'abord pour effectuer une recherche.';

  @override
  String get extensionExIsoZipAvi => 'Extension (ex: iso, zip, avi)';

  @override
  String get tailleMinMo => 'Taille Min (Mo)';

  @override
  String get tailleMaxMo => 'Taille Max (Mo)';

  @override
  String get effacerFiltres => 'Effacer filtres';

  @override
  String get serveurEd2k => 'Serveur eD2K';

  @override
  String get globalUdp => 'Global UDP';

  @override
  String get nomDuFichier => 'Nom du fichier';

  @override
  String get taille => 'Taille';

  @override
  String get hachageEd2k => 'Hachage eD2K';

  @override
  String cliquerPourCopierLeHash(String fileHash) {
    return 'Cliquer pour copier le hash : $fileHash';
  }

  @override
  String hachageCopie(String fileHash) {
    return 'Hachage copié : $fileHash';
  }

  @override
  String get termine => 'Terminé';

  @override
  String get telecharger => 'Télécharger';

  @override
  String get demanderSourcesAuServeur => 'Demander sources au serveur';

  @override
  String get copierLienEd2k => 'Copier lien eD2K';

  @override
  String get lienEd2kCopieDansLe => 'Lien eD2K copié dans le presse-papier';

  @override
  String get afficher => 'Afficher';

  @override
  String rechercheEnCoursSur(String targetLabel) {
    return 'Recherche en cours sur $targetLabel...';
  }

  @override
  String get rechercheSurLeReseauEd2k => 'Recherche sur le Réseau eD2K';

  @override
  String resultatS(String resultCount) {
    return '$resultCount résultat(s)';
  }

  @override
  String get effacerLesResultats => 'Effacer les résultats';

  @override
  String effacerLesResultats2(String resultCount) {
    return 'Effacer les résultats ($resultCount)';
  }

  @override
  String get effacerTousLesServeurs => 'Effacer tous les serveurs ?';

  @override
  String get cetteOperationSupprimeLaListe =>
      'Cette operation supprime la liste des serveurs enregistres dans votre base locale.';

  @override
  String get aucunServeurEd2kEnregistre => 'Aucun serveur eD2K enregistre';

  @override
  String get importezUnFichierServerMet =>
      'Importez un fichier server.met local pour inspecter et gerer vos serveurs.';

  @override
  String get selectionnerServerMet => 'Selectionner server.met';

  @override
  String get parDefautEmuleSionEst =>
      'Par défaut, Emule-Sion est confiné en boucle locale (LocalLab). Basculez sur PublicEd2kPreview pour autoriser les serveurs publics.';

  @override
  String get modifierLeProfil => 'Modifier le profil';

  @override
  String get rechercherUnServeurNomIp =>
      'Rechercher un serveur (nom, IP, port)...';

  @override
  String get toutEffacer => 'Tout effacer';

  @override
  String get securiteProfil => 'Sécurité / Profil';

  @override
  String get etatFiltre => 'Etat / Filtre';

  @override
  String get nomDuServeur => 'Nom du Serveur';

  @override
  String get ipPort => 'IP : Port';

  @override
  String get ping => 'Ping';

  @override
  String get description => 'Description';

  @override
  String get preference => 'Preference';

  @override
  String get seConnecter => 'Se connecter';

  @override
  String get supprimer => 'Supprimer';

  @override
  String get gestionLocaleDesServeursEd2k => 'Gestion Locale des Serveurs eD2K';

  @override
  String get importerServerMet => 'Importer server.met';

  @override
  String get mettreAJourDepuisUrl => 'Mettre à jour depuis URL';

  @override
  String get mettreAJourDepuisUne => 'Mettre à jour depuis une URL';

  @override
  String get indiquezLAdresseHttpOu =>
      'Indiquez l\'adresse HTTP ou HTTPS d\'un fichier server.met distant pour mettre à jour automatiquement votre annuaire de serveurs :';

  @override
  String get urlDuServerMet => 'URL du server.met';

  @override
  String get noteLesAdressesIpDe =>
      'Note : Les adresses IP de réseaux privés (LAN/RFC1918) et loopback sont rejetées par sécurité (anti-SSRF).';

  @override
  String get telechargerEtMettreAJour => 'Télécharger et Mettre à jour';

  @override
  String ms(String ping) {
    return '$ping ms';
  }

  @override
  String v2(String version) {
    return 'v$version';
  }

  @override
  String get importerUnFichierServerMet =>
      'Importer un fichier server.met local';

  @override
  String get indiquezLeCheminAbsoluVers =>
      'Indiquez le chemin absolu vers votre fichier server.met local :';

  @override
  String get cheminDuFichierServerMet => 'Chemin du fichier server.met';

  @override
  String get apercuDuFichierServerMet => 'Apercu du fichier server.met';

  @override
  String fichier(String filePath) {
    return 'Fichier : $filePath';
  }

  @override
  String get total => 'Total';

  @override
  String get autorises => 'Autorises';

  @override
  String get bloquesFiltreIp => 'Bloques (Filtre IP)';

  @override
  String get lesServeursMarquesEnRouge =>
      'Les serveurs marques en rouge correspondent a des plages de votre filtre IP local. Ils seront importes avec le statut bloque.';

  @override
  String get bloque => 'Bloque';

  @override
  String get autorise => 'Autorise';

  @override
  String get confirmerImportation => 'Confirmer importation';

  @override
  String get aucunServeurConnecte => 'Aucun serveur connecté';

  @override
  String utilisateurs(String currentUsers, String currentFiles) {
    return '$currentUsers utilisateurs • $currentFiles fichiers';
  }

  @override
  String get garantieDeSecuriteAntiSsrf => 'Garantie de Sécurité Anti-SSRF';

  @override
  String get memeEnModePublicLa =>
      'Même en mode public, la politique réseau d\'Emule-Sion bloque rigoureusement tout trafic sortant vers les adresses de votre réseau local privé (192.168.x.x, 10.x.x.x, 172.16-31.x.x) pour protéger vos équipements domestiques.';

  @override
  String get fermer => 'Fermer';

  @override
  String get revenirEnModeConfineLab =>
      'Revenir en Mode Confiné (Lab 127.0.0.1)';

  @override
  String get activerLeModeReseauPublic => 'Activer le Mode Réseau Public (WAN)';

  @override
  String get profilReseauActuel => 'Profil Réseau Actuel : ';

  @override
  String get statutUpnpIgdBoxRouteur => 'Statut UPnP IGD (Box / Routeur NAT)';

  @override
  String get passerelleBoxDetectee => 'Passerelle / Box détectée :';

  @override
  String get adresseIpExterneWan => 'Adresse IP Externe (WAN) :';

  @override
  String portTcpEd2k(String tcpPort) {
    return 'Port TCP eD2K ($tcpPort) :';
  }

  @override
  String portUdpKad(String udpPort) {
    return 'Port UDP Kad ($udpPort) :';
  }

  @override
  String get testerRafraichirUpnp => 'Tester / Rafraîchir UPnP';

  @override
  String get sevenDays => '7 Days';

  @override
  String get thirtyDaysDefault => '30 Days (Default)';

  @override
  String get ninetyDays => '90 Days';

  @override
  String twentyFourHours(
    String last24hCompletedCount,
    String last7dCompletedCount,
  ) {
    return '24h: $last24hCompletedCount | 7d: $last7dCompletedCount';
  }

  @override
  String receivedOfTotal(
    String receivedBytes,
    String totalBytes,
    String progress,
  ) {
    return '$receivedBytes of $totalBytes ($progress)';
  }

  @override
  String get serverEnable => 'Activer';

  @override
  String get serverDisable => 'Desactiver';

  @override
  String get eventType => 'Event Type';

  @override
  String get statusLabel => 'Status';

  @override
  String get transferred => 'Transferred';

  @override
  String get duration => 'Duration';

  @override
  String get resumes => 'Resumes';

  @override
  String get verificationResult => 'Verification Result';

  @override
  String get sanitizedPath => 'Sanitized Path';

  @override
  String get timestamp => 'Timestamp';

  @override
  String get errorMessage => 'Error Message';

  @override
  String get language => 'Langue';

  @override
  String get languageDescription => 'Langue de l\'interface utilisateur';

  @override
  String get french => 'Français';

  @override
  String get english => 'English';
}
