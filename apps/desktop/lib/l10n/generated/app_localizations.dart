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
