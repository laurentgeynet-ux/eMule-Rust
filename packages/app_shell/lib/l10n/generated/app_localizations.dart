import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_fr.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('fr'),
    Locale('en'),
  ];

  /// Application brand name.
  ///
  /// In fr, this message translates to:
  /// **'Emule-Sion'**
  String get appName;

  /// No description provided for @appTagline.
  ///
  /// In fr, this message translates to:
  /// **'Modern eD2K Client'**
  String get appTagline;

  /// No description provided for @loopbackLabMode.
  ///
  /// In fr, this message translates to:
  /// **'Loopback Lab Mode'**
  String get loopbackLabMode;

  /// No description provided for @tooltipLoopbackLabMode.
  ///
  /// In fr, this message translates to:
  /// **'Laboratory mode.\nConnections are restricted to 127.0.0.1.\n\nNo public eD2K servers, Kad nodes,\nor external peers are contacted.'**
  String get tooltipLoopbackLabMode;

  /// No description provided for @devModeBadge.
  ///
  /// In fr, this message translates to:
  /// **'DEV'**
  String get devModeBadge;

  /// No description provided for @sectionDownloads.
  ///
  /// In fr, this message translates to:
  /// **'DOWNLOADS'**
  String get sectionDownloads;

  /// No description provided for @sectionCategories.
  ///
  /// In fr, this message translates to:
  /// **'CATEGORIES'**
  String get sectionCategories;

  /// No description provided for @sectionAuditActivity.
  ///
  /// In fr, this message translates to:
  /// **'AUDIT & ACTIVITY'**
  String get sectionAuditActivity;

  /// No description provided for @navHistoryStats.
  ///
  /// In fr, this message translates to:
  /// **'History & Statistics'**
  String get navHistoryStats;

  /// No description provided for @navServers.
  ///
  /// In fr, this message translates to:
  /// **'Serveurs eD2K'**
  String get navServers;

  /// No description provided for @navSearch.
  ///
  /// In fr, this message translates to:
  /// **'Recherche eD2K'**
  String get navSearch;

  /// No description provided for @navKad.
  ///
  /// In fr, this message translates to:
  /// **'Kad DHT'**
  String get navKad;

  /// No description provided for @filterAll.
  ///
  /// In fr, this message translates to:
  /// **'All Downloads'**
  String get filterAll;

  /// No description provided for @filterActive.
  ///
  /// In fr, this message translates to:
  /// **'Active'**
  String get filterActive;

  /// No description provided for @filterPaused.
  ///
  /// In fr, this message translates to:
  /// **'Paused'**
  String get filterPaused;

  /// No description provided for @filterCompleted.
  ///
  /// In fr, this message translates to:
  /// **'Completed'**
  String get filterCompleted;

  /// No description provided for @filterFailed.
  ///
  /// In fr, this message translates to:
  /// **'Failed'**
  String get filterFailed;

  /// No description provided for @uncategorized.
  ///
  /// In fr, this message translates to:
  /// **'Sans catégorie'**
  String get uncategorized;

  /// No description provided for @searchPlaceholder.
  ///
  /// In fr, this message translates to:
  /// **'Search downloads...'**
  String get searchPlaceholder;

  /// No description provided for @btnAddLink.
  ///
  /// In fr, this message translates to:
  /// **'Add Link'**
  String get btnAddLink;

  /// No description provided for @btnAddEd2kLink.
  ///
  /// In fr, this message translates to:
  /// **'Add eD2K Link'**
  String get btnAddEd2kLink;

  /// No description provided for @btnAddDownload.
  ///
  /// In fr, this message translates to:
  /// **'Add Download'**
  String get btnAddDownload;

  /// No description provided for @btnCancel.
  ///
  /// In fr, this message translates to:
  /// **'Cancel'**
  String get btnCancel;

  /// No description provided for @btnClose.
  ///
  /// In fr, this message translates to:
  /// **'Fermer'**
  String get btnClose;

  /// No description provided for @btnSave.
  ///
  /// In fr, this message translates to:
  /// **'Enregistrer'**
  String get btnSave;

  /// No description provided for @btnDelete.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer'**
  String get btnDelete;

  /// No description provided for @btnRetry.
  ///
  /// In fr, this message translates to:
  /// **'Réessayer'**
  String get btnRetry;

  /// No description provided for @btnCopy.
  ///
  /// In fr, this message translates to:
  /// **'Copier'**
  String get btnCopy;

  /// No description provided for @btnCopied.
  ///
  /// In fr, this message translates to:
  /// **'Copié'**
  String get btnCopied;

  /// No description provided for @actionSettings.
  ///
  /// In fr, this message translates to:
  /// **'Settings'**
  String get actionSettings;

  /// No description provided for @actionImportPartMet.
  ///
  /// In fr, this message translates to:
  /// **'Import .part.met'**
  String get actionImportPartMet;

  /// No description provided for @actionGuidedLabTest.
  ///
  /// In fr, this message translates to:
  /// **'Guided Local Test'**
  String get actionGuidedLabTest;

  /// No description provided for @tooltipAddCategory.
  ///
  /// In fr, this message translates to:
  /// **'Add Category'**
  String get tooltipAddCategory;

  /// No description provided for @tooltipRefresh.
  ///
  /// In fr, this message translates to:
  /// **'Refresh Downloads'**
  String get tooltipRefresh;

  /// No description provided for @tooltipDevTools.
  ///
  /// In fr, this message translates to:
  /// **'Toggle Simulation & Debug Tools'**
  String get tooltipDevTools;

  /// No description provided for @tooltipResume.
  ///
  /// In fr, this message translates to:
  /// **'Resume'**
  String get tooltipResume;

  /// No description provided for @tooltipPause.
  ///
  /// In fr, this message translates to:
  /// **'Pause'**
  String get tooltipPause;

  /// No description provided for @tooltipDelete.
  ///
  /// In fr, this message translates to:
  /// **'Delete'**
  String get tooltipDelete;

  /// No description provided for @emptyStateTitle.
  ///
  /// In fr, this message translates to:
  /// **'No downloads in this view'**
  String get emptyStateTitle;

  /// No description provided for @emptyStateSubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Add an eD2K link to create a local download entry.\nThis lab build operates in loopback-only mode.'**
  String get emptyStateSubtitle;

  /// No description provided for @zeroSpeedDual.
  ///
  /// In fr, this message translates to:
  /// **'↓ 0 B/s   ↑ 0 B/s'**
  String get zeroSpeedDual;

  /// No description provided for @daemonConnected.
  ///
  /// In fr, this message translates to:
  /// **'Daemon Connected'**
  String get daemonConnected;

  /// No description provided for @daemonConnecting.
  ///
  /// In fr, this message translates to:
  /// **'Connecting to daemon...'**
  String get daemonConnecting;

  /// No description provided for @daemonReconnecting.
  ///
  /// In fr, this message translates to:
  /// **'Reconnecting...'**
  String get daemonReconnecting;

  /// No description provided for @daemonError.
  ///
  /// In fr, this message translates to:
  /// **'Daemon Error'**
  String get daemonError;

  /// No description provided for @daemonDisconnected.
  ///
  /// In fr, this message translates to:
  /// **'Daemon Disconnected'**
  String get daemonDisconnected;

  /// No description provided for @daemonVersionLine.
  ///
  /// In fr, this message translates to:
  /// **'Daemon v{version} (proto v{protocol})'**
  String daemonVersionLine(String version, String protocol);

  /// No description provided for @dialogAddTitle.
  ///
  /// In fr, this message translates to:
  /// **'Add eD2K Download'**
  String get dialogAddTitle;

  /// No description provided for @dialogFieldLabel.
  ///
  /// In fr, this message translates to:
  /// **'eD2K Link URL'**
  String get dialogFieldLabel;

  /// No description provided for @dialogFieldHint.
  ///
  /// In fr, this message translates to:
  /// **'ed2k://|file|name|size|hash|/'**
  String get dialogFieldHint;

  /// No description provided for @dialogValidationPrefix.
  ///
  /// In fr, this message translates to:
  /// **'Must start with \"ed2k://|file|\"'**
  String get dialogValidationPrefix;

  /// No description provided for @dialogValidationParts.
  ///
  /// In fr, this message translates to:
  /// **'Must contain: name, size, and hash'**
  String get dialogValidationParts;

  /// No description provided for @diagnosticsBadge.
  ///
  /// In fr, this message translates to:
  /// **'Diagnostics'**
  String get diagnosticsBadge;

  /// No description provided for @tooltipDiagnostics.
  ///
  /// In fr, this message translates to:
  /// **'Click to inspect system & workdir diagnostics.'**
  String get tooltipDiagnostics;

  /// No description provided for @tooltipSystemDiagnostics.
  ///
  /// In fr, this message translates to:
  /// **'System Diagnostics & Confinement (127.0.0.1)'**
  String get tooltipSystemDiagnostics;

  /// No description provided for @tooltipOpenDrawer.
  ///
  /// In fr, this message translates to:
  /// **'Ouvrir le menu de navigation'**
  String get tooltipOpenDrawer;

  /// No description provided for @tooltipLabGuide.
  ///
  /// In fr, this message translates to:
  /// **'Guided Local Test Flow'**
  String get tooltipLabGuide;

  /// No description provided for @killSwitchBadgeDisabled.
  ///
  /// In fr, this message translates to:
  /// **'Disabled'**
  String get killSwitchBadgeDisabled;

  /// No description provided for @tooltipPolicyKill.
  ///
  /// In fr, this message translates to:
  /// **'Network Subsystem is DISABLED (Kill Switch engaged).\nClick to view policy and re-enable.'**
  String get tooltipPolicyKill;

  /// No description provided for @tooltipPolicyInfo.
  ///
  /// In fr, this message translates to:
  /// **'Network Policy: {profile}\nStatus: {status}\nClick to inspect policy, quotas, and audit log.'**
  String tooltipPolicyInfo(String profile, String status);

  /// No description provided for @tooltipKillSwitchEnable.
  ///
  /// In fr, this message translates to:
  /// **'Re-enable Network Subsystem'**
  String get tooltipKillSwitchEnable;

  /// No description provided for @tooltipKillSwitchDisable.
  ///
  /// In fr, this message translates to:
  /// **'Kill Switch: Disconnect All Sockets Immediately'**
  String get tooltipKillSwitchDisable;

  /// No description provided for @tooltipIpFilter.
  ///
  /// In fr, this message translates to:
  /// **'IP Filter & Rate Limits'**
  String get tooltipIpFilter;

  /// No description provided for @addEd2kDownload.
  ///
  /// In fr, this message translates to:
  /// **'Add eD2K Download'**
  String get addEd2kDownload;

  /// No description provided for @ed2kLink.
  ///
  /// In fr, this message translates to:
  /// **'eD2K Link'**
  String get ed2kLink;

  /// No description provided for @category.
  ///
  /// In fr, this message translates to:
  /// **'Category'**
  String get category;

  /// No description provided for @targetDirectory.
  ///
  /// In fr, this message translates to:
  /// **'Target Directory'**
  String get targetDirectory;

  /// No description provided for @defaultDownloadsFolder.
  ///
  /// In fr, this message translates to:
  /// **'Default downloads folder'**
  String get defaultDownloadsFolder;

  /// No description provided for @browse.
  ///
  /// In fr, this message translates to:
  /// **'Browse...'**
  String get browse;

  /// No description provided for @diskAllocation.
  ///
  /// In fr, this message translates to:
  /// **'Disk Allocation'**
  String get diskAllocation;

  /// No description provided for @cancel.
  ///
  /// In fr, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @addDownload.
  ///
  /// In fr, this message translates to:
  /// **'Add Download'**
  String get addDownload;

  /// No description provided for @eGMoviesMusicLinux.
  ///
  /// In fr, this message translates to:
  /// **'e.g. Movies, Music, Linux ISOs'**
  String get eGMoviesMusicLinux;

  /// No description provided for @inheritGlobalDestination.
  ///
  /// In fr, this message translates to:
  /// **'Inherit global destination'**
  String get inheritGlobalDestination;

  /// No description provided for @devSimulationTools.
  ///
  /// In fr, this message translates to:
  /// **'Dev Simulation Tools:'**
  String get devSimulationTools;

  /// No description provided for @selected.
  ///
  /// In fr, this message translates to:
  /// **'Selected: \"{fileName}\"'**
  String selected(String fileName);

  /// No description provided for @inject256KbChunk.
  ///
  /// In fr, this message translates to:
  /// **'Inject 256 KB Chunk'**
  String get inject256KbChunk;

  /// No description provided for @inject1MbChunk.
  ///
  /// In fr, this message translates to:
  /// **'Inject 1 MB Chunk'**
  String get inject1MbChunk;

  /// No description provided for @clickAnyDownloadRowAbove.
  ///
  /// In fr, this message translates to:
  /// **'Click any download row above to select it for data injection tests.'**
  String get clickAnyDownloadRowAbove;

  /// No description provided for @closeDebugBar.
  ///
  /// In fr, this message translates to:
  /// **'Close Debug Bar'**
  String get closeDebugBar;

  /// No description provided for @systemLabDiagnostics.
  ///
  /// In fr, this message translates to:
  /// **'System & Lab Diagnostics'**
  String get systemLabDiagnostics;

  /// No description provided for @workingDirectoryMode.
  ///
  /// In fr, this message translates to:
  /// **'Working Directory & Mode'**
  String get workingDirectoryMode;

  /// No description provided for @mode.
  ///
  /// In fr, this message translates to:
  /// **'Mode'**
  String get mode;

  /// No description provided for @isolatedDataFolder.
  ///
  /// In fr, this message translates to:
  /// **'(Isolated data folder)'**
  String get isolatedDataFolder;

  /// No description provided for @internalData.
  ///
  /// In fr, this message translates to:
  /// **'Internal Data'**
  String get internalData;

  /// No description provided for @downloads.
  ///
  /// In fr, this message translates to:
  /// **'Downloads'**
  String get downloads;

  /// No description provided for @daemonProcess.
  ///
  /// In fr, this message translates to:
  /// **'Daemon & Process'**
  String get daemonProcess;

  /// No description provided for @processPid.
  ///
  /// In fr, this message translates to:
  /// **'Process PID'**
  String get processPid;

  /// No description provided for @rpcEndpoint.
  ///
  /// In fr, this message translates to:
  /// **'RPC Endpoint'**
  String get rpcEndpoint;

  /// No description provided for @daemonVersion.
  ///
  /// In fr, this message translates to:
  /// **'Daemon Version'**
  String get daemonVersion;

  /// No description provided for @instanceId.
  ///
  /// In fr, this message translates to:
  /// **'Instance ID'**
  String get instanceId;

  /// No description provided for @sessionId.
  ///
  /// In fr, this message translates to:
  /// **'Session ID'**
  String get sessionId;

  /// No description provided for @daemonLog.
  ///
  /// In fr, this message translates to:
  /// **'Daemon Log'**
  String get daemonLog;

  /// No description provided for @capabilities.
  ///
  /// In fr, this message translates to:
  /// **'Capabilities'**
  String get capabilities;

  /// No description provided for @none.
  ///
  /// In fr, this message translates to:
  /// **'None'**
  String get none;

  /// No description provided for @copyShareableSanitized.
  ///
  /// In fr, this message translates to:
  /// **'Copy Shareable (Sanitized)'**
  String get copyShareableSanitized;

  /// No description provided for @sanitizedShareableDiagnosticsCopied.
  ///
  /// In fr, this message translates to:
  /// **'Sanitized shareable diagnostics copied'**
  String get sanitizedShareableDiagnosticsCopied;

  /// No description provided for @copyFullLocalDev.
  ///
  /// In fr, this message translates to:
  /// **'Copy Full (Local Dev)'**
  String get copyFullLocalDev;

  /// No description provided for @fullDiagnosticsCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Full diagnostics copied to clipboard'**
  String get fullDiagnosticsCopiedToClipboard;

  /// No description provided for @openLabGuide.
  ///
  /// In fr, this message translates to:
  /// **'Open Lab Guide'**
  String get openLabGuide;

  /// No description provided for @close.
  ///
  /// In fr, this message translates to:
  /// **'Close'**
  String get close;

  /// No description provided for @details.
  ///
  /// In fr, this message translates to:
  /// **'Details: {fileName}'**
  String details(String fileName);

  /// No description provided for @inspectFullDetails.
  ///
  /// In fr, this message translates to:
  /// **'Inspect Full Details'**
  String get inspectFullDetails;

  /// No description provided for @downloadId.
  ///
  /// In fr, this message translates to:
  /// **'Download ID'**
  String get downloadId;

  /// No description provided for @totalSize.
  ///
  /// In fr, this message translates to:
  /// **'Total Size'**
  String get totalSize;

  /// No description provided for @verifiedBytes.
  ///
  /// In fr, this message translates to:
  /// **'Verified Bytes'**
  String get verifiedBytes;

  /// No description provided for @status.
  ///
  /// In fr, this message translates to:
  /// **'Status'**
  String get status;

  /// No description provided for @sources.
  ///
  /// In fr, this message translates to:
  /// **'Sources'**
  String get sources;

  /// No description provided for @downloadNotFound.
  ///
  /// In fr, this message translates to:
  /// **'Download Not Found'**
  String get downloadNotFound;

  /// No description provided for @theRequestedDownloadIsNo.
  ///
  /// In fr, this message translates to:
  /// **'The requested download is no longer available.'**
  String get theRequestedDownloadIsNo;

  /// No description provided for @importPartMetExperimental.
  ///
  /// In fr, this message translates to:
  /// **'Import .part.met (Experimental)'**
  String get importPartMetExperimental;

  /// No description provided for @importAnExistingPartialOr.
  ///
  /// In fr, this message translates to:
  /// **'Import an existing partial or completed eMule download safely.'**
  String get importAnExistingPartialOr;

  /// No description provided for @pathToPartMetFile.
  ///
  /// In fr, this message translates to:
  /// **'Path to .part.met file'**
  String get pathToPartMetFile;

  /// No description provided for @inspect.
  ///
  /// In fr, this message translates to:
  /// **'Inspect'**
  String get inspect;

  /// No description provided for @associatedPartFilePathOptional.
  ///
  /// In fr, this message translates to:
  /// **'Associated .part file path (optional, auto-detected)'**
  String get associatedPartFilePathOptional;

  /// No description provided for @autoDetectedNextToMet.
  ///
  /// In fr, this message translates to:
  /// **'Auto-detected next to .met file'**
  String get autoDetectedNextToMet;

  /// No description provided for @targetDownloadDirectory.
  ///
  /// In fr, this message translates to:
  /// **'Target Download Directory'**
  String get targetDownloadDirectory;

  /// No description provided for @targetDirectory2.
  ///
  /// In fr, this message translates to:
  /// **'Target directory'**
  String get targetDirectory2;

  /// No description provided for @categoryOptional.
  ///
  /// In fr, this message translates to:
  /// **'Category (Optional)'**
  String get categoryOptional;

  /// No description provided for @importDownload.
  ///
  /// In fr, this message translates to:
  /// **'Import Download'**
  String get importDownload;

  /// No description provided for @compatibilityAlert.
  ///
  /// In fr, this message translates to:
  /// **'Compatibility Alert'**
  String get compatibilityAlert;

  /// No description provided for @retry.
  ///
  /// In fr, this message translates to:
  /// **'Retry'**
  String get retry;

  /// No description provided for @ipFilterRateLimiters.
  ///
  /// In fr, this message translates to:
  /// **'IP Filter & Rate Limiters'**
  String get ipFilterRateLimiters;

  /// No description provided for @localDefensiveIpFilterIpfilter.
  ///
  /// In fr, this message translates to:
  /// **'Local defensive IP filter (ipfilter.dat) and Token Bucket bandwidth quotas'**
  String get localDefensiveIpFilterIpfilter;

  /// No description provided for @connectedDownloadToLoopbackSource.
  ///
  /// In fr, this message translates to:
  /// **'Connected download to loopback source ({peerAddr})'**
  String connectedDownloadToLoopbackSource(String peerAddr);

  /// No description provided for @cleanupCompletedFileSDeleted.
  ///
  /// In fr, this message translates to:
  /// **'Cleanup completed: {deletedFilesCount} file(s) deleted across {folders} ({freedBytes} freed).'**
  String cleanupCompletedFileSDeleted(
    String deletedFilesCount,
    String folders,
    String freedBytes,
  );

  /// No description provided for @guidedLocalLoopbackTestFlow.
  ///
  /// In fr, this message translates to:
  /// **'Guided Local Loopback Test Flow'**
  String get guidedLocalLoopbackTestFlow;

  /// No description provided for @fullEndToEndEd2k.
  ///
  /// In fr, this message translates to:
  /// **'Full end-to-end eD2K testing on 127.0.0.1 without touching a terminal.'**
  String get fullEndToEndEd2k;

  /// No description provided for @networkPolicyDefenseGuard.
  ///
  /// In fr, this message translates to:
  /// **'Network Policy & Defense Guard'**
  String get networkPolicyDefenseGuard;

  /// No description provided for @defensiveSocketAuthorizationAntiSsrf.
  ///
  /// In fr, this message translates to:
  /// **'Defensive socket authorization, anti-SSRF isolation, and resource quotas'**
  String get defensiveSocketAuthorizationAntiSsrf;

  /// No description provided for @redactedNetworkAuditLogRecent.
  ///
  /// In fr, this message translates to:
  /// **'Redacted Network Audit Log (Recent Decisions)'**
  String get redactedNetworkAuditLogRecent;

  /// No description provided for @sparseInstantGrowsDynamically.
  ///
  /// In fr, this message translates to:
  /// **'Sparse (Instant, grows dynamically)'**
  String get sparseInstantGrowsDynamically;

  /// No description provided for @preallocateFullSizeAvoidsFragmentation.
  ///
  /// In fr, this message translates to:
  /// **'Preallocate (Full size, avoids fragmentation)'**
  String get preallocateFullSizeAvoidsFragmentation;

  /// No description provided for @noneMinimalInitialDiskUse.
  ///
  /// In fr, this message translates to:
  /// **'None (Minimal initial disk use)'**
  String get noneMinimalInitialDiskUse;

  /// No description provided for @sansCategorieNone.
  ///
  /// In fr, this message translates to:
  /// **'Sans catégorie (None)'**
  String get sansCategorieNone;

  /// No description provided for @hash.
  ///
  /// In fr, this message translates to:
  /// **'Hash: {hash}'**
  String hash(String hash);

  /// No description provided for @rrggbb.
  ///
  /// In fr, this message translates to:
  /// **'#RRGGBB'**
  String get rrggbb;

  /// No description provided for @normalDefault.
  ///
  /// In fr, this message translates to:
  /// **'Normal (Default)'**
  String get normalDefault;

  /// No description provided for @low.
  ///
  /// In fr, this message translates to:
  /// **'Low'**
  String get low;

  /// No description provided for @normal.
  ///
  /// In fr, this message translates to:
  /// **'Normal'**
  String get normal;

  /// No description provided for @high.
  ///
  /// In fr, this message translates to:
  /// **'High'**
  String get high;

  /// No description provided for @changeTargetDestination.
  ///
  /// In fr, this message translates to:
  /// **'Change Target Destination'**
  String get changeTargetDestination;

  /// No description provided for @selectTargetDestinationFolderFor.
  ///
  /// In fr, this message translates to:
  /// **'Select target destination folder for \"{fileName}\":'**
  String selectTargetDestinationFolderFor(String fileName);

  /// No description provided for @targetDirectoryPath.
  ///
  /// In fr, this message translates to:
  /// **'Target Directory Path'**
  String get targetDirectoryPath;

  /// No description provided for @eGDownloadsOrC.
  ///
  /// In fr, this message translates to:
  /// **'e.g. downloads or C:\\TargetDir'**
  String get eGDownloadsOrC;

  /// No description provided for @theFolderMustExistAnd.
  ///
  /// In fr, this message translates to:
  /// **'The folder must exist and be writable. Active .part data remains in data/temp/ until completed.'**
  String get theFolderMustExistAnd;

  /// No description provided for @saveDestination.
  ///
  /// In fr, this message translates to:
  /// **'Save Destination'**
  String get saveDestination;

  /// No description provided for @copiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'{label} copied to clipboard'**
  String copiedToClipboard(String label);

  /// No description provided for @resume.
  ///
  /// In fr, this message translates to:
  /// **'Resume'**
  String get resume;

  /// No description provided for @pause.
  ///
  /// In fr, this message translates to:
  /// **'Pause'**
  String get pause;

  /// No description provided for @retryLoopbackSource.
  ///
  /// In fr, this message translates to:
  /// **'Retry Loopback Source'**
  String get retryLoopbackSource;

  /// No description provided for @openCompletedFile.
  ///
  /// In fr, this message translates to:
  /// **'Open Completed File'**
  String get openCompletedFile;

  /// No description provided for @remove.
  ///
  /// In fr, this message translates to:
  /// **'Remove...'**
  String get remove;

  /// No description provided for @diagnosticsRedactedExport.
  ///
  /// In fr, this message translates to:
  /// **'Diagnostics & Redacted Export'**
  String get diagnosticsRedactedExport;

  /// No description provided for @exportSanitizedDiagnosticsZeroAuth.
  ///
  /// In fr, this message translates to:
  /// **'Export sanitized diagnostics (zero auth token secrets, loopback 127.0.0.1 confinement verification, redacted user directories).'**
  String get exportSanitizedDiagnosticsZeroAuth;

  /// No description provided for @copyRedactedDiagnostic.
  ///
  /// In fr, this message translates to:
  /// **'Copy Redacted Diagnostic'**
  String get copyRedactedDiagnostic;

  /// No description provided for @downloadDiagnosticCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Download diagnostic copied to clipboard (sanitized: no secrets)'**
  String get downloadDiagnosticCopiedToClipboard;

  /// No description provided for @identityEd2kLinks.
  ///
  /// In fr, this message translates to:
  /// **'Identity & eD2K Links'**
  String get identityEd2kLinks;

  /// No description provided for @downloadUuid.
  ///
  /// In fr, this message translates to:
  /// **'Download UUID'**
  String get downloadUuid;

  /// No description provided for @ed2kHashMd4.
  ///
  /// In fr, this message translates to:
  /// **'eD2K Hash (MD4)'**
  String get ed2kHashMd4;

  /// No description provided for @canonicalEd2kLink.
  ///
  /// In fr, this message translates to:
  /// **'Canonical eD2K Link'**
  String get canonicalEd2kLink;

  /// No description provided for @transferMetricsSpeed.
  ///
  /// In fr, this message translates to:
  /// **'Transfer Metrics & Speed'**
  String get transferMetricsSpeed;

  /// No description provided for @received.
  ///
  /// In fr, this message translates to:
  /// **'Received'**
  String get received;

  /// No description provided for @remaining.
  ///
  /// In fr, this message translates to:
  /// **'Remaining'**
  String get remaining;

  /// No description provided for @verified.
  ///
  /// In fr, this message translates to:
  /// **'Verified'**
  String get verified;

  /// No description provided for @currentRate.
  ///
  /// In fr, this message translates to:
  /// **'Current Rate'**
  String get currentRate;

  /// No description provided for @averageRate.
  ///
  /// In fr, this message translates to:
  /// **'Average Rate'**
  String get averageRate;

  /// No description provided for @eta.
  ///
  /// In fr, this message translates to:
  /// **'ETA'**
  String get eta;

  /// No description provided for @attempts.
  ///
  /// In fr, this message translates to:
  /// **'Attempts'**
  String get attempts;

  /// No description provided for @lastError.
  ///
  /// In fr, this message translates to:
  /// **'Last Error: {lastError}'**
  String lastError(String lastError);

  /// No description provided for @ed2kPartMapPartsOf.
  ///
  /// In fr, this message translates to:
  /// **'eD2K Part Map ({length} parts of 9.28 MB)'**
  String ed2kPartMapPartsOf(String length);

  /// No description provided for @partBreakdownIsInitializing.
  ///
  /// In fr, this message translates to:
  /// **'Part breakdown is initializing...'**
  String get partBreakdownIsInitializing;

  /// No description provided for @part.
  ///
  /// In fr, this message translates to:
  /// **'Part #{partIndex}\nRange: {startOffset} - {endOffset}\nSize: {size}\nReceived: {receivedBytes}\nStatus: {status}\nSources: {availableSourceCount}'**
  String part(
    String partIndex,
    String startOffset,
    String endOffset,
    String size,
    String receivedBytes,
    String status,
    String availableSourceCount,
  );

  /// No description provided for @verified2.
  ///
  /// In fr, this message translates to:
  /// **'Verified ({verifiedCount})'**
  String verified2(String verifiedCount);

  /// No description provided for @completed.
  ///
  /// In fr, this message translates to:
  /// **'Completed ({completedCount})'**
  String completed(String completedCount);

  /// No description provided for @downloading.
  ///
  /// In fr, this message translates to:
  /// **'Downloading ({downloadingCount})'**
  String downloading(String downloadingCount);

  /// No description provided for @missing.
  ///
  /// In fr, this message translates to:
  /// **'Missing ({missingCount})'**
  String missing(String missingCount);

  /// No description provided for @noSource.
  ///
  /// In fr, this message translates to:
  /// **'No source ({unavailableCount})'**
  String noSource(String unavailableCount);

  /// No description provided for @storageTargetDestination.
  ///
  /// In fr, this message translates to:
  /// **'Storage & Target Destination'**
  String get storageTargetDestination;

  /// No description provided for @temporaryPartPathConfined.
  ///
  /// In fr, this message translates to:
  /// **'Temporary Part Path (Confined)'**
  String get temporaryPartPathConfined;

  /// No description provided for @finalTargetDestination.
  ///
  /// In fr, this message translates to:
  /// **'Final Target Destination'**
  String get finalTargetDestination;

  /// No description provided for @changeDestination.
  ///
  /// In fr, this message translates to:
  /// **'Change Destination'**
  String get changeDestination;

  /// No description provided for @noteChangingDestinationRedirectsWhere.
  ///
  /// In fr, this message translates to:
  /// **'Note: Changing destination redirects where the completed file is assembled. Ongoing .part chunks remain safely confined to data/temp/ until final verification.'**
  String get noteChangingDestinationRedirectsWhere;

  /// No description provided for @removeDownload.
  ///
  /// In fr, this message translates to:
  /// **'Remove Download?'**
  String get removeDownload;

  /// No description provided for @removeFromTheDownloadsManager.
  ///
  /// In fr, this message translates to:
  /// **'Remove \"{fileName}\" from the downloads manager?'**
  String removeFromTheDownloadsManager(String fileName);

  /// No description provided for @theDownloadEntryInSqlite.
  ///
  /// In fr, this message translates to:
  /// **'• The download entry in SQLite will be permanently deleted.\n• Temporary files (.part and .part.met) in data/temp/ will be removed if selected below.\n• The final completed file in destination (if completed) will NOT be deleted.'**
  String get theDownloadEntryInSqlite;

  /// No description provided for @deleteTemporaryPartFileFrom.
  ///
  /// In fr, this message translates to:
  /// **'Delete temporary .part file from data/temp/'**
  String get deleteTemporaryPartFileFrom;

  /// No description provided for @confirmRemove.
  ///
  /// In fr, this message translates to:
  /// **'Confirm Remove'**
  String get confirmRemove;

  /// No description provided for @sourcesEd2kSessionPeer.
  ///
  /// In fr, this message translates to:
  /// **'SOURCES ED2K & SESSION PEER'**
  String get sourcesEd2kSessionPeer;

  /// No description provided for @ajouterSourceIpPortEx.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter source IP:port (ex: 127.0.0.1:39735)'**
  String get ajouterSourceIpPortEx;

  /// No description provided for @ajouter.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter'**
  String get ajouter;

  /// No description provided for @demanderAuServeur.
  ///
  /// In fr, this message translates to:
  /// **'Demander au serveur'**
  String get demanderAuServeur;

  /// No description provided for @demanderAuReseauKad.
  ///
  /// In fr, this message translates to:
  /// **'Demander au réseau Kad'**
  String get demanderAuReseauKad;

  /// No description provided for @actualiserLaListe.
  ///
  /// In fr, this message translates to:
  /// **'Actualiser la liste'**
  String get actualiserLaListe;

  /// No description provided for @sessionActive.
  ///
  /// In fr, this message translates to:
  /// **'Session active : {peerAddress}'**
  String sessionActive(String peerAddress);

  /// No description provided for @recu.
  ///
  /// In fr, this message translates to:
  /// **'Reçu : {transferredBytes} ({transferRateBps})'**
  String recu(String transferredBytes, String transferRateBps);

  /// No description provided for @deconnecter.
  ///
  /// In fr, this message translates to:
  /// **'Déconnecter'**
  String get deconnecter;

  /// No description provided for @cooldownS.
  ///
  /// In fr, this message translates to:
  /// **'Cooldown {cooldownSecondsRemaining}s'**
  String cooldownS(String cooldownSecondsRemaining);

  /// No description provided for @forcer.
  ///
  /// In fr, this message translates to:
  /// **'Forcer'**
  String get forcer;

  /// No description provided for @noCategory.
  ///
  /// In fr, this message translates to:
  /// **'No Category'**
  String get noCategory;

  /// No description provided for @totalSize2.
  ///
  /// In fr, this message translates to:
  /// **'Total Size:'**
  String get totalSize2;

  /// No description provided for @estAcquiredGapsComplement.
  ///
  /// In fr, this message translates to:
  /// **'Est. Acquired / Gaps Complement:'**
  String get estAcquiredGapsComplement;

  /// No description provided for @partCount.
  ///
  /// In fr, this message translates to:
  /// **'Part Count:'**
  String get partCount;

  /// No description provided for @ed2kHash.
  ///
  /// In fr, this message translates to:
  /// **'eD2K Hash'**
  String get ed2kHash;

  /// No description provided for @importSuccessfulTheDownloadIs.
  ///
  /// In fr, this message translates to:
  /// **'Import successful! The download is paused and verified parts have been staged.'**
  String get importSuccessfulTheDownloadIs;

  /// No description provided for @auditReport.
  ///
  /// In fr, this message translates to:
  /// **'Audit Report'**
  String get auditReport;

  /// No description provided for @auditReportCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Audit report copied to clipboard'**
  String get auditReportCopiedToClipboard;

  /// No description provided for @copyReport.
  ///
  /// In fr, this message translates to:
  /// **'Copy Report'**
  String get copyReport;

  /// No description provided for @done.
  ///
  /// In fr, this message translates to:
  /// **'Done'**
  String get done;

  /// No description provided for @theDownloadWillBeStaged.
  ///
  /// In fr, this message translates to:
  /// **'The download will be staged into data/temp/ and initialized in Paused state. L’import ne contacte aucun peer et le téléchargement restera en pause.'**
  String get theDownloadWillBeStaged;

  /// No description provided for @confirmCleanup.
  ///
  /// In fr, this message translates to:
  /// **'Confirm Cleanup'**
  String get confirmCleanup;

  /// No description provided for @thisWillRemoveFixtureFile.
  ///
  /// In fr, this message translates to:
  /// **'This will remove {fixtureCount} fixture file(s) in <workdir>/fixtures/ and clean temporary download part files. Incoming and finished downloads will never be touched.\n\nAre you sure you want to proceed?'**
  String thisWillRemoveFixtureFile(String fixtureCount);

  /// No description provided for @deleteTestData.
  ///
  /// In fr, this message translates to:
  /// **'Delete Test Data'**
  String get deleteTestData;

  /// No description provided for @storageDataInternalStateVs.
  ///
  /// In fr, this message translates to:
  /// **'Storage: data/ (Internal State) vs downloads/ (Completed Files)'**
  String get storageDataInternalStateVs;

  /// No description provided for @openFolder.
  ///
  /// In fr, this message translates to:
  /// **'Open Folder'**
  String get openFolder;

  /// No description provided for @internalDataDbTempPart.
  ///
  /// In fr, this message translates to:
  /// **'• Internal data (DB, temp .part, fixtures): {workdir}'**
  String internalDataDbTempPart(String workdir);

  /// No description provided for @completedDownloads.
  ///
  /// In fr, this message translates to:
  /// **'• Completed downloads: {downloadsDir}'**
  String completedDownloads(String downloadsDir);

  /// No description provided for @cleanTestDataFixturesIncomplete.
  ///
  /// In fr, this message translates to:
  /// **'Clean Test Data (Fixtures & Incomplete Parts)'**
  String get cleanTestDataFixturesIncomplete;

  /// No description provided for @cleanupTestData.
  ///
  /// In fr, this message translates to:
  /// **'Cleanup Test Data'**
  String get cleanupTestData;

  /// No description provided for @connectDownloadToLoopbackSource.
  ///
  /// In fr, this message translates to:
  /// **'Connect Download to Loopback Source'**
  String get connectDownloadToLoopbackSource;

  /// No description provided for @noDownloadsInQueueAdd.
  ///
  /// In fr, this message translates to:
  /// **'No downloads in queue. Add an eD2K link in Step 2.'**
  String get noDownloadsInQueueAdd;

  /// No description provided for @targetDownloadTask.
  ///
  /// In fr, this message translates to:
  /// **'Target Download Task'**
  String get targetDownloadTask;

  /// No description provided for @warningDownloadSize.
  ///
  /// In fr, this message translates to:
  /// **'Warning: Download size ({totalBytes}) does not match fixture ({fixtureSize}).'**
  String warningDownloadSize(String totalBytes, String fixtureSize);

  /// No description provided for @startLoopbackPeerInStep.
  ///
  /// In fr, this message translates to:
  /// **'Start loopback peer in Step 3 before connecting.'**
  String get startLoopbackPeerInStep;

  /// No description provided for @finalFile.
  ///
  /// In fr, this message translates to:
  /// **'Final file: {downloadsDir}\\{fileName}'**
  String finalFile(String fileName, String downloadsDir);

  /// No description provided for @generateLocalTestFixture.
  ///
  /// In fr, this message translates to:
  /// **'Generate Local Test Fixture'**
  String get generateLocalTestFixture;

  /// No description provided for @obtainCanonicalEd2kLinkAdd.
  ///
  /// In fr, this message translates to:
  /// **'Obtain Canonical eD2K Link & Add to Downloads'**
  String get obtainCanonicalEd2kLinkAdd;

  /// No description provided for @copyLink.
  ///
  /// In fr, this message translates to:
  /// **'Copy Link'**
  String get copyLink;

  /// No description provided for @ed2kLinkCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'eD2K Link copied to clipboard'**
  String get ed2kLinkCopiedToClipboard;

  /// No description provided for @downloadTaskCreatedInUi.
  ///
  /// In fr, this message translates to:
  /// **'Download task created in UI'**
  String get downloadTaskCreatedInUi;

  /// No description provided for @completeStep1ToGenerate.
  ///
  /// In fr, this message translates to:
  /// **'Complete Step 1 to generate an eD2K fixture first.'**
  String get completeStep1ToGenerate;

  /// No description provided for @loopbackTestPeer1270.
  ///
  /// In fr, this message translates to:
  /// **'Loopback Test Peer (127.0.0.1)'**
  String get loopbackTestPeer1270;

  /// No description provided for @lastImportReport.
  ///
  /// In fr, this message translates to:
  /// **'Last Import Report'**
  String get lastImportReport;

  /// No description provided for @invalidLinesSkipped.
  ///
  /// In fr, this message translates to:
  /// **'Invalid lines skipped: {invalidLines}'**
  String invalidLinesSkipped(String invalidLines);

  /// No description provided for @rangesLoaded.
  ///
  /// In fr, this message translates to:
  /// **'{rangeCount} ranges loaded'**
  String rangesLoaded(String rangeCount);

  /// No description provided for @totalBlockedConnectionAttempts.
  ///
  /// In fr, this message translates to:
  /// **'Total blocked connection attempts: {hitsCount}'**
  String totalBlockedConnectionAttempts(String hitsCount);

  /// No description provided for @importIpfilterDat.
  ///
  /// In fr, this message translates to:
  /// **'Import ipfilter.dat'**
  String get importIpfilterDat;

  /// No description provided for @loadALocalPeerguardianEmule.
  ///
  /// In fr, this message translates to:
  /// **'Load a local PeerGuardian / eMule formatted ipfilter.dat file. Remote URL download is disallowed for safety.'**
  String get loadALocalPeerguardianEmule;

  /// No description provided for @import.
  ///
  /// In fr, this message translates to:
  /// **'Import'**
  String get import;

  /// No description provided for @clear.
  ///
  /// In fr, this message translates to:
  /// **'Clear'**
  String get clear;

  /// No description provided for @noNetworkEventsRecordedYet.
  ///
  /// In fr, this message translates to:
  /// **'No network events recorded yet.'**
  String get noNetworkEventsRecordedYet;

  /// No description provided for @activeConnectivityProfile.
  ///
  /// In fr, this message translates to:
  /// **'Active Connectivity Profile'**
  String get activeConnectivityProfile;

  /// No description provided for @activeConnectionsLimits.
  ///
  /// In fr, this message translates to:
  /// **'Active Connections & Limits'**
  String get activeConnectionsLimits;

  /// No description provided for @peerSockets.
  ///
  /// In fr, this message translates to:
  /// **'Peer Sockets'**
  String get peerSockets;

  /// No description provided for @serverSockets.
  ///
  /// In fr, this message translates to:
  /// **'Server Sockets'**
  String get serverSockets;

  /// No description provided for @listeners.
  ///
  /// In fr, this message translates to:
  /// **'Listeners'**
  String get listeners;

  /// No description provided for @connectTimeout.
  ///
  /// In fr, this message translates to:
  /// **'Connect Timeout'**
  String get connectTimeout;

  /// No description provided for @maxFrameSize.
  ///
  /// In fr, this message translates to:
  /// **'Max Frame Size'**
  String get maxFrameSize;

  /// No description provided for @maxBufferMemory.
  ///
  /// In fr, this message translates to:
  /// **'Max Buffer Memory'**
  String get maxBufferMemory;

  /// No description provided for @status2.
  ///
  /// In fr, this message translates to:
  /// **'Status: {label}'**
  String status2(String label);

  /// No description provided for @reEnable.
  ///
  /// In fr, this message translates to:
  /// **'Re-enable'**
  String get reEnable;

  /// No description provided for @disconnectAll.
  ///
  /// In fr, this message translates to:
  /// **'Disconnect All'**
  String get disconnectAll;

  /// No description provided for @bandwidthTokenBucketQuotas.
  ///
  /// In fr, this message translates to:
  /// **'Bandwidth & Token Bucket Quotas'**
  String get bandwidthTokenBucketQuotas;

  /// No description provided for @limitsAreEnforcedAsynchronouslyUsing.
  ///
  /// In fr, this message translates to:
  /// **'Limits are enforced asynchronously using Token Buckets. 0 means unlimited.'**
  String get limitsAreEnforcedAsynchronouslyUsing;

  /// No description provided for @downloadLimitKbS0.
  ///
  /// In fr, this message translates to:
  /// **'Download Limit (KB/s) [0 = unlimited]'**
  String get downloadLimitKbS0;

  /// No description provided for @uploadLimitKbS0.
  ///
  /// In fr, this message translates to:
  /// **'Upload Limit (KB/s) [0 = unlimited]'**
  String get uploadLimitKbS0;

  /// No description provided for @maxNewConnectionsMinute.
  ///
  /// In fr, this message translates to:
  /// **'Max New Connections / Minute'**
  String get maxNewConnectionsMinute;

  /// No description provided for @saveLimits.
  ///
  /// In fr, this message translates to:
  /// **'Save Limits'**
  String get saveLimits;

  /// No description provided for @inspectDetailsPartMap.
  ///
  /// In fr, this message translates to:
  /// **'Inspect Details & Part Map'**
  String get inspectDetailsPartMap;

  /// No description provided for @remove2.
  ///
  /// In fr, this message translates to:
  /// **'Remove'**
  String get remove2;

  /// No description provided for @removeFromDownloads.
  ///
  /// In fr, this message translates to:
  /// **'Remove \"{fileName}\" from downloads?'**
  String removeFromDownloads(String fileName);

  /// No description provided for @deleteDownloadedPartTempFiles.
  ///
  /// In fr, this message translates to:
  /// **'Delete downloaded part/temp files from disk'**
  String get deleteDownloadedPartTempFiles;

  /// No description provided for @id.
  ///
  /// In fr, this message translates to:
  /// **'ID: {id}'**
  String id(String id);

  /// No description provided for @src.
  ///
  /// In fr, this message translates to:
  /// **'{sourceCount} src'**
  String src(String sourceCount);

  /// No description provided for @fullId.
  ///
  /// In fr, this message translates to:
  /// **'Full ID: {id}'**
  String fullId(String id);

  /// No description provided for @downloadIdCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Download ID copied to clipboard'**
  String get downloadIdCopiedToClipboard;

  /// No description provided for @sources2.
  ///
  /// In fr, this message translates to:
  /// **'{sourceCount} sources'**
  String sources2(String sourceCount);

  /// No description provided for @downloadSettingsStorage.
  ///
  /// In fr, this message translates to:
  /// **'Download Settings & Storage'**
  String get downloadSettingsStorage;

  /// No description provided for @globalDefaultDownloadDirectory.
  ///
  /// In fr, this message translates to:
  /// **'Global Default Download Directory'**
  String get globalDefaultDownloadDirectory;

  /// No description provided for @thisPathIsUsedAs.
  ///
  /// In fr, this message translates to:
  /// **'This path is used as the default target destination when adding new downloads. Temporary .part files remain strictly confined in data/temp/ until completed.'**
  String get thisPathIsUsedAs;

  /// No description provided for @destinationDirectoryPath.
  ///
  /// In fr, this message translates to:
  /// **'Destination Directory Path'**
  String get destinationDirectoryPath;

  /// No description provided for @eGCDownloadsOr.
  ///
  /// In fr, this message translates to:
  /// **'e.g. C:\\Downloads or downloads'**
  String get eGCDownloadsOr;

  /// No description provided for @save.
  ///
  /// In fr, this message translates to:
  /// **'Save'**
  String get save;

  /// No description provided for @resetToPortableDefault.
  ///
  /// In fr, this message translates to:
  /// **'Reset to Portable Default'**
  String get resetToPortableDefault;

  /// No description provided for @modeReseauTraverseeBoxUpnp.
  ///
  /// In fr, this message translates to:
  /// **'Mode Réseau & Traversée Box (UPnP)'**
  String get modeReseauTraverseeBoxUpnp;

  /// No description provided for @basculeEntreLaboratoireLocalEt.
  ///
  /// In fr, this message translates to:
  /// **'Bascule entre laboratoire local et réseau public mondial.'**
  String get basculeEntreLaboratoireLocalEt;

  /// No description provided for @configurer.
  ///
  /// In fr, this message translates to:
  /// **'Configurer'**
  String get configurer;

  /// No description provided for @historyRetentionPolicy.
  ///
  /// In fr, this message translates to:
  /// **'History Retention Policy'**
  String get historyRetentionPolicy;

  /// No description provided for @historicalTransitionEventsOlderThan.
  ///
  /// In fr, this message translates to:
  /// **'Historical transition events older than the retention period are automatically pruned on daemon startup.'**
  String get historicalTransitionEventsOlderThan;

  /// No description provided for @keepHistoryFor.
  ///
  /// In fr, this message translates to:
  /// **'Keep history for: '**
  String get keepHistoryFor;

  /// No description provided for @indefinitelyNoPruning.
  ///
  /// In fr, this message translates to:
  /// **'Indefinitely (No pruning)'**
  String get indefinitelyNoPruning;

  /// No description provided for @delete.
  ///
  /// In fr, this message translates to:
  /// **'Delete \"{name}\"?'**
  String delete(String name);

  /// No description provided for @deletingThisCategoryWillMove.
  ///
  /// In fr, this message translates to:
  /// **'Deleting this category will move all associated downloads to \"Sans catégorie\" (Uncategorized).\n\nNo downloads or files will be deleted.'**
  String get deletingThisCategoryWillMove;

  /// No description provided for @deleteCategory.
  ///
  /// In fr, this message translates to:
  /// **'Delete Category'**
  String get deleteCategory;

  /// No description provided for @categoryOptions.
  ///
  /// In fr, this message translates to:
  /// **'Category Options'**
  String get categoryOptions;

  /// No description provided for @editCategory.
  ///
  /// In fr, this message translates to:
  /// **'Edit Category'**
  String get editCategory;

  /// No description provided for @clearDownloadHistory.
  ///
  /// In fr, this message translates to:
  /// **'Clear Download History?'**
  String get clearDownloadHistory;

  /// No description provided for @areYouSureYouWant.
  ///
  /// In fr, this message translates to:
  /// **'Are you sure you want to permanently delete all download history and statistics records?\n\nThis action is irreversible.'**
  String get areYouSureYouWant;

  /// No description provided for @clearAllHistory.
  ///
  /// In fr, this message translates to:
  /// **'Clear All History'**
  String get clearAllHistory;

  /// No description provided for @noDownloadHistoryRecordedYet.
  ///
  /// In fr, this message translates to:
  /// **'No download history recorded yet'**
  String get noDownloadHistoryRecordedYet;

  /// No description provided for @completedResumedFailedAndRemoved.
  ///
  /// In fr, this message translates to:
  /// **'Completed, resumed, failed, and removed transfer events will appear here.'**
  String get completedResumedFailedAndRemoved;

  /// No description provided for @eventDetail.
  ///
  /// In fr, this message translates to:
  /// **'Event Detail: {fileName}'**
  String eventDetail(String fileName);

  /// No description provided for @viewDetails.
  ///
  /// In fr, this message translates to:
  /// **'View details'**
  String get viewDetails;

  /// No description provided for @of2.
  ///
  /// In fr, this message translates to:
  /// **'{transferredBytes} of {totalBytes}'**
  String of2(String transferredBytes, String totalBytes);

  /// No description provided for @hash2.
  ///
  /// In fr, this message translates to:
  /// **'Hash: {ed2kHash}'**
  String hash2(String ed2kHash);

  /// No description provided for @of3.
  ///
  /// In fr, this message translates to:
  /// **'of {totalBytes}'**
  String of3(String totalBytes);

  /// No description provided for @res.
  ///
  /// In fr, this message translates to:
  /// **'{resumeCount} res. / {connectionAttempts} att.'**
  String res(String resumeCount, String connectionAttempts);

  /// No description provided for @exportHistory.
  ///
  /// In fr, this message translates to:
  /// **'Export History ({toUpperCase})'**
  String exportHistory(String toUpperCase);

  /// No description provided for @reportIsSanitizedAllLocal.
  ///
  /// In fr, this message translates to:
  /// **'Report is sanitized: all local usernames and filesystem paths are redacted.'**
  String get reportIsSanitizedAllLocal;

  /// No description provided for @copyToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Copy to Clipboard'**
  String get copyToClipboard;

  /// No description provided for @exportCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Export copied to clipboard'**
  String get exportCopiedToClipboard;

  /// No description provided for @filterByEvent.
  ///
  /// In fr, this message translates to:
  /// **'Filter by Event:'**
  String get filterByEvent;

  /// No description provided for @allEvents.
  ///
  /// In fr, this message translates to:
  /// **'All Events'**
  String get allEvents;

  /// No description provided for @completed2.
  ///
  /// In fr, this message translates to:
  /// **'Completed'**
  String get completed2;

  /// No description provided for @failed.
  ///
  /// In fr, this message translates to:
  /// **'Failed'**
  String get failed;

  /// No description provided for @resumed.
  ///
  /// In fr, this message translates to:
  /// **'Resumed'**
  String get resumed;

  /// No description provided for @verifying.
  ///
  /// In fr, this message translates to:
  /// **'Verifying'**
  String get verifying;

  /// No description provided for @removed.
  ///
  /// In fr, this message translates to:
  /// **'Removed'**
  String get removed;

  /// No description provided for @recordS.
  ///
  /// In fr, this message translates to:
  /// **'{length} record(s)'**
  String recordS(String length);

  /// No description provided for @totalCompleted.
  ///
  /// In fr, this message translates to:
  /// **'Total Completed'**
  String get totalCompleted;

  /// No description provided for @activityWindows.
  ///
  /// In fr, this message translates to:
  /// **'Activity Windows'**
  String get activityWindows;

  /// No description provided for @resumesRetries.
  ///
  /// In fr, this message translates to:
  /// **'Resumes & Retries'**
  String get resumesRetries;

  /// No description provided for @failuresLogged.
  ///
  /// In fr, this message translates to:
  /// **'{totalFailedCount} failures logged'**
  String failuresLogged(String totalFailedCount);

  /// No description provided for @integrityHealth.
  ///
  /// In fr, this message translates to:
  /// **'Integrity Health'**
  String get integrityHealth;

  /// No description provided for @historyLocalStatistics.
  ///
  /// In fr, this message translates to:
  /// **'History & Local Statistics'**
  String get historyLocalStatistics;

  /// No description provided for @exportHistoryReport.
  ///
  /// In fr, this message translates to:
  /// **'Export history report'**
  String get exportHistoryReport;

  /// No description provided for @exportAsJsonSanitized.
  ///
  /// In fr, this message translates to:
  /// **'Export as JSON (Sanitized)'**
  String get exportAsJsonSanitized;

  /// No description provided for @exportAsCsvSanitized.
  ///
  /// In fr, this message translates to:
  /// **'Export as CSV (Sanitized)'**
  String get exportAsCsvSanitized;

  /// No description provided for @exportReport.
  ///
  /// In fr, this message translates to:
  /// **'Export Report'**
  String get exportReport;

  /// No description provided for @clearHistory.
  ///
  /// In fr, this message translates to:
  /// **'Clear History'**
  String get clearHistory;

  /// No description provided for @refresh.
  ///
  /// In fr, this message translates to:
  /// **'Refresh'**
  String get refresh;

  /// No description provided for @amorcerLeReseauKadBootstrap.
  ///
  /// In fr, this message translates to:
  /// **'Amorcer le réseau Kad (Bootstrap)'**
  String get amorcerLeReseauKadBootstrap;

  /// No description provided for @adresseIp.
  ///
  /// In fr, this message translates to:
  /// **'Adresse IP'**
  String get adresseIp;

  /// No description provided for @portUdp.
  ///
  /// In fr, this message translates to:
  /// **'Port UDP'**
  String get portUdp;

  /// No description provided for @annuler.
  ///
  /// In fr, this message translates to:
  /// **'Annuler'**
  String get annuler;

  /// No description provided for @lancerLAmorcage.
  ///
  /// In fr, this message translates to:
  /// **'Lancer l\'amorçage'**
  String get lancerLAmorcage;

  /// No description provided for @amorcerKadDepuisUneUrl.
  ///
  /// In fr, this message translates to:
  /// **'Amorcer Kad depuis une URL'**
  String get amorcerKadDepuisUneUrl;

  /// No description provided for @telechargezUnFichierNodesDat.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargez un fichier nodes.dat distant pour amorcer votre table de routage Kad Kademlia (ex: https://upd.emule-security.org/nodes.dat) :'**
  String get telechargezUnFichierNodesDat;

  /// No description provided for @urlDuNodesDat.
  ///
  /// In fr, this message translates to:
  /// **'URL du nodes.dat'**
  String get urlDuNodesDat;

  /// No description provided for @noteSeulsLesProtocolesHttp.
  ///
  /// In fr, this message translates to:
  /// **'Note : Seuls les protocoles HTTP/HTTPS publics sont acceptés. Le trafic vers les réseaux locaux (RFC1918) et loopback est rejeté par sécurité (anti-SSRF).'**
  String get noteSeulsLesProtocolesHttp;

  /// No description provided for @telechargerEtAmorcer.
  ///
  /// In fr, this message translates to:
  /// **'Télécharger et Amorcer'**
  String get telechargerEtAmorcer;

  /// No description provided for @effacerLaTableDeRoutage.
  ///
  /// In fr, this message translates to:
  /// **'Effacer la table de routage Kad ?'**
  String get effacerLaTableDeRoutage;

  /// No description provided for @tousLesContactsKadEn.
  ///
  /// In fr, this message translates to:
  /// **'Tous les contacts Kad en mémoire et dans la base SQLite locale seront supprimés.\nAucun paquet réseau ne sera envoyé.'**
  String get tousLesContactsKadEn;

  /// No description provided for @effacer.
  ///
  /// In fr, this message translates to:
  /// **'Effacer'**
  String get effacer;

  /// No description provided for @kadId128BitXor.
  ///
  /// In fr, this message translates to:
  /// **'Kad ID (128-bit XOR)'**
  String get kadId128BitXor;

  /// No description provided for @adresseIpPorts.
  ///
  /// In fr, this message translates to:
  /// **'Adresse IP & Ports'**
  String get adresseIpPorts;

  /// No description provided for @version.
  ///
  /// In fr, this message translates to:
  /// **'Version'**
  String get version;

  /// No description provided for @statut.
  ///
  /// In fr, this message translates to:
  /// **'Statut'**
  String get statut;

  /// No description provided for @echecs.
  ///
  /// In fr, this message translates to:
  /// **'Échecs'**
  String get echecs;

  /// No description provided for @derniereVue.
  ///
  /// In fr, this message translates to:
  /// **'Dernière vue'**
  String get derniereVue;

  /// No description provided for @actions.
  ///
  /// In fr, this message translates to:
  /// **'Actions'**
  String get actions;

  /// No description provided for @v.
  ///
  /// In fr, this message translates to:
  /// **'v{version}'**
  String v(String version);

  /// No description provided for @envoyerKadPing0x60.
  ///
  /// In fr, this message translates to:
  /// **'Envoyer Kad Ping (0x60)'**
  String get envoyerKadPing0x60;

  /// No description provided for @aucunContactKad.
  ///
  /// In fr, this message translates to:
  /// **'Aucun contact Kad'**
  String get aucunContactKad;

  /// No description provided for @importezUnFichierNodesDat.
  ///
  /// In fr, this message translates to:
  /// **'Importez un fichier nodes.dat pour amorcer la table de routage K-buckets.'**
  String get importezUnFichierNodesDat;

  /// No description provided for @importerNodesDat.
  ///
  /// In fr, this message translates to:
  /// **'Importer nodes.dat'**
  String get importerNodesDat;

  /// No description provided for @filtrerParKadIdIp.
  ///
  /// In fr, this message translates to:
  /// **'Filtrer par Kad ID, IP ou Port...'**
  String get filtrerParKadIdIp;

  /// No description provided for @effacerLaTable.
  ///
  /// In fr, this message translates to:
  /// **'Effacer la table'**
  String get effacerLaTable;

  /// No description provided for @importerUnFichierNodesDat.
  ///
  /// In fr, this message translates to:
  /// **'Importer un fichier nodes.dat'**
  String get importerUnFichierNodesDat;

  /// No description provided for @specifiezLeCheminAbsoluVers.
  ///
  /// In fr, this message translates to:
  /// **'Spécifiez le chemin absolu vers un fichier nodes.dat (v0, v1 ou v2).'**
  String get specifiezLeCheminAbsoluVers;

  /// No description provided for @inspecter.
  ///
  /// In fr, this message translates to:
  /// **'Inspecter'**
  String get inspecter;

  /// No description provided for @formatNodesDatVDetecte.
  ///
  /// In fr, this message translates to:
  /// **'Format nodes.dat v{version} détecté'**
  String formatNodesDatVDetecte(String version);

  /// No description provided for @contactsDeclares.
  ///
  /// In fr, this message translates to:
  /// **'• Contacts déclarés : {declaredCount}'**
  String contactsDeclares(String declaredCount);

  /// No description provided for @contactsValidesAdmissibles.
  ///
  /// In fr, this message translates to:
  /// **'• Contacts valides & admissibles : {validContactsCount}'**
  String contactsValidesAdmissibles(String validContactsCount);

  /// No description provided for @rejetesParLaPolitiqueReseau.
  ///
  /// In fr, this message translates to:
  /// **'• Rejetés par la politique réseau : {rejectedByPolicyCount}'**
  String rejetesParLaPolitiqueReseau(String rejectedByPolicyCount);

  /// No description provided for @rejetesParLeFiltreIp.
  ///
  /// In fr, this message translates to:
  /// **'• Rejetés par le filtre IP : {rejectedByFilterCount}'**
  String rejetesParLeFiltreIp(String rejectedByFilterCount);

  /// No description provided for @importerLesContacts.
  ///
  /// In fr, this message translates to:
  /// **'Importer les contacts'**
  String get importerLesContacts;

  /// No description provided for @idKadLocal.
  ///
  /// In fr, this message translates to:
  /// **'ID Kad Local :'**
  String get idKadLocal;

  /// No description provided for @arreterUdp.
  ///
  /// In fr, this message translates to:
  /// **'Arrêter UDP'**
  String get arreterUdp;

  /// No description provided for @socketUdp.
  ///
  /// In fr, this message translates to:
  /// **'Socket UDP'**
  String get socketUdp;

  /// No description provided for @paquetsTxRx.
  ///
  /// In fr, this message translates to:
  /// **'Paquets TX / RX'**
  String get paquetsTxRx;

  /// No description provided for @contactsActifs.
  ///
  /// In fr, this message translates to:
  /// **'Contacts actifs'**
  String get contactsActifs;

  /// No description provided for @bucketsActifs.
  ///
  /// In fr, this message translates to:
  /// **'Buckets actifs'**
  String get bucketsActifs;

  /// No description provided for @contactsEnBase.
  ///
  /// In fr, this message translates to:
  /// **'Contacts en base'**
  String get contactsEnBase;

  /// No description provided for @profil.
  ///
  /// In fr, this message translates to:
  /// **'Profil'**
  String get profil;

  /// No description provided for @reseauKadDhtKademlia.
  ///
  /// In fr, this message translates to:
  /// **'Réseau Kad DHT (Kademlia)'**
  String get reseauKadDhtKademlia;

  /// No description provided for @actualiser.
  ///
  /// In fr, this message translates to:
  /// **'Actualiser'**
  String get actualiser;

  /// No description provided for @amorcerBootstrap.
  ///
  /// In fr, this message translates to:
  /// **'Amorcer (Bootstrap)'**
  String get amorcerBootstrap;

  /// No description provided for @amorcerDepuisUrl.
  ///
  /// In fr, this message translates to:
  /// **'Amorcer depuis URL'**
  String get amorcerDepuisUrl;

  /// No description provided for @allerAKad.
  ///
  /// In fr, this message translates to:
  /// **'Aller à Kad'**
  String get allerAKad;

  /// No description provided for @allerAuxServeurs.
  ///
  /// In fr, this message translates to:
  /// **'Aller aux Serveurs'**
  String get allerAuxServeurs;

  /// No description provided for @nonConnecteAUnServeur.
  ///
  /// In fr, this message translates to:
  /// **'Non connecté à un serveur eD2K.'**
  String get nonConnecteAUnServeur;

  /// No description provided for @connectezVousDAbordPour.
  ///
  /// In fr, this message translates to:
  /// **'Connectez-vous d\'abord pour effectuer une recherche.'**
  String get connectezVousDAbordPour;

  /// No description provided for @vousNEtesActuellementPas.
  ///
  /// In fr, this message translates to:
  /// **'Vous n\'êtes actuellement pas connecté à un serveur eD2K. Connectez-vous d\'abord pour effectuer une recherche.'**
  String get vousNEtesActuellementPas;

  /// No description provided for @extensionExIsoZipAvi.
  ///
  /// In fr, this message translates to:
  /// **'Extension (ex: iso, zip, avi)'**
  String get extensionExIsoZipAvi;

  /// No description provided for @tailleMinMo.
  ///
  /// In fr, this message translates to:
  /// **'Taille Min (Mo)'**
  String get tailleMinMo;

  /// No description provided for @tailleMaxMo.
  ///
  /// In fr, this message translates to:
  /// **'Taille Max (Mo)'**
  String get tailleMaxMo;

  /// No description provided for @effacerFiltres.
  ///
  /// In fr, this message translates to:
  /// **'Effacer filtres'**
  String get effacerFiltres;

  /// No description provided for @serveurEd2k.
  ///
  /// In fr, this message translates to:
  /// **'Serveur eD2K'**
  String get serveurEd2k;

  /// No description provided for @globalUdp.
  ///
  /// In fr, this message translates to:
  /// **'Global UDP'**
  String get globalUdp;

  /// No description provided for @nomDuFichier.
  ///
  /// In fr, this message translates to:
  /// **'Nom du fichier'**
  String get nomDuFichier;

  /// No description provided for @taille.
  ///
  /// In fr, this message translates to:
  /// **'Taille'**
  String get taille;

  /// No description provided for @hachageEd2k.
  ///
  /// In fr, this message translates to:
  /// **'Hachage eD2K'**
  String get hachageEd2k;

  /// No description provided for @cliquerPourCopierLeHash.
  ///
  /// In fr, this message translates to:
  /// **'Cliquer pour copier le hash : {fileHash}'**
  String cliquerPourCopierLeHash(String fileHash);

  /// No description provided for @hachageCopie.
  ///
  /// In fr, this message translates to:
  /// **'Hachage copié : {fileHash}'**
  String hachageCopie(String fileHash);

  /// No description provided for @termine.
  ///
  /// In fr, this message translates to:
  /// **'Terminé'**
  String get termine;

  /// No description provided for @telecharger.
  ///
  /// In fr, this message translates to:
  /// **'Télécharger'**
  String get telecharger;

  /// No description provided for @demanderSourcesAuServeur.
  ///
  /// In fr, this message translates to:
  /// **'Demander sources au serveur'**
  String get demanderSourcesAuServeur;

  /// No description provided for @copierLienEd2k.
  ///
  /// In fr, this message translates to:
  /// **'Copier lien eD2K'**
  String get copierLienEd2k;

  /// No description provided for @lienEd2kCopieDansLe.
  ///
  /// In fr, this message translates to:
  /// **'Lien eD2K copié dans le presse-papier'**
  String get lienEd2kCopieDansLe;

  /// No description provided for @afficher.
  ///
  /// In fr, this message translates to:
  /// **'Afficher'**
  String get afficher;

  /// No description provided for @rechercheEnCoursSur.
  ///
  /// In fr, this message translates to:
  /// **'Recherche en cours sur {targetLabel}...'**
  String rechercheEnCoursSur(String targetLabel);

  /// No description provided for @rechercheSurLeReseauEd2k.
  ///
  /// In fr, this message translates to:
  /// **'Recherche sur le Réseau eD2K'**
  String get rechercheSurLeReseauEd2k;

  /// No description provided for @resultatS.
  ///
  /// In fr, this message translates to:
  /// **'{resultCount} résultat(s)'**
  String resultatS(String resultCount);

  /// No description provided for @effacerLesResultats.
  ///
  /// In fr, this message translates to:
  /// **'Effacer les résultats'**
  String get effacerLesResultats;

  /// No description provided for @effacerLesResultats2.
  ///
  /// In fr, this message translates to:
  /// **'Effacer les résultats ({resultCount})'**
  String effacerLesResultats2(String resultCount);

  /// No description provided for @effacerTousLesServeurs.
  ///
  /// In fr, this message translates to:
  /// **'Effacer tous les serveurs ?'**
  String get effacerTousLesServeurs;

  /// No description provided for @cetteOperationSupprimeLaListe.
  ///
  /// In fr, this message translates to:
  /// **'Cette operation supprime la liste des serveurs enregistres dans votre base locale.'**
  String get cetteOperationSupprimeLaListe;

  /// No description provided for @aucunServeurEd2kEnregistre.
  ///
  /// In fr, this message translates to:
  /// **'Aucun serveur eD2K enregistre'**
  String get aucunServeurEd2kEnregistre;

  /// No description provided for @importezUnFichierServerMet.
  ///
  /// In fr, this message translates to:
  /// **'Importez un fichier server.met local pour inspecter et gerer vos serveurs.'**
  String get importezUnFichierServerMet;

  /// No description provided for @selectionnerServerMet.
  ///
  /// In fr, this message translates to:
  /// **'Selectionner server.met'**
  String get selectionnerServerMet;

  /// No description provided for @parDefautEmuleSionEst.
  ///
  /// In fr, this message translates to:
  /// **'Par défaut, Emule-Sion est confiné en boucle locale (LocalLab). Basculez sur PublicEd2kPreview pour autoriser les serveurs publics.'**
  String get parDefautEmuleSionEst;

  /// No description provided for @modifierLeProfil.
  ///
  /// In fr, this message translates to:
  /// **'Modifier le profil'**
  String get modifierLeProfil;

  /// No description provided for @rechercherUnServeurNomIp.
  ///
  /// In fr, this message translates to:
  /// **'Rechercher un serveur (nom, IP, port)...'**
  String get rechercherUnServeurNomIp;

  /// No description provided for @toutEffacer.
  ///
  /// In fr, this message translates to:
  /// **'Tout effacer'**
  String get toutEffacer;

  /// No description provided for @securiteProfil.
  ///
  /// In fr, this message translates to:
  /// **'Sécurité / Profil'**
  String get securiteProfil;

  /// No description provided for @etatFiltre.
  ///
  /// In fr, this message translates to:
  /// **'Etat / Filtre'**
  String get etatFiltre;

  /// No description provided for @nomDuServeur.
  ///
  /// In fr, this message translates to:
  /// **'Nom du Serveur'**
  String get nomDuServeur;

  /// No description provided for @ipPort.
  ///
  /// In fr, this message translates to:
  /// **'IP : Port'**
  String get ipPort;

  /// No description provided for @ping.
  ///
  /// In fr, this message translates to:
  /// **'Ping'**
  String get ping;

  /// No description provided for @description.
  ///
  /// In fr, this message translates to:
  /// **'Description'**
  String get description;

  /// No description provided for @preference.
  ///
  /// In fr, this message translates to:
  /// **'Preference'**
  String get preference;

  /// No description provided for @seConnecter.
  ///
  /// In fr, this message translates to:
  /// **'Se connecter'**
  String get seConnecter;

  /// No description provided for @supprimer.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer'**
  String get supprimer;

  /// No description provided for @gestionLocaleDesServeursEd2k.
  ///
  /// In fr, this message translates to:
  /// **'Gestion Locale des Serveurs eD2K'**
  String get gestionLocaleDesServeursEd2k;

  /// No description provided for @importerServerMet.
  ///
  /// In fr, this message translates to:
  /// **'Importer server.met'**
  String get importerServerMet;

  /// No description provided for @mettreAJourDepuisUrl.
  ///
  /// In fr, this message translates to:
  /// **'Mettre à jour depuis URL'**
  String get mettreAJourDepuisUrl;

  /// No description provided for @mettreAJourDepuisUne.
  ///
  /// In fr, this message translates to:
  /// **'Mettre à jour depuis une URL'**
  String get mettreAJourDepuisUne;

  /// No description provided for @indiquezLAdresseHttpOu.
  ///
  /// In fr, this message translates to:
  /// **'Indiquez l\'adresse HTTP ou HTTPS d\'un fichier server.met distant pour mettre à jour automatiquement votre annuaire de serveurs :'**
  String get indiquezLAdresseHttpOu;

  /// No description provided for @urlDuServerMet.
  ///
  /// In fr, this message translates to:
  /// **'URL du server.met'**
  String get urlDuServerMet;

  /// No description provided for @noteLesAdressesIpDe.
  ///
  /// In fr, this message translates to:
  /// **'Note : Les adresses IP de réseaux privés (LAN/RFC1918) et loopback sont rejetées par sécurité (anti-SSRF).'**
  String get noteLesAdressesIpDe;

  /// No description provided for @telechargerEtMettreAJour.
  ///
  /// In fr, this message translates to:
  /// **'Télécharger et Mettre à jour'**
  String get telechargerEtMettreAJour;

  /// No description provided for @ms.
  ///
  /// In fr, this message translates to:
  /// **'{ping} ms'**
  String ms(String ping);

  /// No description provided for @v2.
  ///
  /// In fr, this message translates to:
  /// **'v{version}'**
  String v2(String version);

  /// No description provided for @importerUnFichierServerMet.
  ///
  /// In fr, this message translates to:
  /// **'Importer un fichier server.met local'**
  String get importerUnFichierServerMet;

  /// No description provided for @indiquezLeCheminAbsoluVers.
  ///
  /// In fr, this message translates to:
  /// **'Indiquez le chemin absolu vers votre fichier server.met local :'**
  String get indiquezLeCheminAbsoluVers;

  /// No description provided for @cheminDuFichierServerMet.
  ///
  /// In fr, this message translates to:
  /// **'Chemin du fichier server.met'**
  String get cheminDuFichierServerMet;

  /// No description provided for @apercuDuFichierServerMet.
  ///
  /// In fr, this message translates to:
  /// **'Apercu du fichier server.met'**
  String get apercuDuFichierServerMet;

  /// No description provided for @fichier.
  ///
  /// In fr, this message translates to:
  /// **'Fichier : {filePath}'**
  String fichier(String filePath);

  /// No description provided for @total.
  ///
  /// In fr, this message translates to:
  /// **'Total'**
  String get total;

  /// No description provided for @autorises.
  ///
  /// In fr, this message translates to:
  /// **'Autorises'**
  String get autorises;

  /// No description provided for @bloquesFiltreIp.
  ///
  /// In fr, this message translates to:
  /// **'Bloques (Filtre IP)'**
  String get bloquesFiltreIp;

  /// No description provided for @lesServeursMarquesEnRouge.
  ///
  /// In fr, this message translates to:
  /// **'Les serveurs marques en rouge correspondent a des plages de votre filtre IP local. Ils seront importes avec le statut bloque.'**
  String get lesServeursMarquesEnRouge;

  /// No description provided for @bloque.
  ///
  /// In fr, this message translates to:
  /// **'Bloque'**
  String get bloque;

  /// No description provided for @autorise.
  ///
  /// In fr, this message translates to:
  /// **'Autorise'**
  String get autorise;

  /// No description provided for @confirmerImportation.
  ///
  /// In fr, this message translates to:
  /// **'Confirmer importation'**
  String get confirmerImportation;

  /// No description provided for @aucunServeurConnecte.
  ///
  /// In fr, this message translates to:
  /// **'Aucun serveur connecté'**
  String get aucunServeurConnecte;

  /// No description provided for @utilisateurs.
  ///
  /// In fr, this message translates to:
  /// **'{currentUsers} utilisateurs • {currentFiles} fichiers'**
  String utilisateurs(String currentUsers, String currentFiles);

  /// No description provided for @garantieDeSecuriteAntiSsrf.
  ///
  /// In fr, this message translates to:
  /// **'Garantie de Sécurité Anti-SSRF'**
  String get garantieDeSecuriteAntiSsrf;

  /// No description provided for @memeEnModePublicLa.
  ///
  /// In fr, this message translates to:
  /// **'Même en mode public, la politique réseau d\'Emule-Sion bloque rigoureusement tout trafic sortant vers les adresses de votre réseau local privé (192.168.x.x, 10.x.x.x, 172.16-31.x.x) pour protéger vos équipements domestiques.'**
  String get memeEnModePublicLa;

  /// No description provided for @fermer.
  ///
  /// In fr, this message translates to:
  /// **'Fermer'**
  String get fermer;

  /// No description provided for @revenirEnModeConfineLab.
  ///
  /// In fr, this message translates to:
  /// **'Revenir en Mode Confiné (Lab 127.0.0.1)'**
  String get revenirEnModeConfineLab;

  /// No description provided for @activerLeModeReseauPublic.
  ///
  /// In fr, this message translates to:
  /// **'Activer le Mode Réseau Public (WAN)'**
  String get activerLeModeReseauPublic;

  /// No description provided for @profilReseauActuel.
  ///
  /// In fr, this message translates to:
  /// **'Profil Réseau Actuel : '**
  String get profilReseauActuel;

  /// No description provided for @statutUpnpIgdBoxRouteur.
  ///
  /// In fr, this message translates to:
  /// **'Statut UPnP IGD (Box / Routeur NAT)'**
  String get statutUpnpIgdBoxRouteur;

  /// No description provided for @passerelleBoxDetectee.
  ///
  /// In fr, this message translates to:
  /// **'Passerelle / Box détectée :'**
  String get passerelleBoxDetectee;

  /// No description provided for @adresseIpExterneWan.
  ///
  /// In fr, this message translates to:
  /// **'Adresse IP Externe (WAN) :'**
  String get adresseIpExterneWan;

  /// No description provided for @portTcpEd2k.
  ///
  /// In fr, this message translates to:
  /// **'Port TCP eD2K ({tcpPort}) :'**
  String portTcpEd2k(String tcpPort);

  /// No description provided for @portUdpKad.
  ///
  /// In fr, this message translates to:
  /// **'Port UDP Kad ({udpPort}) :'**
  String portUdpKad(String udpPort);

  /// No description provided for @testerRafraichirUpnp.
  ///
  /// In fr, this message translates to:
  /// **'Tester / Rafraîchir UPnP'**
  String get testerRafraichirUpnp;

  /// No description provided for @sevenDays.
  ///
  /// In fr, this message translates to:
  /// **'7 Days'**
  String get sevenDays;

  /// No description provided for @thirtyDaysDefault.
  ///
  /// In fr, this message translates to:
  /// **'30 Days (Default)'**
  String get thirtyDaysDefault;

  /// No description provided for @ninetyDays.
  ///
  /// In fr, this message translates to:
  /// **'90 Days'**
  String get ninetyDays;

  /// No description provided for @twentyFourHours.
  ///
  /// In fr, this message translates to:
  /// **'24h: {last24hCompletedCount} | 7d: {last7dCompletedCount}'**
  String twentyFourHours(
    String last24hCompletedCount,
    String last7dCompletedCount,
  );

  /// No description provided for @receivedOfTotal.
  ///
  /// In fr, this message translates to:
  /// **'{receivedBytes} of {totalBytes} ({progress})'**
  String receivedOfTotal(
    String receivedBytes,
    String totalBytes,
    String progress,
  );

  /// No description provided for @serverEnable.
  ///
  /// In fr, this message translates to:
  /// **'Activer'**
  String get serverEnable;

  /// No description provided for @serverDisable.
  ///
  /// In fr, this message translates to:
  /// **'Desactiver'**
  String get serverDisable;

  /// No description provided for @eventType.
  ///
  /// In fr, this message translates to:
  /// **'Event Type'**
  String get eventType;

  /// No description provided for @statusLabel.
  ///
  /// In fr, this message translates to:
  /// **'Status'**
  String get statusLabel;

  /// No description provided for @transferred.
  ///
  /// In fr, this message translates to:
  /// **'Transferred'**
  String get transferred;

  /// No description provided for @duration.
  ///
  /// In fr, this message translates to:
  /// **'Duration'**
  String get duration;

  /// No description provided for @resumes.
  ///
  /// In fr, this message translates to:
  /// **'Resumes'**
  String get resumes;

  /// No description provided for @verificationResult.
  ///
  /// In fr, this message translates to:
  /// **'Verification Result'**
  String get verificationResult;

  /// No description provided for @sanitizedPath.
  ///
  /// In fr, this message translates to:
  /// **'Sanitized Path'**
  String get sanitizedPath;

  /// No description provided for @timestamp.
  ///
  /// In fr, this message translates to:
  /// **'Timestamp'**
  String get timestamp;

  /// No description provided for @errorMessage.
  ///
  /// In fr, this message translates to:
  /// **'Error Message'**
  String get errorMessage;

  /// No description provided for @language.
  ///
  /// In fr, this message translates to:
  /// **'Langue'**
  String get language;

  /// No description provided for @languageDescription.
  ///
  /// In fr, this message translates to:
  /// **'Langue de l\'interface utilisateur'**
  String get languageDescription;

  /// No description provided for @french.
  ///
  /// In fr, this message translates to:
  /// **'Français'**
  String get french;

  /// No description provided for @english.
  ///
  /// In fr, this message translates to:
  /// **'English'**
  String get english;

  /// No description provided for @daemonConnectTitle.
  ///
  /// In fr, this message translates to:
  /// **'Connexion au daemon'**
  String get daemonConnectTitle;

  /// No description provided for @daemonConnectDescription.
  ///
  /// In fr, this message translates to:
  /// **'Emule-Sion Web pilote un mule-daemon distant. Indiquez l\'adresse WebSocket du daemon.'**
  String get daemonConnectDescription;

  /// No description provided for @daemonUrlLabel.
  ///
  /// In fr, this message translates to:
  /// **'Adresse du daemon'**
  String get daemonUrlLabel;

  /// No description provided for @daemonUrlHint.
  ///
  /// In fr, this message translates to:
  /// **'ws://192.168.1.10:4660/rpc'**
  String get daemonUrlHint;

  /// No description provided for @daemonUrlInvalid.
  ///
  /// In fr, this message translates to:
  /// **'Adresse invalide — format attendu : ws://hôte:port/rpc'**
  String get daemonUrlInvalid;

  /// No description provided for @connect.
  ///
  /// In fr, this message translates to:
  /// **'Se connecter'**
  String get connect;

  /// No description provided for @changeDaemon.
  ///
  /// In fr, this message translates to:
  /// **'Changer de daemon'**
  String get changeDaemon;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'fr'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'fr':
      return AppLocalizationsFr();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
