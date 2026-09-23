// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appName => 'EMule-Rust';

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
  String get navUploads => 'Uploads';

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
  String get searchPlaceholderShares => 'Search shared files...';

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
  String get tooltipSessionTotals =>
      'Session totals — bytes downloaded / uploaded since daemon start.\nClick to open History & Statistics.';

  @override
  String get tooltipLifetimeTotals =>
      'Lifetime totals — bytes downloaded / uploaded since install.\nClick to open History & Statistics. Reset in Settings → Advanced.';

  @override
  String get settingsLifetimeResetTile => 'Reset lifetime transfer totals';

  @override
  String get settingsLifetimeResetSub =>
      'Zeroes the cumulative downloaded / uploaded counters shown in the top bar. Session totals are kept.';

  @override
  String get lifetimeResetConfirmTitle => 'Reset lifetime totals?';

  @override
  String get lifetimeResetConfirmBody =>
      'The cumulative download and upload counters since install will be reset to zero. Current session totals are not affected.';

  @override
  String get lifetimeResetConfirmButton => 'Reset';

  @override
  String get renameFileTooltip => 'Rename file';

  @override
  String get renameFileTitle => 'Rename file';

  @override
  String get renameFileLabel => 'New file name';

  @override
  String get renameFileInvalid =>
      'Invalid name — must not be empty, be \'.\'/\'..\', end with \'.\' or contain < > : \" / \\ | ? *';

  @override
  String get renameFileFailed =>
      'Rename failed on the daemon (name taken or file locked)';

  @override
  String get renameFileApply => 'Rename';

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
  String get sansCategorieNone => 'Uncategorized';

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
      '• The download entry in SQLite will be permanently deleted.\n• Temporary files (.part and .part.met) in data/temp/ will be removed.\n• If the download is completed, an option below also allows deleting the final file in the destination folder.';

  @override
  String get deleteTemporaryPartFileFrom =>
      'Delete temporary .part file from data/temp/';

  @override
  String get confirmRemove => 'Confirm Remove';

  @override
  String get sourcesEd2kSessionPeer => 'SOURCES ED2K & SESSION PEER';

  @override
  String get ajouterSourceIpPortEx =>
      'Add source IP:port (e.g. 127.0.0.1:39735)';

  @override
  String get ajouter => 'Add';

  @override
  String get demanderAuServeur => 'Query the server';

  @override
  String get demanderAuReseauKad => 'Query the Kad network';

  @override
  String get actualiserLaListe => 'Refresh list';

  @override
  String sessionActive(String peerAddress) {
    return 'Active session: $peerAddress';
  }

  @override
  String recu(String transferredBytes, String transferRateBps) {
    return 'Received: $transferredBytes ($transferRateBps)';
  }

  @override
  String get deconnecter => 'Disconnect';

  @override
  String cooldownS(String cooldownSecondsRemaining) {
    return 'Cooldown ${cooldownSecondsRemaining}s';
  }

  @override
  String get forcer => 'Force';

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
      'The download will be staged into data/temp/ and initialized in Paused state. The import never contacts any eD2K peer.';

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
      'Limits are enforced asynchronously via Token Buckets and persisted across daemon restarts. 0 means unlimited.';

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
  String get deleteSharedCompletedFile =>
      'Also delete the shared file in the destination folder';

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
      'This path is the default destination for new downloads. .part files stay in the configured temporary directory until completed.';

  @override
  String get destinationDirectoryPath => 'Destination Directory Path';

  @override
  String get eGCDownloadsOr => 'e.g. C:\\Downloads or downloads';

  @override
  String get save => 'Save';

  @override
  String get resetToPortableDefault => 'Reset to Portable Default';

  @override
  String get modeReseauTraverseeBoxUpnp =>
      'Network Mode & Router Traversal (UPnP)';

  @override
  String get basculeEntreLaboratoireLocalEt =>
      'Switch between local lab and worldwide public network.';

  @override
  String get configurer => 'Configure';

  @override
  String get historyRetentionPolicy => 'History Retention Policy';

  @override
  String get historicalTransitionEventsOlderThan =>
      'History events older than the retention period are pruned at daemon startup and on every change of this setting.';

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
      'Deleting this category will move all associated downloads to \"Uncategorized\".\n\nNo downloads or files will be deleted.';

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
  String get amorcerLeReseauKadBootstrap => 'Bootstrap the Kad network';

  @override
  String get adresseIp => 'IP Address';

  @override
  String get portUdp => 'UDP Port';

  @override
  String get annuler => 'Cancel';

  @override
  String get lancerLAmorcage => 'Start bootstrap';

  @override
  String get amorcerKadDepuisUneUrl => 'Bootstrap Kad from a URL';

  @override
  String get telechargezUnFichierNodesDat =>
      'Download a remote nodes.dat file to bootstrap your Kad Kademlia routing table (e.g. https://upd.emule-security.org/nodes.dat):';

  @override
  String get urlDuNodesDat => 'nodes.dat URL';

  @override
  String get noteSeulsLesProtocolesHttp =>
      'Note: Only public HTTP/HTTPS protocols are accepted. Traffic to local networks (RFC1918) and loopback is rejected for safety (anti-SSRF).';

  @override
  String get telechargerEtAmorcer => 'Download and Bootstrap';

  @override
  String get effacerLaTableDeRoutage => 'Clear the Kad routing table?';

  @override
  String get tousLesContactsKadEn =>
      'All Kad contacts in memory and in the local SQLite database will be deleted.\nNo network packet will be sent.';

  @override
  String get effacer => 'Clear';

  @override
  String get kadId128BitXor => 'Kad ID (128-bit XOR)';

  @override
  String get adresseIpPorts => 'IP Address & Ports';

  @override
  String get version => 'Version';

  @override
  String get statut => 'Status';

  @override
  String get echecs => 'Failed';

  @override
  String get derniereVue => 'Last seen';

  @override
  String get actions => 'Actions';

  @override
  String v(String version) {
    return 'v$version';
  }

  @override
  String get envoyerKadPing0x60 => 'Send Kad Ping (0x60)';

  @override
  String get aucunContactKad => 'No Kad contacts';

  @override
  String get importezUnFichierNodesDat =>
      'Import a nodes.dat file to bootstrap the K-buckets routing table.';

  @override
  String get importerNodesDat => 'Import nodes.dat';

  @override
  String get filtrerParKadIdIp => 'Filter by Kad ID, IP or Port...';

  @override
  String get effacerLaTable => 'Clear table';

  @override
  String get importerUnFichierNodesDat => 'Import a nodes.dat file';

  @override
  String get specifiezLeCheminAbsoluVers =>
      'Enter the absolute path to a nodes.dat file (v0, v1 or v2).';

  @override
  String get inspecter => 'Inspect';

  @override
  String formatNodesDatVDetecte(String version) {
    return 'nodes.dat format v$version detected';
  }

  @override
  String contactsDeclares(String declaredCount) {
    return '• Declared contacts: $declaredCount';
  }

  @override
  String contactsValidesAdmissibles(String validContactsCount) {
    return '• Valid & eligible contacts: $validContactsCount';
  }

  @override
  String rejetesParLaPolitiqueReseau(String rejectedByPolicyCount) {
    return '• Rejected by network policy: $rejectedByPolicyCount';
  }

  @override
  String rejetesParLeFiltreIp(String rejectedByFilterCount) {
    return '• Rejected by IP filter: $rejectedByFilterCount';
  }

  @override
  String get importerLesContacts => 'Import contacts';

  @override
  String get idKadLocal => 'Local Kad ID:';

  @override
  String get arreterUdp => 'Stop UDP';

  @override
  String get socketUdp => 'UDP Socket';

  @override
  String get paquetsTxRx => 'TX / RX Packets';

  @override
  String get contactsActifs => 'Active contacts';

  @override
  String get bucketsActifs => 'Active buckets';

  @override
  String get contactsEnBase => 'Stored contacts';

  @override
  String get profil => 'Profile';

  @override
  String get reseauKadDhtKademlia => 'Kad DHT Network (Kademlia)';

  @override
  String get actualiser => 'Refresh';

  @override
  String get amorcerBootstrap => 'Bootstrap';

  @override
  String get amorcerDepuisUrl => 'Bootstrap from URL';

  @override
  String get allerAKad => 'Go to Kad';

  @override
  String get allerAuxServeurs => 'Go to Servers';

  @override
  String get nonConnecteAUnServeur => 'Not connected to an eD2K server.';

  @override
  String get connectezVousDAbordPour => 'Connect first to perform a search.';

  @override
  String get vousNEtesActuellementPas =>
      'You are not currently connected to an eD2K server. Connect first to perform a search.';

  @override
  String get extensionExIsoZipAvi => 'Extension (ex: iso, zip, avi)';

  @override
  String get tailleMinMo => 'Min Size (MB)';

  @override
  String get tailleMaxMo => 'Max Size (MB)';

  @override
  String get effacerFiltres => 'Clear filters';

  @override
  String get serveurEd2k => 'eD2K Server';

  @override
  String get globalUdp => 'Global (UDP)';

  @override
  String get nomDuFichier => 'File name';

  @override
  String get taille => 'Size';

  @override
  String get hachageEd2k => 'eD2K Hash';

  @override
  String cliquerPourCopierLeHash(String fileHash) {
    return 'Click to copy hash: $fileHash';
  }

  @override
  String hachageCopie(String fileHash) {
    return 'Hash copied: $fileHash';
  }

  @override
  String get termine => 'Completed';

  @override
  String get telecharger => 'Download';

  @override
  String get demanderSourcesAuServeur => 'Request sources from server';

  @override
  String get copierLienEd2k => 'Copy eD2K link';

  @override
  String get lienEd2kCopieDansLe => 'eD2K link copied to clipboard';

  @override
  String get afficher => 'Show';

  @override
  String rechercheEnCoursSur(String targetLabel) {
    return 'Searching on $targetLabel...';
  }

  @override
  String get rechercheSurLeReseauEd2k => 'Search on the eD2K Network';

  @override
  String resultatS(String resultCount) {
    return '$resultCount result(s)';
  }

  @override
  String get effacerLesResultats => 'Clear results';

  @override
  String effacerLesResultats2(String resultCount) {
    return 'Clear results ($resultCount)';
  }

  @override
  String get effacerTousLesServeurs => 'Clear all servers?';

  @override
  String get cetteOperationSupprimeLaListe =>
      'This operation deletes the server list stored in your local database.';

  @override
  String get aucunServeurEd2kEnregistre => 'No eD2K servers registered';

  @override
  String get importezUnFichierServerMet =>
      'Import a local server.met file to inspect and manage your servers.';

  @override
  String get selectionnerServerMet => 'Select server.met';

  @override
  String get parDefauteMuleRustEst =>
      'By default, EMule-Rust is confined to loopback (LocalLab). Switch to PublicEd2kPreview to allow public servers.';

  @override
  String get modifierLeProfil => 'Edit profile';

  @override
  String get rechercherUnServeurNomIp => 'Search a server (name, IP, port)...';

  @override
  String get toutEffacer => 'Clear all';

  @override
  String get securiteProfil => 'Security / Profile';

  @override
  String get etatFiltre => 'State / Filter';

  @override
  String get nomDuServeur => 'Server Name';

  @override
  String get ipPort => 'IP : Port';

  @override
  String get ping => 'Ping';

  @override
  String get description => 'Description';

  @override
  String get preference => 'Preference';

  @override
  String get seConnecter => 'Connect';

  @override
  String get supprimer => 'Delete';

  @override
  String get gestionLocaleDesServeursEd2k => 'Local eD2K Server Management';

  @override
  String get importerServerMet => 'Import server.met';

  @override
  String get mettreAJourDepuisUrl => 'Update from URL';

  @override
  String get mettreAJourDepuisUne => 'Update from a URL';

  @override
  String get indiquezLAdresseHttpOu =>
      'Enter the HTTP or HTTPS address of a remote server.met file to automatically update your server list.';

  @override
  String get urlDuServerMet => 'server.met URL';

  @override
  String get noteLesAdressesIpDe =>
      'Note: Private LAN (RFC1918) and loopback IP addresses are rejected for safety (anti-SSRF).';

  @override
  String get telechargerEtMettreAJour => 'Download and Update';

  @override
  String ms(String ping) {
    return '$ping ms';
  }

  @override
  String v2(String version) {
    return 'v$version';
  }

  @override
  String get importerUnFichierServerMet => 'Import a local server.met file';

  @override
  String get indiquezLeCheminAbsoluVers =>
      'Enter the absolute path to your local server.met file:';

  @override
  String get cheminDuFichierServerMet => 'Path to server.met file';

  @override
  String get apercuDuFichierServerMet => 'server.met file preview';

  @override
  String fichier(String filePath) {
    return 'File: $filePath';
  }

  @override
  String get total => 'Total';

  @override
  String get autorises => 'Allowed';

  @override
  String get bloquesFiltreIp => 'Blocked (IP filter)';

  @override
  String get lesServeursMarquesEnRouge =>
      'Servers marked in red match ranges in your local IP filter. They will be imported with blocked status.';

  @override
  String get bloque => 'Blocked';

  @override
  String get autorise => 'Allowed';

  @override
  String get confirmerImportation => 'Confirm import';

  @override
  String get aucunServeurConnecte => 'No server connected';

  @override
  String utilisateurs(String currentUsers, String currentFiles) {
    return '$currentUsers users • $currentFiles files';
  }

  @override
  String get garantieDeSecuriteAntiSsrf => 'Anti-SSRF Security Guarantee';

  @override
  String get memeEnModePublicLa =>
      'Even in public mode, EMule-Rust\'s network policy rigorously blocks all outbound traffic to your private local network addresses (192.168.x.x, 10.x.x.x, 172.16-31.x.x) to protect your home devices.';

  @override
  String get fermer => 'Close';

  @override
  String get revenirEnModeConfineLab =>
      'Return to Confined Mode (Lab 127.0.0.1)';

  @override
  String get activerLeModeReseauPublic => 'Enable Public Network Mode (WAN)';

  @override
  String get statutUpnpIgdBoxRouteur => 'UPnP IGD status (router/NAT box)';

  @override
  String get passerelleBoxDetectee => 'Gateway / Router detected:';

  @override
  String get adresseIpExterneWan => 'External WAN IP address:';

  @override
  String portTcpEd2k(String tcpPort) {
    return 'eD2K TCP port ($tcpPort):';
  }

  @override
  String portUdpKad(String udpPort) {
    return 'Kad UDP port ($udpPort):';
  }

  @override
  String get testerRafraichirUpnp => 'Test / Refresh UPnP';

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
  String get serverEnable => 'Enable';

  @override
  String get serverDisable => 'Disable';

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
  String get language => 'Language';

  @override
  String get languageDescription => 'User interface language';

  @override
  String get french => 'Français';

  @override
  String get english => 'English';

  @override
  String get daemonConnectTitle => 'Connect to daemon';

  @override
  String get daemonConnectDescription =>
      'EMule-Rust Web controls a remote mule-daemon. Enter the daemon\'s WebSocket address.';

  @override
  String get daemonUrlLabel => 'Daemon address';

  @override
  String get daemonUrlHint => 'ws://192.168.1.10:4660/rpc';

  @override
  String get daemonUrlInvalid =>
      'Invalid address — expected format: ws://host:port/rpc';

  @override
  String get daemonTokenLabel => 'Auth token';

  @override
  String get daemonTokenHint => 'Contents of auth.token (leave empty if none)';

  @override
  String get connect => 'Connect';

  @override
  String get changeDaemon => 'Change daemon';

  @override
  String get webInterface => 'Web interface';

  @override
  String get sectionUploads => 'Uploads';

  @override
  String get uploadsActive => 'ACTIVE UPLOADS';

  @override
  String get uploadsQueue => 'WAITING QUEUE';

  @override
  String get uploadsActiveEmpty => 'No active uploads';

  @override
  String get uploadsQueueEmpty => 'Waiting queue is empty';

  @override
  String get uploadsClient => 'Client';

  @override
  String get uploadsFile => 'File';

  @override
  String get uploadsRate => 'Rate';

  @override
  String get uploadsRank => 'Rank';

  @override
  String get uploadsCancelTooltip => 'Cancel upload';

  @override
  String get settingsSectionGeneral => 'General';

  @override
  String get settingsSectionConnection => 'Connection';

  @override
  String get settingsSectionTransfers => 'Transfers';

  @override
  String get settingsSectionSecurity => 'Security';

  @override
  String get settingsSectionFriends => 'Friends';

  @override
  String get settingsSectionAdvanced => 'Advanced';

  @override
  String get settingsGeneralSubtitle =>
      'Identity, folders, automatic sharing, history and language';

  @override
  String get settingsConnectionSubtitle =>
      'Network profile, isolation, socket budgets and UPnP';

  @override
  String get settingsTransfersSubtitle => 'Swarm source connection limits';

  @override
  String get settingsSecuritySubtitle =>
      'IP filter, rate limits and anti-SSRF policy';

  @override
  String get settingsFriendsSubtitle =>
      'Dedicated upload slot bypassing the queue (eMule friend slot)';

  @override
  String get settingsAdvancedSubtitle =>
      'Diagnostics, lab tools and daemon information';

  @override
  String get settingsApply => 'Apply';

  @override
  String get settingsSaving => 'Saving…';

  @override
  String get settingsNicknameLabel => 'Nickname';

  @override
  String get settingsNicknameHelper =>
      'Announced to eD2K servers and peers (eMule \"nick\"; empty = default)';

  @override
  String get settingsShareDownloadDirTitle =>
      'Automatically share the download folder';

  @override
  String get settingsShareDownloadDirSub =>
      'Files in the Incoming folder are hashed and served to peers at daemon startup (like eMule\'s Incoming folder).';

  @override
  String get settingsTempDirLabel => 'Temporary folder (.part)';

  @override
  String get settingsTempDirHelper =>
      'Where in-progress files are stored (eMule \"Temporary directory\"; empty = daemon folder)';

  @override
  String get settingsIchTitle => 'ICH — intelligent part verification';

  @override
  String get settingsIchSub =>
      'Verifies each completed part via MD4 (eMule \"ICH\"; disabling skips the intermediate check)';

  @override
  String get settingsRememberCancelledTitle => 'Remember cancelled files';

  @override
  String get settingsRememberCancelledSub =>
      'Adds a history entry on cancellation (eMule \"Remember cancelled files\")';

  @override
  String get settingsRememberDownloadedTitle => 'Remember downloaded files';

  @override
  String get settingsRememberDownloadedSub =>
      'Adds a history entry on completion (eMule \"Remember downloaded files\")';

  @override
  String get settingsDirEmptyError => 'Directory path cannot be empty.';

  @override
  String get settingsDirSaved =>
      'Default download directory updated successfully.';

  @override
  String get settingsDirProbeFailed => 'Directory write probe failed.';

  @override
  String get settingsDirResetDone => 'Reset to portable default directory.';

  @override
  String get settingsDirResetFailed => 'Failed to reset download directory.';

  @override
  String get settingsCapPortsTitle => 'Capacities & ports';

  @override
  String get settingsCapDlLabel => 'Download capacity (KB/s)';

  @override
  String get settingsCapDlHint =>
      'Declared downlink speed — caps the download limit at 90% (0 = none)';

  @override
  String get settingsCapUlLabel => 'Upload capacity (KB/s)';

  @override
  String get settingsCapUlHint =>
      'Declared uplink speed — USS reference, caps the upload limit at 80% (0 = none)';

  @override
  String get settingsTcpPortLabel => 'TCP port';

  @override
  String get settingsTcpPortHint =>
      'eD2K client port — immediate listener rebind';

  @override
  String get settingsUdpPortLabel => 'UDP port';

  @override
  String get settingsUdpPortHint => 'Kad/UDP port — hot socket rebind';

  @override
  String get settingsUdpEnabledTitle => 'UDP enabled';

  @override
  String get settingsUdpEnabledSub =>
      'Kad UDP, server UDP requests and UDP re-asks (eMule \"UDP disabled\")';

  @override
  String get settingsUpnpToggleTitle => 'Set up ports via UPnP';

  @override
  String get settingsUpnpToggleSub =>
      'Automatically maps/unmaps TCP+UDP on the router';

  @override
  String get settingsProtocolTitle => 'Protocol & limits';

  @override
  String get settingsObfEnabledTitle => 'Obfuscated connections supported';

  @override
  String get settingsObfEnabledSub =>
      'eD2K/Kad protocol encryption (eMule \"Support obfuscated connections\")';

  @override
  String get settingsObfRequiredTitle => 'Obfuscation required';

  @override
  String get settingsObfRequiredSub =>
      'Rejects peers without encryption (eMule \"Require obfuscated connections\")';

  @override
  String get settingsUssTitle => 'USS — Upload Speed Sense';

  @override
  String get settingsUssSub =>
      'Dynamically adjusts the upload limit based on measured latency';

  @override
  String get settingsMaxConnLabel => 'Max connections';

  @override
  String get settingsMaxConnHint =>
      'Global peer-socket budget, 1–65000 (eMule MaxConnections — unbounded)';

  @override
  String get settingsConnPerMinLabel => 'Connections / min';

  @override
  String get settingsConnPerMinHint =>
      'New outbound connections per minute, 1–1200 (eMule ≈ 240)';

  @override
  String get settingsServersTitle => 'eD2K servers';

  @override
  String get settingsServerReconnectTitle => 'Automatic reconnection';

  @override
  String get settingsServerReconnectSub =>
      'Reconnects to the last server after a connection loss (eMule \"Reconnect\")';

  @override
  String get settingsServerAutoconnectTitle => 'Connect at startup';

  @override
  String get settingsServerAutoconnectSub =>
      'Automatically connects to the best known server at launch (eMule \"Autoconnect\")';

  @override
  String get settingsServerAutoremoveTitle => 'Remove dead servers';

  @override
  String get settingsServerAutoremoveSub =>
      'Removes a server from the list on first reconnection failure (eMule \"DeadServerRetry\" = 1)';

  @override
  String get settingsSafeConnectTitle => 'Safe connect';

  @override
  String get settingsSafeConnectSub =>
      'Blocks download resumption until either the eD2K server or Kad is available';

  @override
  String get settingsServerAutoupdateTitle => 'Update server list at startup';

  @override
  String get settingsServerAutoupdateSub =>
      'Imports the server.met file below at daemon launch';

  @override
  String get settingsServerMetLabel => 'server.met file';

  @override
  String get settingsServerMetHint =>
      'Local path of the .met file imported at startup (no HTTP fetch)';

  @override
  String upnpConfiguredMsg(String ip) {
    return 'UPnP configured (WAN IP: $ip).';
  }

  @override
  String get upnpNoGateway =>
      'UPnP gateway not detected or disabled on your router.';

  @override
  String upnpRefreshFailed(String error) {
    return 'UPnP refresh failed: $error';
  }

  @override
  String settingsInvalidNumMin(String label, String value, int min) {
    return '$label: \"$value\" is invalid (integer ≥ $min expected)';
  }

  @override
  String settingsInvalidNumRange(String label, String value, int min, int max) {
    return '$label: \"$value\" is invalid (integer between $min and $max expected)';
  }

  @override
  String get killSwitchConfirmTitle => 'Disconnect all connections?';

  @override
  String get killSwitchConfirmBody =>
      'The kill switch immediately cuts all network sockets (peers, servers, listeners) until manually re-enabled.';

  @override
  String get killSwitchConfirmAction => 'Disconnect all';

  @override
  String get settingsMaxGlobalLabel => 'Maximum sources (global)';

  @override
  String get settingsMaxGlobalHint =>
      'Sources activated by the swarm allocator across all files — distinct from the socket budget (Connection)';

  @override
  String get settingsMaxPerFileLabel => 'Maximum sources per file';

  @override
  String get settingsMaxPerFileHint =>
      'Default cap — overridable per file (FT_MAXSOURCES)';

  @override
  String get settingsMinPerFileLabel => 'Minimum sources per file';

  @override
  String get settingsMinPerFileHint =>
      'Guaranteed per-file source quota even when the global swarm is saturated';

  @override
  String get settingsMaxQueueLabel => 'Max upload queue';

  @override
  String get settingsMaxQueueHint =>
      'Clients waiting for an upload slot, 10–5000 (eMule QueueSize, default 5000)';

  @override
  String get settingsMinUploadLabel => 'Min upload rate per slot (KB/s)';

  @override
  String get settingsMinUploadHint =>
      'No new slot if per-slot average drops below this threshold; 0 = disabled (eMule MinUpload, default 1)';

  @override
  String get settingsDailyQuotaLabel =>
      'Daily upload quota (GiB, 0 = unlimited)';

  @override
  String get settingsDailyQuotaHint =>
      'EMule-Rust option: beyond the quota, no upload slot is granted. Resets daily (UTC).';

  @override
  String get settingsAutoPriorityTitle => 'Auto-priority for new downloads';

  @override
  String get settingsAutoPrioritySub =>
      'Files ≥ 1 eD2K part (9.28 MiB) start at \"high\" priority';

  @override
  String get settingsSwarmHealingTitle => 'Swarm healing';

  @override
  String get settingsSwarmHealingSub =>
      'EMule-Rust option: prioritizes the rarest swarm parts (few remaining sources) to become a backup source before they vanish';

  @override
  String get settingsExclusiveTitle => 'EMule-Rust exclusive options';

  @override
  String get settingsParanoidTitle => 'Paranoid mode';

  @override
  String get settingsParanoidSub =>
      'Only serves obfuscated peers whose Secure Ident signature was verified — classic eMule clients are rejected (greatly reduces compatibility)';

  @override
  String get settingsIdentityRotationLabel =>
      'Identity rotation (hours, 0 = disabled)';

  @override
  String get settingsIdentityRotationHelper =>
      'Periodically regenerates the RSA key, eD2K user hash and KadID. Invalidates accumulated credits — deliberately ephemeral identity.';

  @override
  String settingsInvalidLimits(String fields) {
    return 'Invalid limits: $fields';
  }

  @override
  String get settingsLimitDlField => 'download limit (KB/s)';

  @override
  String get settingsLimitUlField => 'upload limit (KB/s)';

  @override
  String get settingsLimitConnField => 'connections/minute';

  @override
  String settingsIdentityRotationInvalid(String value) {
    return 'Identity rotation: \"$value\" is invalid (integer between 0 and 8760 expected)';
  }

  @override
  String get rateLimitsCapacityNote =>
      'These ceilings throttle real traffic (token buckets), capped at 90% (download) / 80% (upload) of the declared line capacities (Connection tab).';

  @override
  String get rateLimitsUssActiveNote =>
      'USS is active: upload is driven automatically from measured latency. The upload limit below bounds its ceiling — 0 = no ceiling.';

  @override
  String limitsEffective(int value) {
    return 'Currently applied: $value KB/s';
  }

  @override
  String limitsEffectiveRatio(int value) {
    return 'Capped by the eD2K ratio — applied: $value KB/s';
  }

  @override
  String limitsUssDriven(int value) {
    return 'Driven by USS — current dynamic limit: $value KB/s. The value you set bounds the USS ceiling.';
  }

  @override
  String get settingsDiagnosticTile => 'Daemon diagnostics';

  @override
  String get settingsDiagnosticSub =>
      'Health, connections and internal statistics';

  @override
  String get settingsImportPartTile => 'Import a .part.met';

  @override
  String get settingsImportPartSub => 'Resume an existing eMule download';

  @override
  String get settingsLabGuideTile => 'Lab mode guide';

  @override
  String get settingsLabGuideSub =>
      'Loopback test scenarios without WAN traffic';

  @override
  String get settingsDebugBarTitle => 'Debug simulation bar';

  @override
  String get settingsDebugBarOn =>
      'Shows the simulation bar at the bottom of the screen';

  @override
  String get settingsDebugBarOff =>
      'Only available with dev tools (ENABLE_DEV_TOOLS or fake daemon)';

  @override
  String settingsDaemonVersion(String version, String protocol) {
    return 'Daemon $version — protocol $protocol';
  }

  @override
  String get settingsAddFriendTitle => 'Add a friend';

  @override
  String get settingsUserHashLabel => 'UserHash (hex)';

  @override
  String get settingsUserHashHint => '32 hexadecimal characters';

  @override
  String get settingsFriendNameLabel => 'Name (optional)';

  @override
  String get settingsNoFriends => 'No friends configured.';

  @override
  String get settingsRemoveFriendTooltip => 'Remove';

  @override
  String settingsRetentionDaysCustom(int days) {
    return '$days days';
  }

  @override
  String get settingsDirCustomHelper =>
      'Custom folder (persisted to DB, applied immediately)';

  @override
  String get settingsDirPortableHelper =>
      'Default portable folder (<bundle_root>/downloads)';

  @override
  String get netBannerKillActive =>
      'Kill switch active: all sockets are cut — inbound and outbound connections blocked.';

  @override
  String get netBannerPolicyActive =>
      'Every socket is subject to the active network profile\'s constraints (allowlist, budgets, IP filter).';

  @override
  String get upnpGatewayConnected => 'Connected';

  @override
  String get upnpGatewayAbsent => 'Not detected / disabled';

  @override
  String get upnpIpUnknown => 'Unknown';

  @override
  String get upnpMappedHighId => 'Mapped (HighID)';

  @override
  String get upnpMappedOpen => 'Mapped (Open)';

  @override
  String get upnpNotMapped => 'Not mapped';

  @override
  String get ipFilterActive => 'Filter active';

  @override
  String get ipFilterDisabled => 'Filter disabled';

  @override
  String get networksCardTitle => 'Networks';

  @override
  String get networkEd2kTitle => 'eD2K Network';

  @override
  String get networkKadTitle => 'Kad Network';

  @override
  String get netProfileEd2kDesc =>
      'Outbound TCP to public eD2K peers/servers. Rejects private LAN & loopback (anti-SSRF).';

  @override
  String get netProfileKadDesc =>
      'Kademlia DHT — outbound UDP for WAN routing. Enabling Kad also enables eD2K.';

  @override
  String ipReportTotalLines(int count) {
    return 'Total lines read: $count';
  }

  @override
  String ipReportValidEntries(int count) {
    return 'Valid entries: $count';
  }

  @override
  String ipReportCoalesced(int count) {
    return 'Coalesced (merged) ranges: $count';
  }

  @override
  String ipReportFinalRanges(int count) {
    return 'Final active intervals: $count';
  }

  @override
  String get connDisconnected => 'Disconnected';

  @override
  String get connConnecting => 'Connecting';

  @override
  String get connConnectingEllipsis => 'Connecting...';

  @override
  String get connConnected => 'Connected';

  @override
  String get connDisconnecting => 'Disconnecting...';

  @override
  String get connError => 'Error';

  @override
  String get peerHandshaking => 'eD2K Handshake...';

  @override
  String get peerQueued => 'In queue';

  @override
  String get peerTransferring => 'Downloading';

  @override
  String get dlStatusQueued => 'Queued';

  @override
  String get dlStatusWaitingForSources => 'Waiting for sources';

  @override
  String get dlStatusAllocating => 'Allocating';

  @override
  String get dlStatusDownloading => 'Downloading';

  @override
  String get dlStatusPaused => 'Paused';

  @override
  String get dlStatusVerifying => 'Verifying';

  @override
  String get dlStatusCompleting => 'Completing';

  @override
  String get dlStatusCompleted => 'Completed';

  @override
  String get dlStatusFailed => 'Failed';

  @override
  String get profileLocalLab => 'Local Lab (Loopback Only)';

  @override
  String get profilePublicEd2k => 'Public eD2K Preview';

  @override
  String get profilePublicEd2kKad => 'Public eD2K + Kad Preview';

  @override
  String get subsystemDisabled => 'Disabled (Kill Switch)';

  @override
  String get subsystemReady => 'Ready (Preview)';

  @override
  String get subsystemShuttingDown => 'Shutting Down';

  @override
  String get daemonIdle => 'Idle';

  @override
  String get daemonDiscovering => 'Discovering Daemon...';

  @override
  String get daemonStarting => 'Starting Daemon...';

  @override
  String get daemonProtocolMismatch => 'Protocol Version Incompatible';

  @override
  String get daemonAuthError => 'Authentication Failed';

  @override
  String get navShares => 'Shared Files';

  @override
  String get badgeEd2kHighId => 'eD2K · HighID';

  @override
  String get badgeEd2kLowId => 'eD2K · LowID';

  @override
  String get badgeEd2kConnecting => 'eD2K · Connecting…';

  @override
  String get badgeEd2kDisconnecting => 'eD2K · Disconnecting…';

  @override
  String get badgeEd2kDisconnected => 'eD2K · Disconnected';

  @override
  String get badgeKadStopped => 'Kad · Stopped';

  @override
  String get badgeKadConnected => 'Kad · Connected';

  @override
  String get badgeKadBootstrapping => 'Kad · Bootstrapping';

  @override
  String get daemonCardTooltipTitle => 'Daemon Status';

  @override
  String daemonCardTooltipServer(Object name) {
    return 'Server: $name';
  }

  @override
  String get daemonCardTooltipDiagnostics => 'Click to view Diagnostics';

  @override
  String get sharesTitle => 'Shared Files';

  @override
  String sharesFileCount(Object count) {
    return '$count file(s)';
  }

  @override
  String get sharesRescanTooltip => 'Rescan Incoming folder';

  @override
  String get sharesEmptyState =>
      'No shared files.\nFiles in the Incoming folder and active or completed downloads are shared automatically.';

  @override
  String sharesPartialParts(Object count) {
    return ' — partial ($count part(s))';
  }

  @override
  String get sharesTotalSentTooltip => 'Total sent for this file';

  @override
  String get sharePrioVeryLow => 'Very low';

  @override
  String get sharePrioVeryHigh => 'Very high';

  @override
  String get sharesEnabledTooltip => 'Shared';

  @override
  String get sharesDisabledTooltip => 'Suspended';

  @override
  String get sharesRenameDisabledTooltip =>
      'Download in progress — rename it from the Downloads tab';

  @override
  String get sharesRenameTooltip => 'Rename file';

  @override
  String get sharesDeleteDisabledTooltip =>
      'Download in progress — remove it from the Downloads tab';

  @override
  String get sharesDeleteTooltip => 'Delete file from disk';

  @override
  String get sharesDeleteTitle => 'Delete the file?';

  @override
  String sharesDeleteBody(Object name) {
    return '$name will be permanently deleted from disk and removed from shares.';
  }

  @override
  String get sourcesMaxDialogTitle => 'Max sources (FT_MAXSOURCES)';

  @override
  String get sourcesMaxHint => 'Empty = global limit';

  @override
  String get sourcesMaxHelper =>
      'Max number of sources for this file (1-10000)';

  @override
  String get apply => 'Apply';

  @override
  String get srcNoHash => 'No eD2K hash for this file.';

  @override
  String get srcLoading => 'Loading sources...';

  @override
  String get srcNone => 'No known sources for this file.';

  @override
  String srcConnected(Object count) {
    return 'Connected ($count)';
  }

  @override
  String srcKnown(Object count) {
    return 'Known sources ($count)';
  }

  @override
  String srcCount(Object count) {
    return 'Sources ($count)';
  }

  @override
  String srcPriorityLabel(Object priority) {
    return 'Priority: $priority';
  }

  @override
  String get srcMaxTooltip => 'Max sources for this file (FT_MAXSOURCES)';

  @override
  String srcMaxShort(Object value) {
    return 'Src max: $value';
  }

  @override
  String get peerRetry => 'Retry';

  @override
  String get peerConnect => 'Connect';

  @override
  String get pendingDestination => 'Pending destination...';

  @override
  String get pendingCalculation => 'Pending calculation...';

  @override
  String get tooltipDownloadPriority => 'Download priority';

  @override
  String get tooltipAssignCategory => 'Assign Category';

  @override
  String tooltipCategoryName(Object name) {
    return 'Category: $name';
  }

  @override
  String get peerSrcAllowed => 'Allowed';

  @override
  String get peerSrcFiltered => 'IP Filtered';

  @override
  String get peerSrcRejected => 'Profile Rejected';

  @override
  String get peerSrcTried => 'Tried';

  @override
  String get peerSrcFailed => 'Failed';

  @override
  String get peerSrcCandidate => 'Candidate';

  @override
  String get importPartOnDisk => '.part File on Disk:';

  @override
  String importPartPresent(Object size) {
    return 'Present ($size)';
  }

  @override
  String get importPartMissing => 'MISSING (.part file not found)';

  @override
  String get openFileExplorerTooltip => 'Open completed file in file explorer';

  @override
  String get openFileDisabledTooltip =>
      'Available once download and verification finish';

  @override
  String addLinkDuplicate(Object name) {
    return 'This file is already in the downloads list (\"$name\")';
  }

  @override
  String get pickDownloadDir => 'Select Download Directory';

  @override
  String get addLinkGenericError => 'Error while adding the download';

  @override
  String get catPickDir => 'Select Default Category Download Directory';

  @override
  String get catNameRequired => 'Category name cannot be empty';

  @override
  String get catEditTitle => 'Edit Category';

  @override
  String get catNewTitle => 'New Category';

  @override
  String get catNameLabel => 'Category Name *';

  @override
  String get catColorLabel => 'Color Tag';

  @override
  String get catDestLabel => 'Default Destination (Optional)';

  @override
  String get catPrioLabel => 'Default Priority (Optional)';

  @override
  String get catSaveChanges => 'Save Changes';

  @override
  String get catCreate => 'Create Category';

  @override
  String get opFailed => 'Operation failed';

  @override
  String get uploadsScore => 'Score';

  @override
  String get uploadsCredit => 'Credit';

  @override
  String get uploadsBanned => 'Banned client (score 0)';

  @override
  String get kadVerified => 'Verified';

  @override
  String get kadNotVerified => 'Not verified';

  @override
  String get kadBootstrapDescPublic =>
      'Enter the public IP address and UDP port of a Kad node to bootstrap the routing table (e.g. 212.83.184.225).';

  @override
  String get kadBootstrapDescLab =>
      'Enter the IP address and UDP port of a bootstrap node (in the LocalLab profile, only 127.0.0.1 is allowed).';

  @override
  String get kadStartUdpPublic => 'Start Kad UDP';

  @override
  String get kadStartUdpLab => 'Start Loopback UDP';

  @override
  String kadUdpActive(Object addr) {
    return 'Active ($addr)';
  }

  @override
  String get kadUdpInactive => 'Inactive';

  @override
  String get kadTimeNever => 'Never';

  @override
  String kadTimeMinutesAgo(Object min) {
    return '${min}m ago';
  }

  @override
  String kadTimeHoursAgo(Object hours) {
    return '${hours}h ago';
  }

  @override
  String kadTimeDaysAgo(Object days) {
    return '${days}d ago';
  }

  @override
  String kadBannerPublic(Object addr) {
    return 'Global Kad DHT network active on $addr | Kademlia routing open | Anti-SSRF protection active.';
  }

  @override
  String kadBannerLab(Object addr) {
    return 'UDP socket active: $addr (Confined Lab Mode) | Instant kill switch.';
  }

  @override
  String get kadBannerStartPublic =>
      'Kad UDP transport: click \"Start UDP\" to join the global Kademlia network.';

  @override
  String get kadBannerStartLab =>
      'Confined Kad UDP transport: only loopback traffic (127.0.0.1) is allowed in the LocalLab profile.';

  @override
  String get searchExplainKad =>
      'KADEMLIA2_SEARCH_KEYWORD requests query Kad nodes storing the MD4 keyword index.';

  @override
  String get searchExplainGlobal =>
      'OP_GLOBSEARCHREQ3/REQ2 requests are sent via UDP to all known servers (TCP port +4). REQ3 is used when the server advertises EXT_GETFILES + LARGEFILES.';

  @override
  String get searchExplainServer =>
      'OP_SEARCHREQUEST requests directly query the connected eD2K server index.';

  @override
  String searchNoResults(Object term) {
    return 'No results found for « $term »';
  }

  @override
  String get searchPromptKeywords => 'Enter keywords above to start a search';

  @override
  String searchEnableTarget(Object target) {
    return 'Enable $target to start searching for files';
  }

  @override
  String get searchTryOther => 'Try other terms or broaden your size filters';

  @override
  String get searchHideFilters => 'Hide filters';

  @override
  String get searchShowFilters => 'Show advanced filters';

  @override
  String get searchHintKad => 'Kad keywords (min. 3 letters)...';

  @override
  String get searchHintServer => 'File name or keywords...';

  @override
  String get searchButtonSearching => 'Searching...';

  @override
  String get searchButton => 'Search';

  @override
  String get searchTargetServer => 'eD2K Server';

  @override
  String get searchTargetKad => 'Kad DHT Network';

  @override
  String get searchTargetLabelKad => 'the Kad DHT network';

  @override
  String get searchTargetLabelGlobal => 'eD2K servers (UDP)';

  @override
  String get searchTargetLabelServer => 'the eD2K server';

  @override
  String get searchTargetEnableGlobal => 'the eD2K server list';

  @override
  String get searchTargetEnableServer => 'an eD2K server';

  @override
  String kadActiveBanner(
    Object addr,
    Object buckets,
    Object contacts,
    Object profile,
  ) {
    return 'Kad DHT Active • Contacts: $contacts ($buckets k-buckets) • Bind: $addr • Profile: $profile';
  }

  @override
  String get unknown => 'Unknown';

  @override
  String get kadInactiveBanner =>
      'The Kad DHT network is not yet active. Start the Kad socket or perform a bootstrap.';

  @override
  String get searchGlobalBanner =>
      'Global UDP search — queries all known servers (REQ3 if the server supports EXT_GETFILES + LARGEFILES).';

  @override
  String searchConnectedBanner(
    Object addr,
    Object id,
    Object idType,
    Object name,
  ) {
    return 'Connected to $name ($addr) • Client ID: $id ($idType)';
  }

  @override
  String get serverUnnamed => 'Unnamed';

  @override
  String get serverDisconnectBtn => 'Disconnect';

  @override
  String get serverBannerConnected =>
      'eD2K connections available. Check your network profile (LocalLab vs PublicEd2kPreview) before connecting. Addresses in your IP filter are automatically flagged.';

  @override
  String get serverBannerDisconnected =>
      'Network warning: importing or managing a server list does not establish any outbound connection. Addresses in your active IP filter are automatically flagged.';

  @override
  String get serverBlockedByFilter => 'Blocked by IP Filter';

  @override
  String get serverDisabledBadge => 'Disabled';

  @override
  String get historyIntegrityErrors => 'Integrity errors detected';

  @override
  String get labStep2Desc =>
      'Use the hash to create a tracked download task inside the client.';

  @override
  String get labAddedToDownloads => 'Added to Downloads!';

  @override
  String get labAddToDownloads => 'Add to Downloads';

  @override
  String get labStep4Desc =>
      'Instructs the client to connect to the 127.0.0.1 source to transfer chunks.';

  @override
  String labDlDone(Object size) {
    return 'Download completed and verified ($size)';
  }

  @override
  String labDlTransferring(Object received, Object speed, Object total) {
    return 'Transferring: $received / $total ($speed)';
  }

  @override
  String labDlReady(Object state) {
    return 'State: $state — Ready to connect to loopback peer.';
  }

  @override
  String get labAlreadyCompleted => 'Already Completed';

  @override
  String get labConnectToSource => 'Connect to Source';

  @override
  String get labStep1Desc =>
      'Creates a deterministic file in <workdir>/fixtures/ and computes its eD2K hash.';

  @override
  String get labGenerating => 'Generating...';

  @override
  String get labGenerateFixture => 'Generate Fixture';

  @override
  String get labStep3Desc =>
      'Spawns a local eD2K protocol peer hosting the test fixtures.';

  @override
  String get labStopPeer => 'Stop Loopback Peer';

  @override
  String get labStartPeer => 'Start Loopback Peer';

  @override
  String labPeerActive(Object addr, Object count) {
    return 'Active on $addr (serving $count file(s))';
  }

  @override
  String get labPeerOffline => 'Peer offline';

  @override
  String get labStep5Desc =>
      'Purges generated files in <workdir>/fixtures and partial files in <workdir>/temp. Completed downloads in downloads/ are preserved and never touched.';

  @override
  String get labCleanupSafe => 'Safe to run in Portable mode.';

  @override
  String get labCleanupRestricted =>
      'Cleanup is restricted in installed mode to protect user files.';

  @override
  String labErrCreateFixture(Object error) {
    return 'Failed to create fixture: $error';
  }

  @override
  String labErrAddDownload(Object error) {
    return 'Failed to add download: $error';
  }

  @override
  String labErrPeerOp(Object error) {
    return 'Peer operation failed: $error';
  }

  @override
  String labErrConnectDownload(Object error) {
    return 'Failed to connect download: $error';
  }

  @override
  String labErrCleanup(Object error) {
    return 'Cleanup failed: $error';
  }

  @override
  String get notAvailable => 'Not available';

  @override
  String get copyFullPath => 'Copy full path';

  @override
  String get copiedPathInternal => 'Internal data path copied to clipboard';

  @override
  String get copyDownloadsPath => 'Copy downloads directory path';

  @override
  String get copiedDownloadsPath => 'Downloads path copied to clipboard';

  @override
  String get copyDaemonLogPath => 'Copy daemon log path';

  @override
  String get copiedDaemonLogPath => 'Daemon log path copied to clipboard';

  @override
  String diagNetworkProfile(Object profile) {
    return 'Network Profile: $profile';
  }

  @override
  String msgDownloadAdded(Object name) {
    return 'File \"$name\" added to downloads';
  }

  @override
  String msgDownloadDuplicate(Object name, Object status) {
    return 'This file is already in the downloads list (\"$name\", status: $status)';
  }

  @override
  String msgImportedFile(Object name, Object parts, Object verified) {
    return 'Imported \"$name\" ($verified/$parts parts verified)';
  }

  @override
  String msgErrImportPartMet(Object error) {
    return 'Failed to import .part.met: $error';
  }

  @override
  String msgErrPause(Object error) {
    return 'Failed to pause: $error';
  }

  @override
  String msgErrResume(Object error) {
    return 'Failed to resume: $error';
  }

  @override
  String msgErrRename(Object error) {
    return 'Failed to rename: $error';
  }

  @override
  String msgErrRemove(Object error) {
    return 'Failed to remove: $error';
  }

  @override
  String msgErrFetchDetails(Object error) {
    return 'Failed to fetch details: $error';
  }

  @override
  String msgDestinationChanged(Object dir) {
    return 'Destination changed to $dir';
  }

  @override
  String msgErrChangeDestination(Object error) {
    return 'Failed to change destination: $error';
  }

  @override
  String msgRetryLoopback(Object name) {
    return 'Retrying loopback connection for \"$name\"';
  }

  @override
  String msgErrRetryConn(Object error) {
    return 'Failed to retry connection: $error';
  }

  @override
  String msgErrOpenFile(Object error) {
    return 'Could not open completed file location: $error';
  }

  @override
  String msgDownloadCompleted(Object name) {
    return 'Download completed: $name';
  }

  @override
  String msgDownloadFailed(Object name) {
    return 'Download failed: $name';
  }

  @override
  String msgCategoryCreated(Object name) {
    return 'Created category \"$name\"';
  }

  @override
  String msgErrCategoryCreate(Object error) {
    return 'Failed to create category: $error';
  }

  @override
  String msgCategoryUpdated(Object name) {
    return 'Updated category \"$name\"';
  }

  @override
  String msgErrCategoryUpdate(Object error) {
    return 'Failed to update category: $error';
  }

  @override
  String msgCategoryDeleted(Object name) {
    return 'Deleted category \"$name\"';
  }

  @override
  String msgErrCategoryDelete(Object error) {
    return 'Failed to delete category: $error';
  }

  @override
  String msgErrCategoryReorder(Object error) {
    return 'Failed to reorder categories: $error';
  }

  @override
  String msgAssignedToCategory(Object name) {
    return 'Assigned to category \"$name\"';
  }

  @override
  String get categoryFallback => 'Category';

  @override
  String msgProfileSwitched(Object profile) {
    return 'Network profile switched: $profile';
  }

  @override
  String msgErrProfileSwitch(Object error) {
    return 'Failed to change network profile: $error';
  }

  @override
  String get msgKillSwitchOn =>
      'Kill switch engaged: all network connections were cut';

  @override
  String msgErrKillSwitch(Object error) {
    return 'Kill switch failed: $error';
  }

  @override
  String get msgNetworkReactivated => 'Network reactivated';

  @override
  String msgErrNetworkReactivate(Object error) {
    return 'Failed to reactivate network: $error';
  }

  @override
  String get msgBudgetsUpdated => 'Resource budgets updated';

  @override
  String msgErrBudgets(Object error) {
    return 'Failed to update resource budgets: $error';
  }

  @override
  String msgIpFilterImported(Object count) {
    return 'IP filter imported: $count ranges';
  }

  @override
  String msgErrIpFilterImport(Object error) {
    return 'Failed to import IP filter: $error';
  }

  @override
  String get msgIpFilterEnabled => 'IP filter enabled';

  @override
  String get msgIpFilterDisabled => 'IP filter disabled';

  @override
  String msgErrIpFilterToggle(Object error) {
    return 'Failed to toggle IP filter: $error';
  }

  @override
  String get msgIpFilterCleared => 'IP filter cleared';

  @override
  String msgErrIpFilterClear(Object error) {
    return 'Failed to clear IP filter: $error';
  }

  @override
  String get msgRateLimitsUpdated => 'Rate limits updated';

  @override
  String msgErrRateLimits(Object error) {
    return 'Failed to update rate limits: $error';
  }

  @override
  String msgErrLoadSources(Object error) {
    return 'Failed to load sources: $error';
  }

  @override
  String msgSourceAdded(Object address) {
    return 'Source $address added';
  }

  @override
  String msgErrAddSource(Object error) {
    return 'Failed to add source: $error';
  }

  @override
  String msgConnInitiated(Object address) {
    return 'Connection to $address initiated';
  }

  @override
  String msgErrConnectSource(Object error) {
    return 'Failed to connect to source: $error';
  }

  @override
  String msgPeerDisconnected(Object address) {
    return 'Peer $address disconnected';
  }

  @override
  String get msgSessionsDisconnected => 'Peer session(s) disconnected';

  @override
  String msgErrDisconnectPeer(Object error) {
    return 'Failed to disconnect peer: $error';
  }

  @override
  String msgPriorityChanged(Object priority) {
    return 'Priority changed: $priority';
  }

  @override
  String msgErrPriority(Object error) {
    return 'Failed to change priority: $error';
  }

  @override
  String get msgSrcLimitReset => 'Source limit reset';

  @override
  String msgSrcLimitSet(Object max) {
    return 'Source limit: $max';
  }

  @override
  String msgErrSrcLimit(Object error) {
    return 'Failed to change limit: $error';
  }

  @override
  String get msgSourcesCleared => 'Sources cleared';

  @override
  String msgErrClearSources(Object error) {
    return 'Failed to clear sources: $error';
  }

  @override
  String msgErrKadContacts(Object error) {
    return 'Failed to fetch Kad contacts: $error';
  }

  @override
  String msgErrNodesDatInspect(Object error) {
    return 'Failed to inspect nodes.dat: $error';
  }

  @override
  String msgKadContactsImported(Object count) {
    return 'Imported $count Kad contacts into K-buckets';
  }

  @override
  String get msgErrNodesDatImport => 'Failed to import nodes.dat';

  @override
  String msgKadContactsCleared(Object count) {
    return 'Cleared $count Kad contacts';
  }

  @override
  String msgErrKadClear(Object error) {
    return 'Failed to clear Kad contacts: $error';
  }

  @override
  String msgKadUdpStarted(Object addr) {
    return 'Kad UDP socket started on $addr';
  }

  @override
  String get msgErrKadUdpStart => 'Failed to start Kad UDP';

  @override
  String get msgKadUdpStopped => 'Kad UDP socket stopped';

  @override
  String get msgErrKadUdpStop => 'Failed to stop Kad UDP';

  @override
  String msgKadPong(Object addr, Object latency) {
    return 'Pong received from $addr in ${latency}ms';
  }

  @override
  String msgErrKadPing(Object addr, Object error) {
    return 'Kad ping failed ($addr): $error';
  }

  @override
  String msgKadBootstrapOk(Object count) {
    return 'Kad bootstrap successful ($count contacts received)';
  }

  @override
  String get msgErrKadBootstrap => 'Kad bootstrap failed';

  @override
  String msgKadBootstrapUrlOk(Object count) {
    return 'Kad bootstrap from URL successful ($count nodes imported)';
  }

  @override
  String get msgErrKadBootstrapUrl => 'Kad bootstrap from URL failed';

  @override
  String msgKadSearchFound(Object count) {
    return 'Kad search: $count closest nodes found';
  }

  @override
  String get msgErrKadSearch => 'Kad search failed';

  @override
  String msgKadSourcesFound(Object count, Object hash) {
    return 'Kad: $count sources found for $hash…';
  }

  @override
  String get msgErrKadSources => 'Kad source search failed';

  @override
  String msgKadPublished(Object count, Object hash) {
    return 'Kad: published to $count nodes for $hash…';
  }

  @override
  String get msgErrKadPublish => 'Kad publish failed';

  @override
  String msgSearchGlobalSent(Object term, Object udp) {
    return 'Global search \"$term\" sent$udp';
  }

  @override
  String msgSearchSent(Object term, Object udp) {
    return 'Search \"$term\" sent to server$udp';
  }

  @override
  String msgSearchNoResponse(Object term) {
    return 'Search \"$term\" finished with no server response';
  }

  @override
  String msgErrSearch(Object error) {
    return 'Search failed: $error';
  }

  @override
  String msgKadSearchDone(Object count, Object term) {
    return 'Kad search \"$term\" finished ($count result(s))';
  }

  @override
  String msgErrKadSearchRun(Object error) {
    return 'Kad search failed: $error';
  }

  @override
  String msgSourceReqSent(Object hash) {
    return 'Source request sent for $hash';
  }

  @override
  String msgErrSourceReq(Object error) {
    return 'Source request failed: $error';
  }

  @override
  String msgErrListServers(Object error) {
    return 'Failed to list servers: $error';
  }

  @override
  String msgErrInspectServerMet(Object error) {
    return 'Failed to inspect server.met: $error';
  }

  @override
  String msgServersImported(Object allowed, Object blocked, Object imported) {
    return 'Imported $imported servers ($allowed allowed, $blocked blocked by IP filter)';
  }

  @override
  String msgErrImportServerMet(Object error) {
    return 'Failed to import server.met: $error';
  }

  @override
  String msgServersUpdated(Object count) {
    return 'Update successful: $count servers processed from URL';
  }

  @override
  String msgErrServersUpdate(Object error) {
    return 'Failed to update servers from URL: $error';
  }

  @override
  String msgErrToggleServer(Object error) {
    return 'Failed to toggle server state: $error';
  }

  @override
  String msgErrDeleteServer(Object error) {
    return 'Failed to delete server: $error';
  }

  @override
  String msgServersCleared(Object count) {
    return 'Cleared $count servers';
  }

  @override
  String msgErrClearServers(Object error) {
    return 'Failed to clear servers: $error';
  }

  @override
  String msgConnectingToServer(Object name) {
    return 'Connecting to server $name...';
  }

  @override
  String msgErrConnectServer(Object error) {
    return 'Connection failed: $error';
  }

  @override
  String get msgDisconnectedFromServer => 'Disconnected from eD2K server';

  @override
  String msgErrDisconnectServer(Object error) {
    return 'Disconnection error: $error';
  }

  @override
  String get msgSettingsUpdated => 'Settings updated';

  @override
  String msgErrSettingsUpdate(Object error) {
    return 'Failed to update settings: $error';
  }

  @override
  String get msgDownloadDirUpdated => 'Default download directory updated';

  @override
  String msgErrDownloadDir(Object error) {
    return 'Failed to set download directory: $error';
  }

  @override
  String get msgDownloadDirReset =>
      'Download directory reset (portable default)';

  @override
  String msgErrDownloadDirReset(Object error) {
    return 'Failed to reset download directory: $error';
  }

  @override
  String msgErrFetchHistory(Object error) {
    return 'Failed to fetch history: $error';
  }

  @override
  String msgHistoryCleared(Object count) {
    return 'Cleared $count history record(s)';
  }

  @override
  String msgErrClearHistory(Object error) {
    return 'Failed to clear history: $error';
  }

  @override
  String msgErrExportHistory(Object error) {
    return 'Failed to export history: $error';
  }

  @override
  String get msgRetentionUnlimited => 'History retention set to unlimited';

  @override
  String msgRetentionDays(Object days) {
    return 'History retention set to $days days';
  }

  @override
  String msgErrRetention(Object error) {
    return 'Failed to update retention policy: $error';
  }

  @override
  String msgBytesInjected(Object count, Object offset) {
    return 'Injected $count bytes at offset $offset';
  }

  @override
  String msgErrInject(Object error) {
    return 'Injection failed: $error';
  }

  @override
  String msgFixtureCreated(Object name) {
    return 'Created fixture $name';
  }

  @override
  String msgPeerStarted(Object addr) {
    return 'Started loopback peer at $addr';
  }

  @override
  String get msgPeerStopped => 'Stopped loopback test peer';

  @override
  String msgConnectedToPeer(Object addr) {
    return 'Connected download to $addr';
  }

  @override
  String msgCatSetError(Object e) {
    return 'Failed to set category: $e';
  }

  @override
  String msgSourcesFound(Object hash, Object n) {
    return '$n source(s) found for $hash';
  }

  @override
  String get kadTimeJustNow => 'Just now';

  @override
  String labCreateFixtureError(Object e) {
    return 'Failed to create fixture: $e';
  }

  @override
  String labAddDownloadError(Object e) {
    return 'Failed to add download: $e';
  }

  @override
  String labPeerError(Object e) {
    return 'Peer operation failed: $e';
  }

  @override
  String labConnectError(Object e) {
    return 'Failed to connect download: $e';
  }

  @override
  String labCleanupError(Object e) {
    return 'Cleanup failed: $e';
  }

  @override
  String labDownloadComplete(Object size) {
    return 'Download completed and verified ($size)';
  }

  @override
  String labTransferring(Object rate, Object recv, Object total) {
    return 'Transferring: $recv / $total ($rate)';
  }

  @override
  String labReadyToConnect(Object state) {
    return 'State: $state — ready to connect to loopback peer.';
  }

  @override
  String get labCleanupPortableOk => 'Safe to run in Portable mode.';

  @override
  String get labCleanupPortableNo =>
      'Cleanup is restricted in installed mode to protect user files.';

  @override
  String histFilesCount(Object n) {
    return '$n files';
  }

  @override
  String histSessionCount(Object n, Object size) {
    return '$n session ($size)';
  }

  @override
  String histResumesCount(Object n) {
    return '$n resumes';
  }

  @override
  String histHashMismatchCount(Object n) {
    return '$n hash mismatch';
  }

  @override
  String get histHashVerified => '100% hash verified';

  @override
  String get histIntegrityErrors => 'Integrity errors detected';

  @override
  String peerReceivedBytes(Object bytes) {
    return 'Received: $bytes';
  }

  @override
  String searchUdpFailed(Object n) {
    return ', $n failed';
  }

  @override
  String get kadIdGenerated => '(Generated at init)';

  @override
  String get ed2kErrPrefix => 'Must start with \"ed2k://|file|\"';

  @override
  String get ed2kErrIncomplete => 'Incomplete eD2K link format';

  @override
  String get ed2kErrInvalid =>
      'Invalid filename, size, or 32-character hex hash';

  @override
  String partsCount(Object n) {
    return '$n part(s)';
  }

  @override
  String sourcesLoopback(Object n) {
    return '$n loopback';
  }

  @override
  String get diagLoopbackTitle => 'Network profile: LocalLab (loopback)';

  @override
  String get diagLoopbackSubtitle =>
      'Traffic confined to 127.0.0.1 — no data leaves the machine.';

  @override
  String get diagPublicSubtitle =>
      'WAN connections allowed — anti-SSRF protection active.';
}
