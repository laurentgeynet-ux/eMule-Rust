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
  /// **'eMule-Rust'**
  String get appName;

  /// No description provided for @appTagline.
  ///
  /// In fr, this message translates to:
  /// **'Client eD2K moderne'**
  String get appTagline;

  /// No description provided for @loopbackLabMode.
  ///
  /// In fr, this message translates to:
  /// **'Mode Lab Loopback'**
  String get loopbackLabMode;

  /// No description provided for @tooltipLoopbackLabMode.
  ///
  /// In fr, this message translates to:
  /// **'Mode laboratoire.\nLes connexions sont limitées à 127.0.0.1.\n\nAucun serveur eD2K public, nœud Kad ou pair externe.'**
  String get tooltipLoopbackLabMode;

  /// No description provided for @devModeBadge.
  ///
  /// In fr, this message translates to:
  /// **'DEV'**
  String get devModeBadge;

  /// No description provided for @sectionDownloads.
  ///
  /// In fr, this message translates to:
  /// **'TÉLÉCHARGEMENTS'**
  String get sectionDownloads;

  /// No description provided for @sectionCategories.
  ///
  /// In fr, this message translates to:
  /// **'CATÉGORIES'**
  String get sectionCategories;

  /// No description provided for @sectionAuditActivity.
  ///
  /// In fr, this message translates to:
  /// **'AUDIT & ACTIVITÉ'**
  String get sectionAuditActivity;

  /// No description provided for @navHistoryStats.
  ///
  /// In fr, this message translates to:
  /// **'Historique & Statistiques'**
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

  /// No description provided for @navUploads.
  ///
  /// In fr, this message translates to:
  /// **'Envois'**
  String get navUploads;

  /// No description provided for @filterAll.
  ///
  /// In fr, this message translates to:
  /// **'Tous les téléchargements'**
  String get filterAll;

  /// No description provided for @filterActive.
  ///
  /// In fr, this message translates to:
  /// **'Actifs'**
  String get filterActive;

  /// No description provided for @filterPaused.
  ///
  /// In fr, this message translates to:
  /// **'En pause'**
  String get filterPaused;

  /// No description provided for @filterCompleted.
  ///
  /// In fr, this message translates to:
  /// **'Terminés'**
  String get filterCompleted;

  /// No description provided for @filterFailed.
  ///
  /// In fr, this message translates to:
  /// **'Échoués'**
  String get filterFailed;

  /// No description provided for @uncategorized.
  ///
  /// In fr, this message translates to:
  /// **'Sans catégorie'**
  String get uncategorized;

  /// No description provided for @searchPlaceholder.
  ///
  /// In fr, this message translates to:
  /// **'Rechercher dans les téléchargements...'**
  String get searchPlaceholder;

  /// No description provided for @searchPlaceholderShares.
  ///
  /// In fr, this message translates to:
  /// **'Rechercher dans les partages...'**
  String get searchPlaceholderShares;

  /// No description provided for @btnAddLink.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter un lien'**
  String get btnAddLink;

  /// No description provided for @btnAddEd2kLink.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter un lien eD2K'**
  String get btnAddEd2kLink;

  /// No description provided for @btnAddDownload.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter un téléchargement'**
  String get btnAddDownload;

  /// No description provided for @btnCancel.
  ///
  /// In fr, this message translates to:
  /// **'Annuler'**
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
  /// **'Paramètres'**
  String get actionSettings;

  /// No description provided for @actionImportPartMet.
  ///
  /// In fr, this message translates to:
  /// **'Importer un .part.met'**
  String get actionImportPartMet;

  /// No description provided for @actionGuidedLabTest.
  ///
  /// In fr, this message translates to:
  /// **'Test local guidé'**
  String get actionGuidedLabTest;

  /// No description provided for @tooltipAddCategory.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter une catégorie'**
  String get tooltipAddCategory;

  /// No description provided for @tooltipRefresh.
  ///
  /// In fr, this message translates to:
  /// **'Actualiser les téléchargements'**
  String get tooltipRefresh;

  /// No description provided for @tooltipDevTools.
  ///
  /// In fr, this message translates to:
  /// **'Outils de simulation & débogage'**
  String get tooltipDevTools;

  /// No description provided for @tooltipResume.
  ///
  /// In fr, this message translates to:
  /// **'Reprendre'**
  String get tooltipResume;

  /// No description provided for @tooltipPause.
  ///
  /// In fr, this message translates to:
  /// **'Mettre en pause'**
  String get tooltipPause;

  /// No description provided for @tooltipDelete.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer'**
  String get tooltipDelete;

  /// No description provided for @emptyStateTitle.
  ///
  /// In fr, this message translates to:
  /// **'Aucun téléchargement dans cette vue'**
  String get emptyStateTitle;

  /// No description provided for @emptyStateSubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Ajoutez un lien eD2K pour créer un téléchargement local.\nCe build lab fonctionne en loopback uniquement.'**
  String get emptyStateSubtitle;

  /// No description provided for @zeroSpeedDual.
  ///
  /// In fr, this message translates to:
  /// **'↓ 0 B/s   ↑ 0 B/s'**
  String get zeroSpeedDual;

  /// No description provided for @daemonConnected.
  ///
  /// In fr, this message translates to:
  /// **'Daemon connecté'**
  String get daemonConnected;

  /// No description provided for @daemonConnecting.
  ///
  /// In fr, this message translates to:
  /// **'Connexion au daemon...'**
  String get daemonConnecting;

  /// No description provided for @daemonReconnecting.
  ///
  /// In fr, this message translates to:
  /// **'Reconnexion...'**
  String get daemonReconnecting;

  /// No description provided for @daemonError.
  ///
  /// In fr, this message translates to:
  /// **'Erreur du daemon'**
  String get daemonError;

  /// No description provided for @daemonDisconnected.
  ///
  /// In fr, this message translates to:
  /// **'Daemon déconnecté'**
  String get daemonDisconnected;

  /// No description provided for @daemonVersionLine.
  ///
  /// In fr, this message translates to:
  /// **'Daemon v{version} (proto v{protocol})'**
  String daemonVersionLine(String version, String protocol);

  /// No description provided for @dialogAddTitle.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter un téléchargement eD2K'**
  String get dialogAddTitle;

  /// No description provided for @dialogFieldLabel.
  ///
  /// In fr, this message translates to:
  /// **'URL du lien eD2K'**
  String get dialogFieldLabel;

  /// No description provided for @dialogFieldHint.
  ///
  /// In fr, this message translates to:
  /// **'ed2k://|file|name|size|hash|/'**
  String get dialogFieldHint;

  /// No description provided for @dialogValidationPrefix.
  ///
  /// In fr, this message translates to:
  /// **'Doit commencer par « ed2k://|file| »'**
  String get dialogValidationPrefix;

  /// No description provided for @dialogValidationParts.
  ///
  /// In fr, this message translates to:
  /// **'Doit contenir : nom, taille et hash'**
  String get dialogValidationParts;

  /// No description provided for @diagnosticsBadge.
  ///
  /// In fr, this message translates to:
  /// **'Diagnostics'**
  String get diagnosticsBadge;

  /// No description provided for @tooltipDiagnostics.
  ///
  /// In fr, this message translates to:
  /// **'Cliquer pour inspecter les diagnostics système & répertoire de travail.'**
  String get tooltipDiagnostics;

  /// No description provided for @tooltipSystemDiagnostics.
  ///
  /// In fr, this message translates to:
  /// **'Diagnostics système & confinement (127.0.0.1)'**
  String get tooltipSystemDiagnostics;

  /// No description provided for @tooltipOpenDrawer.
  ///
  /// In fr, this message translates to:
  /// **'Ouvrir le menu de navigation'**
  String get tooltipOpenDrawer;

  /// No description provided for @tooltipLabGuide.
  ///
  /// In fr, this message translates to:
  /// **'Flux de test local guidé'**
  String get tooltipLabGuide;

  /// No description provided for @killSwitchBadgeDisabled.
  ///
  /// In fr, this message translates to:
  /// **'Désactivé'**
  String get killSwitchBadgeDisabled;

  /// No description provided for @tooltipPolicyKill.
  ///
  /// In fr, this message translates to:
  /// **'Le sous-système réseau est DÉSACTIVÉ (kill switch engagé). Cliquer pour voir la politique et réactiver.'**
  String get tooltipPolicyKill;

  /// No description provided for @tooltipSessionTotals.
  ///
  /// In fr, this message translates to:
  /// **'Totaux de session — octets téléchargés / envoyés depuis le démarrage du daemon.\nCliquer pour ouvrir Historique & Statistiques.'**
  String get tooltipSessionTotals;

  /// No description provided for @tooltipLifetimeTotals.
  ///
  /// In fr, this message translates to:
  /// **'Totaux cumulés depuis l\'installation — téléchargé / envoyé.\nCliquer pour ouvrir Historique & Statistiques. Remise à zéro dans Paramètres → Avancé.'**
  String get tooltipLifetimeTotals;

  /// No description provided for @settingsLifetimeResetTile.
  ///
  /// In fr, this message translates to:
  /// **'Remise à zéro des totaux cumulés'**
  String get settingsLifetimeResetTile;

  /// No description provided for @settingsLifetimeResetSub.
  ///
  /// In fr, this message translates to:
  /// **'Remet à zéro les compteurs téléchargé / envoyé depuis l\'installation affichés dans la barre du haut. Les totaux de session sont conservés.'**
  String get settingsLifetimeResetSub;

  /// No description provided for @settingsLogLevelTile.
  ///
  /// In fr, this message translates to:
  /// **'Niveau de log du daemon'**
  String get settingsLogLevelTile;

  /// No description provided for @settingsLogLevelSub.
  ///
  /// In fr, this message translates to:
  /// **'Verbosité des fichiers de log rotatifs (logs/). Appliquée immédiatement, non persistée au redémarrage.'**
  String get settingsLogLevelSub;

  /// No description provided for @lifetimeResetConfirmTitle.
  ///
  /// In fr, this message translates to:
  /// **'Remettre à zéro les totaux ?'**
  String get lifetimeResetConfirmTitle;

  /// No description provided for @lifetimeResetConfirmBody.
  ///
  /// In fr, this message translates to:
  /// **'Les compteurs cumulés de téléchargement et d\'envoi depuis l\'installation seront remis à zéro. Les totaux de la session en cours ne sont pas modifiés.'**
  String get lifetimeResetConfirmBody;

  /// No description provided for @lifetimeResetConfirmButton.
  ///
  /// In fr, this message translates to:
  /// **'Remettre à zéro'**
  String get lifetimeResetConfirmButton;

  /// No description provided for @renameFileTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Renommer le fichier'**
  String get renameFileTooltip;

  /// No description provided for @renameFileTitle.
  ///
  /// In fr, this message translates to:
  /// **'Renommer le fichier'**
  String get renameFileTitle;

  /// No description provided for @renameFileLabel.
  ///
  /// In fr, this message translates to:
  /// **'Nouveau nom de fichier'**
  String get renameFileLabel;

  /// No description provided for @renameFileInvalid.
  ///
  /// In fr, this message translates to:
  /// **'Nom invalide — non vide, pas \'.\'/\'..\', ne finit pas par \'.\' et sans < > : \" / \\ | ? *'**
  String get renameFileInvalid;

  /// No description provided for @renameFileFailed.
  ///
  /// In fr, this message translates to:
  /// **'Le renommage a échoué côté daemon (nom déjà pris ou fichier verrouillé)'**
  String get renameFileFailed;

  /// No description provided for @renameFileApply.
  ///
  /// In fr, this message translates to:
  /// **'Renommer'**
  String get renameFileApply;

  /// No description provided for @tooltipKillSwitchEnable.
  ///
  /// In fr, this message translates to:
  /// **'Réactiver le sous-système réseau'**
  String get tooltipKillSwitchEnable;

  /// No description provided for @tooltipKillSwitchDisable.
  ///
  /// In fr, this message translates to:
  /// **'Kill switch : déconnecter immédiatement toutes les sockets'**
  String get tooltipKillSwitchDisable;

  /// No description provided for @tooltipIpFilter.
  ///
  /// In fr, this message translates to:
  /// **'Filtre IP & limites de débit'**
  String get tooltipIpFilter;

  /// No description provided for @addEd2kDownload.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter un téléchargement eD2K'**
  String get addEd2kDownload;

  /// No description provided for @ed2kLink.
  ///
  /// In fr, this message translates to:
  /// **'Lien eD2K'**
  String get ed2kLink;

  /// No description provided for @category.
  ///
  /// In fr, this message translates to:
  /// **'Catégorie'**
  String get category;

  /// No description provided for @targetDirectory.
  ///
  /// In fr, this message translates to:
  /// **'Dossier de destination'**
  String get targetDirectory;

  /// No description provided for @defaultDownloadsFolder.
  ///
  /// In fr, this message translates to:
  /// **'Dossier de téléchargements par défaut'**
  String get defaultDownloadsFolder;

  /// No description provided for @browse.
  ///
  /// In fr, this message translates to:
  /// **'Parcourir...'**
  String get browse;

  /// No description provided for @diskAllocation.
  ///
  /// In fr, this message translates to:
  /// **'Allocation disque'**
  String get diskAllocation;

  /// No description provided for @cancel.
  ///
  /// In fr, this message translates to:
  /// **'Annuler'**
  String get cancel;

  /// No description provided for @addDownload.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter le téléchargement'**
  String get addDownload;

  /// No description provided for @eGMoviesMusicLinux.
  ///
  /// In fr, this message translates to:
  /// **'ex : Films, Musique, ISOs Linux'**
  String get eGMoviesMusicLinux;

  /// No description provided for @inheritGlobalDestination.
  ///
  /// In fr, this message translates to:
  /// **'Hériter de la destination globale'**
  String get inheritGlobalDestination;

  /// No description provided for @devSimulationTools.
  ///
  /// In fr, this message translates to:
  /// **'Outils de simulation dev :'**
  String get devSimulationTools;

  /// No description provided for @selected.
  ///
  /// In fr, this message translates to:
  /// **'Sélectionné : \"{fileName}\"'**
  String selected(String fileName);

  /// No description provided for @inject256KbChunk.
  ///
  /// In fr, this message translates to:
  /// **'Injecter un bloc de 256 Ko'**
  String get inject256KbChunk;

  /// No description provided for @inject1MbChunk.
  ///
  /// In fr, this message translates to:
  /// **'Injecter un bloc de 1 Mo'**
  String get inject1MbChunk;

  /// No description provided for @clickAnyDownloadRowAbove.
  ///
  /// In fr, this message translates to:
  /// **'Cliquez sur une ligne de téléchargement ci-dessus pour la sélectionner pour les tests d\'injection.'**
  String get clickAnyDownloadRowAbove;

  /// No description provided for @closeDebugBar.
  ///
  /// In fr, this message translates to:
  /// **'Fermer la barre de débogage'**
  String get closeDebugBar;

  /// No description provided for @systemLabDiagnostics.
  ///
  /// In fr, this message translates to:
  /// **'Diagnostics système & lab'**
  String get systemLabDiagnostics;

  /// No description provided for @workingDirectoryMode.
  ///
  /// In fr, this message translates to:
  /// **'Répertoire de travail & mode'**
  String get workingDirectoryMode;

  /// No description provided for @mode.
  ///
  /// In fr, this message translates to:
  /// **'Mode'**
  String get mode;

  /// No description provided for @isolatedDataFolder.
  ///
  /// In fr, this message translates to:
  /// **'(Dossier de données isolé)'**
  String get isolatedDataFolder;

  /// No description provided for @internalData.
  ///
  /// In fr, this message translates to:
  /// **'Données internes'**
  String get internalData;

  /// No description provided for @downloads.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargements'**
  String get downloads;

  /// No description provided for @daemonProcess.
  ///
  /// In fr, this message translates to:
  /// **'Daemon & processus'**
  String get daemonProcess;

  /// No description provided for @processPid.
  ///
  /// In fr, this message translates to:
  /// **'PID du processus'**
  String get processPid;

  /// No description provided for @rpcEndpoint.
  ///
  /// In fr, this message translates to:
  /// **'Endpoint RPC'**
  String get rpcEndpoint;

  /// No description provided for @daemonVersion.
  ///
  /// In fr, this message translates to:
  /// **'Version du daemon'**
  String get daemonVersion;

  /// No description provided for @instanceId.
  ///
  /// In fr, this message translates to:
  /// **'ID d\'instance'**
  String get instanceId;

  /// No description provided for @sessionId.
  ///
  /// In fr, this message translates to:
  /// **'ID de session'**
  String get sessionId;

  /// No description provided for @daemonLog.
  ///
  /// In fr, this message translates to:
  /// **'Journal du daemon'**
  String get daemonLog;

  /// No description provided for @capabilities.
  ///
  /// In fr, this message translates to:
  /// **'Capacités'**
  String get capabilities;

  /// No description provided for @none.
  ///
  /// In fr, this message translates to:
  /// **'Aucune'**
  String get none;

  /// No description provided for @copyShareableSanitized.
  ///
  /// In fr, this message translates to:
  /// **'Copier version partageable (anonymisée)'**
  String get copyShareableSanitized;

  /// No description provided for @sanitizedShareableDiagnosticsCopied.
  ///
  /// In fr, this message translates to:
  /// **'Diagnostics anonymisés partageables copiés'**
  String get sanitizedShareableDiagnosticsCopied;

  /// No description provided for @copyFullLocalDev.
  ///
  /// In fr, this message translates to:
  /// **'Copier version complète (dev local)'**
  String get copyFullLocalDev;

  /// No description provided for @fullDiagnosticsCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Diagnostics complets copiés dans le presse-papiers'**
  String get fullDiagnosticsCopiedToClipboard;

  /// No description provided for @openLabGuide.
  ///
  /// In fr, this message translates to:
  /// **'Ouvrir le guide lab'**
  String get openLabGuide;

  /// No description provided for @close.
  ///
  /// In fr, this message translates to:
  /// **'Fermer'**
  String get close;

  /// No description provided for @details.
  ///
  /// In fr, this message translates to:
  /// **'Détails : {fileName}'**
  String details(String fileName);

  /// No description provided for @inspectFullDetails.
  ///
  /// In fr, this message translates to:
  /// **'Inspecter tous les détails'**
  String get inspectFullDetails;

  /// No description provided for @downloadId.
  ///
  /// In fr, this message translates to:
  /// **'ID du téléchargement'**
  String get downloadId;

  /// No description provided for @totalSize.
  ///
  /// In fr, this message translates to:
  /// **'Taille totale'**
  String get totalSize;

  /// No description provided for @verifiedBytes.
  ///
  /// In fr, this message translates to:
  /// **'Octets vérifiés'**
  String get verifiedBytes;

  /// No description provided for @status.
  ///
  /// In fr, this message translates to:
  /// **'Statut'**
  String get status;

  /// No description provided for @sources.
  ///
  /// In fr, this message translates to:
  /// **'Sources'**
  String get sources;

  /// No description provided for @downloadNotFound.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargement introuvable'**
  String get downloadNotFound;

  /// No description provided for @theRequestedDownloadIsNo.
  ///
  /// In fr, this message translates to:
  /// **'Le téléchargement demandé n\'est plus disponible.'**
  String get theRequestedDownloadIsNo;

  /// No description provided for @importPartMetExperimental.
  ///
  /// In fr, this message translates to:
  /// **'Importer un .part.met (expérimental)'**
  String get importPartMetExperimental;

  /// No description provided for @importAnExistingPartialOr.
  ///
  /// In fr, this message translates to:
  /// **'Importer un téléchargement eMule partiel ou terminé en toute sécurité.'**
  String get importAnExistingPartialOr;

  /// No description provided for @pathToPartMetFile.
  ///
  /// In fr, this message translates to:
  /// **'Chemin du fichier .part.met'**
  String get pathToPartMetFile;

  /// No description provided for @inspect.
  ///
  /// In fr, this message translates to:
  /// **'Inspecter'**
  String get inspect;

  /// No description provided for @associatedPartFilePathOptional.
  ///
  /// In fr, this message translates to:
  /// **'Chemin du fichier .part associé (optionnel, auto-détecté)'**
  String get associatedPartFilePathOptional;

  /// No description provided for @autoDetectedNextToMet.
  ///
  /// In fr, this message translates to:
  /// **'Auto-détecté à côté du fichier .met'**
  String get autoDetectedNextToMet;

  /// No description provided for @targetDownloadDirectory.
  ///
  /// In fr, this message translates to:
  /// **'Dossier de destination du téléchargement'**
  String get targetDownloadDirectory;

  /// No description provided for @targetDirectory2.
  ///
  /// In fr, this message translates to:
  /// **'Dossier de destination'**
  String get targetDirectory2;

  /// No description provided for @categoryOptional.
  ///
  /// In fr, this message translates to:
  /// **'Catégorie (optionnel)'**
  String get categoryOptional;

  /// No description provided for @importDownload.
  ///
  /// In fr, this message translates to:
  /// **'Importer le téléchargement'**
  String get importDownload;

  /// No description provided for @compatibilityAlert.
  ///
  /// In fr, this message translates to:
  /// **'Alerte de compatibilité'**
  String get compatibilityAlert;

  /// No description provided for @retry.
  ///
  /// In fr, this message translates to:
  /// **'Réessayer'**
  String get retry;

  /// No description provided for @ipFilterRateLimiters.
  ///
  /// In fr, this message translates to:
  /// **'Filtre IP & limiteurs de débit'**
  String get ipFilterRateLimiters;

  /// No description provided for @localDefensiveIpFilterIpfilter.
  ///
  /// In fr, this message translates to:
  /// **'Filtre IP défensif local (ipfilter.dat) et quotas de bande passante par seaux de jetons'**
  String get localDefensiveIpFilterIpfilter;

  /// No description provided for @connectedDownloadToLoopbackSource.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargement connecté à la source loopback ({peerAddr})'**
  String connectedDownloadToLoopbackSource(String peerAddr);

  /// No description provided for @cleanupCompletedFileSDeleted.
  ///
  /// In fr, this message translates to:
  /// **'Nettoyage terminé : {deletedFilesCount} fichier(s) supprimé(s) dans {folders} ({freedBytes} libérés).'**
  String cleanupCompletedFileSDeleted(
    String deletedFilesCount,
    String folders,
    String freedBytes,
  );

  /// No description provided for @guidedLocalLoopbackTestFlow.
  ///
  /// In fr, this message translates to:
  /// **'Flux de test loopback local guidé'**
  String get guidedLocalLoopbackTestFlow;

  /// No description provided for @fullEndToEndEd2k.
  ///
  /// In fr, this message translates to:
  /// **'Test eD2K complet de bout en bout sur 127.0.0.1 sans toucher à un terminal.'**
  String get fullEndToEndEd2k;

  /// No description provided for @networkPolicyDefenseGuard.
  ///
  /// In fr, this message translates to:
  /// **'Politique réseau & garde défensive'**
  String get networkPolicyDefenseGuard;

  /// No description provided for @defensiveSocketAuthorizationAntiSsrf.
  ///
  /// In fr, this message translates to:
  /// **'Autorisation défensive des sockets, isolation anti-SSRF et quotas de ressources'**
  String get defensiveSocketAuthorizationAntiSsrf;

  /// No description provided for @redactedNetworkAuditLogRecent.
  ///
  /// In fr, this message translates to:
  /// **'Journal d\'audit réseau anonymisé (décisions récentes)'**
  String get redactedNetworkAuditLogRecent;

  /// No description provided for @sparseInstantGrowsDynamically.
  ///
  /// In fr, this message translates to:
  /// **'Sparse (instantané, croît dynamiquement)'**
  String get sparseInstantGrowsDynamically;

  /// No description provided for @preallocateFullSizeAvoidsFragmentation.
  ///
  /// In fr, this message translates to:
  /// **'Pré-allouer (taille complète, évite la fragmentation)'**
  String get preallocateFullSizeAvoidsFragmentation;

  /// No description provided for @noneMinimalInitialDiskUse.
  ///
  /// In fr, this message translates to:
  /// **'Aucune (usage disque initial minimal)'**
  String get noneMinimalInitialDiskUse;

  /// No description provided for @sansCategorieNone.
  ///
  /// In fr, this message translates to:
  /// **'Sans catégorie'**
  String get sansCategorieNone;

  /// No description provided for @hash.
  ///
  /// In fr, this message translates to:
  /// **'Hash : {hash}'**
  String hash(String hash);

  /// No description provided for @rrggbb.
  ///
  /// In fr, this message translates to:
  /// **'#RRGGBB'**
  String get rrggbb;

  /// No description provided for @normalDefault.
  ///
  /// In fr, this message translates to:
  /// **'Normale (défaut)'**
  String get normalDefault;

  /// No description provided for @low.
  ///
  /// In fr, this message translates to:
  /// **'Basse'**
  String get low;

  /// No description provided for @normal.
  ///
  /// In fr, this message translates to:
  /// **'Normal'**
  String get normal;

  /// No description provided for @high.
  ///
  /// In fr, this message translates to:
  /// **'Haute'**
  String get high;

  /// No description provided for @changeTargetDestination.
  ///
  /// In fr, this message translates to:
  /// **'Changer la destination'**
  String get changeTargetDestination;

  /// No description provided for @selectTargetDestinationFolderFor.
  ///
  /// In fr, this message translates to:
  /// **'Choisir le dossier de destination pour \"{fileName}\" :'**
  String selectTargetDestinationFolderFor(String fileName);

  /// No description provided for @targetDirectoryPath.
  ///
  /// In fr, this message translates to:
  /// **'Chemin du dossier de destination'**
  String get targetDirectoryPath;

  /// No description provided for @eGDownloadsOrC.
  ///
  /// In fr, this message translates to:
  /// **'ex : downloads ou C:\\TargetDir'**
  String get eGDownloadsOrC;

  /// No description provided for @theFolderMustExistAnd.
  ///
  /// In fr, this message translates to:
  /// **'Le dossier doit exister et être accessible en écriture. Les données .part actives restent dans data/temp/ jusqu\'à la fin.'**
  String get theFolderMustExistAnd;

  /// No description provided for @saveDestination.
  ///
  /// In fr, this message translates to:
  /// **'Enregistrer la destination'**
  String get saveDestination;

  /// No description provided for @copiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'{label} copié dans le presse-papiers'**
  String copiedToClipboard(String label);

  /// No description provided for @resume.
  ///
  /// In fr, this message translates to:
  /// **'Reprendre'**
  String get resume;

  /// No description provided for @pause.
  ///
  /// In fr, this message translates to:
  /// **'Pause'**
  String get pause;

  /// No description provided for @retryLoopbackSource.
  ///
  /// In fr, this message translates to:
  /// **'Réessayer la source loopback'**
  String get retryLoopbackSource;

  /// No description provided for @openCompletedFile.
  ///
  /// In fr, this message translates to:
  /// **'Ouvrir le fichier terminé'**
  String get openCompletedFile;

  /// No description provided for @remove.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer...'**
  String get remove;

  /// No description provided for @diagnosticsRedactedExport.
  ///
  /// In fr, this message translates to:
  /// **'Diagnostics & export anonymisé'**
  String get diagnosticsRedactedExport;

  /// No description provided for @exportSanitizedDiagnosticsZeroAuth.
  ///
  /// In fr, this message translates to:
  /// **'Exporter les diagnostics anonymisés (zéro secret de jeton d\'auth, vérification du confinement loopback 127.0.0.1, noms d\'utilisateur et chemins masqués).'**
  String get exportSanitizedDiagnosticsZeroAuth;

  /// No description provided for @copyRedactedDiagnostic.
  ///
  /// In fr, this message translates to:
  /// **'Copier le diagnostic anonymisé'**
  String get copyRedactedDiagnostic;

  /// No description provided for @downloadDiagnosticCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Diagnostic du téléchargement copié (anonymisé : aucun secret)'**
  String get downloadDiagnosticCopiedToClipboard;

  /// No description provided for @identityEd2kLinks.
  ///
  /// In fr, this message translates to:
  /// **'Identité & liens eD2K'**
  String get identityEd2kLinks;

  /// No description provided for @downloadUuid.
  ///
  /// In fr, this message translates to:
  /// **'UUID du téléchargement'**
  String get downloadUuid;

  /// No description provided for @ed2kHashMd4.
  ///
  /// In fr, this message translates to:
  /// **'Hash eD2K (MD4)'**
  String get ed2kHashMd4;

  /// No description provided for @canonicalEd2kLink.
  ///
  /// In fr, this message translates to:
  /// **'Lien eD2K canonique'**
  String get canonicalEd2kLink;

  /// No description provided for @transferMetricsSpeed.
  ///
  /// In fr, this message translates to:
  /// **'Métriques de transfert & vitesse'**
  String get transferMetricsSpeed;

  /// No description provided for @received.
  ///
  /// In fr, this message translates to:
  /// **'Reçu'**
  String get received;

  /// No description provided for @remaining.
  ///
  /// In fr, this message translates to:
  /// **'Restant'**
  String get remaining;

  /// No description provided for @verified.
  ///
  /// In fr, this message translates to:
  /// **'Vérifié'**
  String get verified;

  /// No description provided for @currentRate.
  ///
  /// In fr, this message translates to:
  /// **'Débit actuel'**
  String get currentRate;

  /// No description provided for @averageRate.
  ///
  /// In fr, this message translates to:
  /// **'Débit moyen'**
  String get averageRate;

  /// No description provided for @eta.
  ///
  /// In fr, this message translates to:
  /// **'ETA'**
  String get eta;

  /// No description provided for @attempts.
  ///
  /// In fr, this message translates to:
  /// **'Tentatives'**
  String get attempts;

  /// No description provided for @lastError.
  ///
  /// In fr, this message translates to:
  /// **'Dernière erreur : {lastError}'**
  String lastError(String lastError);

  /// No description provided for @ed2kPartMapPartsOf.
  ///
  /// In fr, this message translates to:
  /// **'Carte des parts eD2K ({length} parts de 9,28 Mo)'**
  String ed2kPartMapPartsOf(String length);

  /// No description provided for @partBreakdownIsInitializing.
  ///
  /// In fr, this message translates to:
  /// **'Initialisation de la répartition des parts...'**
  String get partBreakdownIsInitializing;

  /// No description provided for @part.
  ///
  /// In fr, this message translates to:
  /// **'Part #{partIndex} Plage : {startOffset} - {endOffset} Taille : {size} Reçu : {receivedBytes} Statut : {status}'**
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
  /// **'Vérifiées ({verifiedCount})'**
  String verified2(String verifiedCount);

  /// No description provided for @completed.
  ///
  /// In fr, this message translates to:
  /// **'Terminées ({completedCount})'**
  String completed(String completedCount);

  /// No description provided for @downloading.
  ///
  /// In fr, this message translates to:
  /// **'En téléchargement ({downloadingCount})'**
  String downloading(String downloadingCount);

  /// No description provided for @missing.
  ///
  /// In fr, this message translates to:
  /// **'Manquantes ({missingCount})'**
  String missing(String missingCount);

  /// No description provided for @noSource.
  ///
  /// In fr, this message translates to:
  /// **'Aucune source ({unavailableCount})'**
  String noSource(String unavailableCount);

  /// No description provided for @storageTargetDestination.
  ///
  /// In fr, this message translates to:
  /// **'Stockage & destination'**
  String get storageTargetDestination;

  /// No description provided for @temporaryPartPathConfined.
  ///
  /// In fr, this message translates to:
  /// **'Chemin .part temporaire (confiné)'**
  String get temporaryPartPathConfined;

  /// No description provided for @finalTargetDestination.
  ///
  /// In fr, this message translates to:
  /// **'Destination finale'**
  String get finalTargetDestination;

  /// No description provided for @changeDestination.
  ///
  /// In fr, this message translates to:
  /// **'Changer la destination'**
  String get changeDestination;

  /// No description provided for @noteChangingDestinationRedirectsWhere.
  ///
  /// In fr, this message translates to:
  /// **'Note : changer la destination redirige l\'endroit où le fichier terminé est assemblé. Les blocs .part en cours restent en sécurité dans le dossier temporaire.'**
  String get noteChangingDestinationRedirectsWhere;

  /// No description provided for @removeDownload.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer le téléchargement ?'**
  String get removeDownload;

  /// No description provided for @removeFromTheDownloadsManager.
  ///
  /// In fr, this message translates to:
  /// **'Retirer \"{fileName}\" du gestionnaire de téléchargements ?'**
  String removeFromTheDownloadsManager(String fileName);

  /// No description provided for @theDownloadEntryInSqlite.
  ///
  /// In fr, this message translates to:
  /// **'• L\'entrée du téléchargement dans SQLite sera définitivement supprimée.\n• Les fichiers temporaires (.part et .part.met) dans data/temp/ seront supprimés.\n• Si le téléchargement est terminé, une option ci-dessous permet aussi de supprimer le fichier final dans le dossier de destination.'**
  String get theDownloadEntryInSqlite;

  /// No description provided for @deleteTemporaryPartFileFrom.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer le fichier .part temporaire de data/temp/'**
  String get deleteTemporaryPartFileFrom;

  /// No description provided for @confirmRemove.
  ///
  /// In fr, this message translates to:
  /// **'Confirmer la suppression'**
  String get confirmRemove;

  /// No description provided for @sourcesEd2kSessionPeer.
  ///
  /// In fr, this message translates to:
  /// **'SOURCES ED2K & PAIRS DE SESSION'**
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
  /// **'Sans catégorie'**
  String get noCategory;

  /// No description provided for @totalSize2.
  ///
  /// In fr, this message translates to:
  /// **'Taille totale :'**
  String get totalSize2;

  /// No description provided for @estAcquiredGapsComplement.
  ///
  /// In fr, this message translates to:
  /// **'Acquis est. / complément des trous :'**
  String get estAcquiredGapsComplement;

  /// No description provided for @partCount.
  ///
  /// In fr, this message translates to:
  /// **'Nombre de parts :'**
  String get partCount;

  /// No description provided for @ed2kHash.
  ///
  /// In fr, this message translates to:
  /// **'Hash eD2K'**
  String get ed2kHash;

  /// No description provided for @importSuccessfulTheDownloadIs.
  ///
  /// In fr, this message translates to:
  /// **'Import réussi ! Le téléchargement est en pause et les parts vérifiées ont été préparées.'**
  String get importSuccessfulTheDownloadIs;

  /// No description provided for @auditReport.
  ///
  /// In fr, this message translates to:
  /// **'Rapport d\'audit'**
  String get auditReport;

  /// No description provided for @auditReportCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Rapport d\'audit copié dans le presse-papiers'**
  String get auditReportCopiedToClipboard;

  /// No description provided for @copyReport.
  ///
  /// In fr, this message translates to:
  /// **'Copier le rapport'**
  String get copyReport;

  /// No description provided for @done.
  ///
  /// In fr, this message translates to:
  /// **'Terminé'**
  String get done;

  /// No description provided for @theDownloadWillBeStaged.
  ///
  /// In fr, this message translates to:
  /// **'Le téléchargement sera préparé dans data/temp/ et initialisé en pause. L\'import ne contacte aucun pair eD2K.'**
  String get theDownloadWillBeStaged;

  /// No description provided for @confirmCleanup.
  ///
  /// In fr, this message translates to:
  /// **'Confirmer le nettoyage'**
  String get confirmCleanup;

  /// No description provided for @thisWillRemoveFixtureFile.
  ///
  /// In fr, this message translates to:
  /// **'Cela supprimera {fixtureCount} fichier(s) fixture dans <workdir>/fixtures/ et nettoiera les fichiers .part temporaires.'**
  String thisWillRemoveFixtureFile(String fixtureCount);

  /// No description provided for @deleteTestData.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer les données de test'**
  String get deleteTestData;

  /// No description provided for @storageDataInternalStateVs.
  ///
  /// In fr, this message translates to:
  /// **'Stockage : data/ (état interne) vs downloads/ (fichiers terminés)'**
  String get storageDataInternalStateVs;

  /// No description provided for @openFolder.
  ///
  /// In fr, this message translates to:
  /// **'Ouvrir le dossier'**
  String get openFolder;

  /// No description provided for @internalDataDbTempPart.
  ///
  /// In fr, this message translates to:
  /// **'• Données internes (DB, .part temporaires, fixtures) : {workdir}'**
  String internalDataDbTempPart(String workdir);

  /// No description provided for @completedDownloads.
  ///
  /// In fr, this message translates to:
  /// **'• Téléchargements terminés : {downloadsDir}'**
  String completedDownloads(String downloadsDir);

  /// No description provided for @cleanTestDataFixturesIncomplete.
  ///
  /// In fr, this message translates to:
  /// **'Nettoyer les données de test (fixtures & parts incomplètes)'**
  String get cleanTestDataFixturesIncomplete;

  /// No description provided for @cleanupTestData.
  ///
  /// In fr, this message translates to:
  /// **'Nettoyer les données de test'**
  String get cleanupTestData;

  /// No description provided for @connectDownloadToLoopbackSource.
  ///
  /// In fr, this message translates to:
  /// **'Connecter le téléchargement à la source loopback'**
  String get connectDownloadToLoopbackSource;

  /// No description provided for @noDownloadsInQueueAdd.
  ///
  /// In fr, this message translates to:
  /// **'Aucun téléchargement en file. Ajoutez un lien eD2K à l\'étape 2.'**
  String get noDownloadsInQueueAdd;

  /// No description provided for @targetDownloadTask.
  ///
  /// In fr, this message translates to:
  /// **'Tâche de téléchargement cible'**
  String get targetDownloadTask;

  /// No description provided for @warningDownloadSize.
  ///
  /// In fr, this message translates to:
  /// **'Attention : la taille du téléchargement ({totalBytes}) ne correspond pas à la fixture ({fixtureSize}).'**
  String warningDownloadSize(String totalBytes, String fixtureSize);

  /// No description provided for @startLoopbackPeerInStep.
  ///
  /// In fr, this message translates to:
  /// **'Démarrez le pair loopback à l\'étape 3 avant de connecter.'**
  String get startLoopbackPeerInStep;

  /// No description provided for @finalFile.
  ///
  /// In fr, this message translates to:
  /// **'Fichier final : {downloadsDir}\\{fileName}'**
  String finalFile(String fileName, String downloadsDir);

  /// No description provided for @generateLocalTestFixture.
  ///
  /// In fr, this message translates to:
  /// **'Générer une fixture de test locale'**
  String get generateLocalTestFixture;

  /// No description provided for @obtainCanonicalEd2kLinkAdd.
  ///
  /// In fr, this message translates to:
  /// **'Obtenir le lien eD2K canonique & ajouter aux téléchargements'**
  String get obtainCanonicalEd2kLinkAdd;

  /// No description provided for @copyLink.
  ///
  /// In fr, this message translates to:
  /// **'Copier le lien'**
  String get copyLink;

  /// No description provided for @ed2kLinkCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Lien eD2K copié dans le presse-papiers'**
  String get ed2kLinkCopiedToClipboard;

  /// No description provided for @downloadTaskCreatedInUi.
  ///
  /// In fr, this message translates to:
  /// **'Tâche de téléchargement créée dans l\'UI'**
  String get downloadTaskCreatedInUi;

  /// No description provided for @completeStep1ToGenerate.
  ///
  /// In fr, this message translates to:
  /// **'Terminez l\'étape 1 pour générer d\'abord une fixture eD2K.'**
  String get completeStep1ToGenerate;

  /// No description provided for @loopbackTestPeer1270.
  ///
  /// In fr, this message translates to:
  /// **'Pair de test loopback (127.0.0.1)'**
  String get loopbackTestPeer1270;

  /// No description provided for @lastImportReport.
  ///
  /// In fr, this message translates to:
  /// **'Dernier rapport d\'import'**
  String get lastImportReport;

  /// No description provided for @invalidLinesSkipped.
  ///
  /// In fr, this message translates to:
  /// **'Lignes invalides ignorées : {invalidLines}'**
  String invalidLinesSkipped(String invalidLines);

  /// No description provided for @rangesLoaded.
  ///
  /// In fr, this message translates to:
  /// **'{rangeCount} plages chargées'**
  String rangesLoaded(String rangeCount);

  /// No description provided for @totalBlockedConnectionAttempts.
  ///
  /// In fr, this message translates to:
  /// **'Connexions bloquées au total : {hitsCount}'**
  String totalBlockedConnectionAttempts(String hitsCount);

  /// No description provided for @importIpfilterDat.
  ///
  /// In fr, this message translates to:
  /// **'Importer un ipfilter.dat'**
  String get importIpfilterDat;

  /// No description provided for @loadALocalPeerguardianEmule.
  ///
  /// In fr, this message translates to:
  /// **'Charge un fichier ipfilter.dat local au format PeerGuardian / eMule. Le téléchargement depuis une URL distante est interdit par sécurité (anti-SSRF).'**
  String get loadALocalPeerguardianEmule;

  /// No description provided for @import.
  ///
  /// In fr, this message translates to:
  /// **'Importer'**
  String get import;

  /// No description provided for @clear.
  ///
  /// In fr, this message translates to:
  /// **'Effacer'**
  String get clear;

  /// No description provided for @noNetworkEventsRecordedYet.
  ///
  /// In fr, this message translates to:
  /// **'Aucun événement réseau enregistré pour le moment.'**
  String get noNetworkEventsRecordedYet;

  /// No description provided for @activeConnectionsLimits.
  ///
  /// In fr, this message translates to:
  /// **'Connexions actives & limites'**
  String get activeConnectionsLimits;

  /// No description provided for @peerSockets.
  ///
  /// In fr, this message translates to:
  /// **'Sockets pairs'**
  String get peerSockets;

  /// No description provided for @serverSockets.
  ///
  /// In fr, this message translates to:
  /// **'Sockets serveur'**
  String get serverSockets;

  /// No description provided for @listeners.
  ///
  /// In fr, this message translates to:
  /// **'Listeners d\'écoute'**
  String get listeners;

  /// No description provided for @connectTimeout.
  ///
  /// In fr, this message translates to:
  /// **'Timeout de connexion'**
  String get connectTimeout;

  /// No description provided for @maxFrameSize.
  ///
  /// In fr, this message translates to:
  /// **'Taille max d\'une trame'**
  String get maxFrameSize;

  /// No description provided for @maxBufferMemory.
  ///
  /// In fr, this message translates to:
  /// **'Mémoire tampon max'**
  String get maxBufferMemory;

  /// No description provided for @status2.
  ///
  /// In fr, this message translates to:
  /// **'Statut : {label}'**
  String status2(String label);

  /// No description provided for @reEnable.
  ///
  /// In fr, this message translates to:
  /// **'Réactiver le réseau'**
  String get reEnable;

  /// No description provided for @disconnectAll.
  ///
  /// In fr, this message translates to:
  /// **'Tout déconnecter (kill switch)'**
  String get disconnectAll;

  /// No description provided for @bandwidthTokenBucketQuotas.
  ///
  /// In fr, this message translates to:
  /// **'Limites de débit (Token Buckets)'**
  String get bandwidthTokenBucketQuotas;

  /// No description provided for @limitsAreEnforcedAsynchronouslyUsing.
  ///
  /// In fr, this message translates to:
  /// **'Les limites sont appliquées de façon asynchrone par des Token Buckets et persistées au redémarrage. 0 = illimité.'**
  String get limitsAreEnforcedAsynchronouslyUsing;

  /// No description provided for @downloadLimitKbS0.
  ///
  /// In fr, this message translates to:
  /// **'Limite de téléchargement (Ko/s) [0 = illimité]'**
  String get downloadLimitKbS0;

  /// No description provided for @uploadLimitKbS0.
  ///
  /// In fr, this message translates to:
  /// **'Limite d\'envoi (Ko/s) [0 = illimité]'**
  String get uploadLimitKbS0;

  /// No description provided for @maxNewConnectionsMinute.
  ///
  /// In fr, this message translates to:
  /// **'Nouvelles connexions max / minute'**
  String get maxNewConnectionsMinute;

  /// No description provided for @saveLimits.
  ///
  /// In fr, this message translates to:
  /// **'Enregistrer les limites'**
  String get saveLimits;

  /// No description provided for @inspectDetailsPartMap.
  ///
  /// In fr, this message translates to:
  /// **'Inspecter détails & carte des parts'**
  String get inspectDetailsPartMap;

  /// No description provided for @remove2.
  ///
  /// In fr, this message translates to:
  /// **'Retirer'**
  String get remove2;

  /// No description provided for @removeFromDownloads.
  ///
  /// In fr, this message translates to:
  /// **'Retirer \"{fileName}\" des téléchargements ?'**
  String removeFromDownloads(String fileName);

  /// No description provided for @deleteDownloadedPartTempFiles.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer les fichiers .part/temp téléchargés du disque'**
  String get deleteDownloadedPartTempFiles;

  /// No description provided for @deleteSharedCompletedFile.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer aussi le fichier partagé dans le dossier de destination'**
  String get deleteSharedCompletedFile;

  /// No description provided for @id.
  ///
  /// In fr, this message translates to:
  /// **'ID : {id}'**
  String id(String id);

  /// No description provided for @src.
  ///
  /// In fr, this message translates to:
  /// **'{sourceCount} src'**
  String src(String sourceCount);

  /// No description provided for @fullId.
  ///
  /// In fr, this message translates to:
  /// **'ID complet : {id}'**
  String fullId(String id);

  /// No description provided for @downloadIdCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'ID du téléchargement copié dans le presse-papiers'**
  String get downloadIdCopiedToClipboard;

  /// No description provided for @sources2.
  ///
  /// In fr, this message translates to:
  /// **'{sourceCount} sources'**
  String sources2(String sourceCount);

  /// No description provided for @downloadSettingsStorage.
  ///
  /// In fr, this message translates to:
  /// **'Paramètres de téléchargement & stockage'**
  String get downloadSettingsStorage;

  /// No description provided for @globalDefaultDownloadDirectory.
  ///
  /// In fr, this message translates to:
  /// **'Dossier de téléchargement par défaut'**
  String get globalDefaultDownloadDirectory;

  /// No description provided for @thisPathIsUsedAs.
  ///
  /// In fr, this message translates to:
  /// **'Ce chemin est la destination par défaut des nouveaux téléchargements. Les fichiers .part restent dans le dossier temporaire configuré jusqu\'à complétion (eMule « Répertoire »).'**
  String get thisPathIsUsedAs;

  /// No description provided for @destinationDirectoryPath.
  ///
  /// In fr, this message translates to:
  /// **'Chemin du dossier de destination'**
  String get destinationDirectoryPath;

  /// No description provided for @eGCDownloadsOr.
  ///
  /// In fr, this message translates to:
  /// **'ex. C:\\Downloads ou downloads'**
  String get eGCDownloadsOr;

  /// No description provided for @save.
  ///
  /// In fr, this message translates to:
  /// **'Enregistrer'**
  String get save;

  /// No description provided for @resetToPortableDefault.
  ///
  /// In fr, this message translates to:
  /// **'Réinitialiser au dossier portable'**
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
  /// **'Rétention de l\'historique'**
  String get historyRetentionPolicy;

  /// No description provided for @historicalTransitionEventsOlderThan.
  ///
  /// In fr, this message translates to:
  /// **'Les événements d\'historique plus anciens que la durée choisie sont purgés au démarrage du daemon et à chaque modification.'**
  String get historicalTransitionEventsOlderThan;

  /// No description provided for @keepHistoryFor.
  ///
  /// In fr, this message translates to:
  /// **'Conserver l\'historique pendant : '**
  String get keepHistoryFor;

  /// No description provided for @indefinitelyNoPruning.
  ///
  /// In fr, this message translates to:
  /// **'Indéfiniment (aucune purge)'**
  String get indefinitelyNoPruning;

  /// No description provided for @delete.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer \"{name}\" ?'**
  String delete(String name);

  /// No description provided for @deletingThisCategoryWillMove.
  ///
  /// In fr, this message translates to:
  /// **'Deleting this category will move all associated downloads to \"Sans catégorie\" (Uncategorized).\n\nNo downloads or files will be deleted.'**
  String get deletingThisCategoryWillMove;

  /// No description provided for @deleteCategory.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer la catégorie'**
  String get deleteCategory;

  /// No description provided for @categoryOptions.
  ///
  /// In fr, this message translates to:
  /// **'Options de catégorie'**
  String get categoryOptions;

  /// No description provided for @editCategory.
  ///
  /// In fr, this message translates to:
  /// **'Modifier la catégorie'**
  String get editCategory;

  /// No description provided for @clearDownloadHistory.
  ///
  /// In fr, this message translates to:
  /// **'Effacer l\'historique des téléchargements ?'**
  String get clearDownloadHistory;

  /// No description provided for @areYouSureYouWant.
  ///
  /// In fr, this message translates to:
  /// **'Voulez-vous vraiment supprimer définitivement tout l\'historique et les statistiques de téléchargement ? Cette action est irréversible.'**
  String get areYouSureYouWant;

  /// No description provided for @clearAllHistory.
  ///
  /// In fr, this message translates to:
  /// **'Effacer tout l\'historique'**
  String get clearAllHistory;

  /// No description provided for @noDownloadHistoryRecordedYet.
  ///
  /// In fr, this message translates to:
  /// **'Aucun historique de téléchargement enregistré'**
  String get noDownloadHistoryRecordedYet;

  /// No description provided for @completedResumedFailedAndRemoved.
  ///
  /// In fr, this message translates to:
  /// **'Les événements de transfert terminés, repris, échoués et supprimés apparaîtront ici.'**
  String get completedResumedFailedAndRemoved;

  /// No description provided for @eventDetail.
  ///
  /// In fr, this message translates to:
  /// **'Détail de l\'événement : {fileName}'**
  String eventDetail(String fileName);

  /// No description provided for @viewDetails.
  ///
  /// In fr, this message translates to:
  /// **'Voir les détails'**
  String get viewDetails;

  /// No description provided for @of2.
  ///
  /// In fr, this message translates to:
  /// **'{transferredBytes} sur {totalBytes}'**
  String of2(String transferredBytes, String totalBytes);

  /// No description provided for @hash2.
  ///
  /// In fr, this message translates to:
  /// **'Hash : {ed2kHash}'**
  String hash2(String ed2kHash);

  /// No description provided for @of3.
  ///
  /// In fr, this message translates to:
  /// **'sur {totalBytes}'**
  String of3(String totalBytes);

  /// No description provided for @res.
  ///
  /// In fr, this message translates to:
  /// **'{resumeCount} res. / {connectionAttempts} att.'**
  String res(String resumeCount, String connectionAttempts);

  /// No description provided for @exportHistory.
  ///
  /// In fr, this message translates to:
  /// **'Exporter l\'historique ({toUpperCase})'**
  String exportHistory(String toUpperCase);

  /// No description provided for @reportIsSanitizedAllLocal.
  ///
  /// In fr, this message translates to:
  /// **'Le rapport est anonymisé : tous les noms d\'utilisateur et chemins locaux sont masqués.'**
  String get reportIsSanitizedAllLocal;

  /// No description provided for @copyToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Copier dans le presse-papiers'**
  String get copyToClipboard;

  /// No description provided for @exportCopiedToClipboard.
  ///
  /// In fr, this message translates to:
  /// **'Export copié dans le presse-papiers'**
  String get exportCopiedToClipboard;

  /// No description provided for @filterByEvent.
  ///
  /// In fr, this message translates to:
  /// **'Filtrer par événement :'**
  String get filterByEvent;

  /// No description provided for @allEvents.
  ///
  /// In fr, this message translates to:
  /// **'Tous les événements'**
  String get allEvents;

  /// No description provided for @completed2.
  ///
  /// In fr, this message translates to:
  /// **'Terminés'**
  String get completed2;

  /// No description provided for @failed.
  ///
  /// In fr, this message translates to:
  /// **'Échoués'**
  String get failed;

  /// No description provided for @resumed.
  ///
  /// In fr, this message translates to:
  /// **'Repris'**
  String get resumed;

  /// No description provided for @verifying.
  ///
  /// In fr, this message translates to:
  /// **'Vérification'**
  String get verifying;

  /// No description provided for @removed.
  ///
  /// In fr, this message translates to:
  /// **'Supprimés'**
  String get removed;

  /// No description provided for @recordS.
  ///
  /// In fr, this message translates to:
  /// **'{length} enregistrement(s)'**
  String recordS(String length);

  /// No description provided for @totalCompleted.
  ///
  /// In fr, this message translates to:
  /// **'Total terminés'**
  String get totalCompleted;

  /// No description provided for @activityWindows.
  ///
  /// In fr, this message translates to:
  /// **'Fenêtres d\'activité'**
  String get activityWindows;

  /// No description provided for @resumesRetries.
  ///
  /// In fr, this message translates to:
  /// **'Reprises & tentatives'**
  String get resumesRetries;

  /// No description provided for @failuresLogged.
  ///
  /// In fr, this message translates to:
  /// **'{totalFailedCount} échecs enregistrés'**
  String failuresLogged(String totalFailedCount);

  /// No description provided for @integrityHealth.
  ///
  /// In fr, this message translates to:
  /// **'Intégrité'**
  String get integrityHealth;

  /// No description provided for @historyLocalStatistics.
  ///
  /// In fr, this message translates to:
  /// **'Historique & statistiques locales'**
  String get historyLocalStatistics;

  /// No description provided for @exportHistoryReport.
  ///
  /// In fr, this message translates to:
  /// **'Exporter le rapport d\'historique'**
  String get exportHistoryReport;

  /// No description provided for @exportAsJsonSanitized.
  ///
  /// In fr, this message translates to:
  /// **'Exporter en JSON (anonymisé)'**
  String get exportAsJsonSanitized;

  /// No description provided for @exportAsCsvSanitized.
  ///
  /// In fr, this message translates to:
  /// **'Exporter en CSV (anonymisé)'**
  String get exportAsCsvSanitized;

  /// No description provided for @exportReport.
  ///
  /// In fr, this message translates to:
  /// **'Exporter le rapport'**
  String get exportReport;

  /// No description provided for @clearHistory.
  ///
  /// In fr, this message translates to:
  /// **'Effacer l\'historique'**
  String get clearHistory;

  /// No description provided for @refresh.
  ///
  /// In fr, this message translates to:
  /// **'Actualiser'**
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
  /// **'ID Kad (XOR 128 bits)'**
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
  /// **'Envoyer un ping Kad (0x60)'**
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
  /// **'Extension (ex : iso, zip, avi)'**
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
  /// **'Effacer les filtres'**
  String get effacerFiltres;

  /// No description provided for @serveurEd2k.
  ///
  /// In fr, this message translates to:
  /// **'Serveur eD2K'**
  String get serveurEd2k;

  /// No description provided for @globalUdp.
  ///
  /// In fr, this message translates to:
  /// **'Global (UDP)'**
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
  /// **'Hash eD2K'**
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
  /// **'Copier le lien eD2K'**
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
  /// **'Sélectionner un server.met'**
  String get selectionnerServerMet;

  /// No description provided for @parDefauteMuleRustEst.
  ///
  /// In fr, this message translates to:
  /// **'Par défaut, eMule-Rust est confiné en boucle locale (LocalLab). Basculez sur PublicEd2kPreview pour autoriser les serveurs publics.'**
  String get parDefauteMuleRustEst;

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
  /// **'État / Filtre'**
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
  /// **'Préférence'**
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
  /// **'Importer un server.met'**
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
  /// **'Indiquez l\'adresse HTTP ou HTTPS d\'un fichier server.met distant pour mettre à jour automatiquement votre annuaire de serveurs.'**
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
  /// **'Autorisés'**
  String get autorises;

  /// No description provided for @bloquesFiltreIp.
  ///
  /// In fr, this message translates to:
  /// **'Bloqués (filtre IP)'**
  String get bloquesFiltreIp;

  /// No description provided for @lesServeursMarquesEnRouge.
  ///
  /// In fr, this message translates to:
  /// **'Les serveurs marques en rouge correspondent a des plages de votre filtre IP local. Ils seront importes avec le statut bloque.'**
  String get lesServeursMarquesEnRouge;

  /// No description provided for @bloque.
  ///
  /// In fr, this message translates to:
  /// **'Bloqué'**
  String get bloque;

  /// No description provided for @autorise.
  ///
  /// In fr, this message translates to:
  /// **'Autorisé'**
  String get autorise;

  /// No description provided for @confirmerImportation.
  ///
  /// In fr, this message translates to:
  /// **'Confirmer l\'importation'**
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
  /// **'Même en mode public, la politique réseau d\'eMule-Rust bloque rigoureusement tout trafic sortant vers les adresses de votre réseau local privé (192.168.x.x, 10.x.x.x, 172.16-31.x.x) pour protéger vos équipements domestiques.'**
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
  /// **'7 jours'**
  String get sevenDays;

  /// No description provided for @thirtyDaysDefault.
  ///
  /// In fr, this message translates to:
  /// **'30 jours (défaut)'**
  String get thirtyDaysDefault;

  /// No description provided for @ninetyDays.
  ///
  /// In fr, this message translates to:
  /// **'90 jours'**
  String get ninetyDays;

  /// No description provided for @twentyFourHours.
  ///
  /// In fr, this message translates to:
  /// **'24 h : {last24hCompletedCount} | 7 j : {last7dCompletedCount}'**
  String twentyFourHours(
    String last24hCompletedCount,
    String last7dCompletedCount,
  );

  /// No description provided for @receivedOfTotal.
  ///
  /// In fr, this message translates to:
  /// **'{receivedBytes} sur {totalBytes} ({progress})'**
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
  /// **'Désactiver'**
  String get serverDisable;

  /// No description provided for @eventType.
  ///
  /// In fr, this message translates to:
  /// **'Type d\'événement'**
  String get eventType;

  /// No description provided for @statusLabel.
  ///
  /// In fr, this message translates to:
  /// **'Statut'**
  String get statusLabel;

  /// No description provided for @transferred.
  ///
  /// In fr, this message translates to:
  /// **'Transféré'**
  String get transferred;

  /// No description provided for @duration.
  ///
  /// In fr, this message translates to:
  /// **'Durée'**
  String get duration;

  /// No description provided for @resumes.
  ///
  /// In fr, this message translates to:
  /// **'Reprises'**
  String get resumes;

  /// No description provided for @verificationResult.
  ///
  /// In fr, this message translates to:
  /// **'Résultat de la vérification'**
  String get verificationResult;

  /// No description provided for @sanitizedPath.
  ///
  /// In fr, this message translates to:
  /// **'Chemin anonymisé'**
  String get sanitizedPath;

  /// No description provided for @timestamp.
  ///
  /// In fr, this message translates to:
  /// **'Horodatage'**
  String get timestamp;

  /// No description provided for @errorMessage.
  ///
  /// In fr, this message translates to:
  /// **'Message d\'erreur'**
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
  /// **'eMule-Rust Web pilote un mule-daemon distant. Indiquez l\'adresse WebSocket du daemon.'**
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

  /// No description provided for @daemonTokenLabel.
  ///
  /// In fr, this message translates to:
  /// **'Jeton d\'authentification'**
  String get daemonTokenLabel;

  /// No description provided for @daemonTokenHint.
  ///
  /// In fr, this message translates to:
  /// **'Contenu du fichier auth.token (laisser vide si absent)'**
  String get daemonTokenHint;

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

  /// No description provided for @webInterface.
  ///
  /// In fr, this message translates to:
  /// **'Interface web'**
  String get webInterface;

  /// No description provided for @sectionUploads.
  ///
  /// In fr, this message translates to:
  /// **'Envois'**
  String get sectionUploads;

  /// No description provided for @uploadsActive.
  ///
  /// In fr, this message translates to:
  /// **'UPLOADS ACTIFS'**
  String get uploadsActive;

  /// No description provided for @uploadsQueue.
  ///
  /// In fr, this message translates to:
  /// **'FILE D\'ATTENTE'**
  String get uploadsQueue;

  /// No description provided for @uploadsActiveEmpty.
  ///
  /// In fr, this message translates to:
  /// **'Aucun upload actif'**
  String get uploadsActiveEmpty;

  /// No description provided for @uploadsQueueEmpty.
  ///
  /// In fr, this message translates to:
  /// **'File d\'attente vide'**
  String get uploadsQueueEmpty;

  /// No description provided for @uploadsClient.
  ///
  /// In fr, this message translates to:
  /// **'Client'**
  String get uploadsClient;

  /// No description provided for @uploadsFile.
  ///
  /// In fr, this message translates to:
  /// **'Fichier'**
  String get uploadsFile;

  /// No description provided for @uploadsRate.
  ///
  /// In fr, this message translates to:
  /// **'Débit'**
  String get uploadsRate;

  /// No description provided for @uploadsRank.
  ///
  /// In fr, this message translates to:
  /// **'Rang'**
  String get uploadsRank;

  /// No description provided for @uploadsCancelTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Annuler l\'upload'**
  String get uploadsCancelTooltip;

  /// No description provided for @settingsSectionGeneral.
  ///
  /// In fr, this message translates to:
  /// **'Général'**
  String get settingsSectionGeneral;

  /// No description provided for @settingsSectionConnection.
  ///
  /// In fr, this message translates to:
  /// **'Connexion'**
  String get settingsSectionConnection;

  /// No description provided for @settingsSectionTransfers.
  ///
  /// In fr, this message translates to:
  /// **'Transferts'**
  String get settingsSectionTransfers;

  /// No description provided for @settingsSectionSecurity.
  ///
  /// In fr, this message translates to:
  /// **'Sécurité'**
  String get settingsSectionSecurity;

  /// No description provided for @settingsSectionFriends.
  ///
  /// In fr, this message translates to:
  /// **'Amis'**
  String get settingsSectionFriends;

  /// No description provided for @settingsSectionAdvanced.
  ///
  /// In fr, this message translates to:
  /// **'Avancé'**
  String get settingsSectionAdvanced;

  /// No description provided for @settingsGeneralSubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Identité, dossiers, partage automatique, historique et langue'**
  String get settingsGeneralSubtitle;

  /// No description provided for @settingsConnectionSubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Profil réseau, isolation, budgets de sockets et UPnP'**
  String get settingsConnectionSubtitle;

  /// No description provided for @settingsTransfersSubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Limites de connexions aux sources du swarm'**
  String get settingsTransfersSubtitle;

  /// No description provided for @settingsSecuritySubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Filtre IP, limites de débit et politique anti-SSRF'**
  String get settingsSecuritySubtitle;

  /// No description provided for @settingsFriendsSubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Slot d\'upload dédié sans passer par la file (friend slot eMule)'**
  String get settingsFriendsSubtitle;

  /// No description provided for @settingsAdvancedSubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Diagnostic, outils de laboratoire et informations daemon'**
  String get settingsAdvancedSubtitle;

  /// No description provided for @settingsApply.
  ///
  /// In fr, this message translates to:
  /// **'Appliquer'**
  String get settingsApply;

  /// No description provided for @settingsSaving.
  ///
  /// In fr, this message translates to:
  /// **'Enregistrement…'**
  String get settingsSaving;

  /// No description provided for @settingsNicknameLabel.
  ///
  /// In fr, this message translates to:
  /// **'Pseudo (nickname)'**
  String get settingsNicknameLabel;

  /// No description provided for @settingsNicknameHelper.
  ///
  /// In fr, this message translates to:
  /// **'Annoncé aux serveurs eD2K et aux pairs (eMule « nick » ; vide = défaut)'**
  String get settingsNicknameHelper;

  /// No description provided for @settingsShareDownloadDirTitle.
  ///
  /// In fr, this message translates to:
  /// **'Partager automatiquement le dossier de téléchargement'**
  String get settingsShareDownloadDirTitle;

  /// No description provided for @settingsShareDownloadDirSub.
  ///
  /// In fr, this message translates to:
  /// **'Les fichiers du dossier Incoming sont hashés et servis aux pairs au démarrage du daemon (comme le dossier Incoming d\'eMule).'**
  String get settingsShareDownloadDirSub;

  /// No description provided for @settingsTempDirLabel.
  ///
  /// In fr, this message translates to:
  /// **'Dossier temporaire (.part)'**
  String get settingsTempDirLabel;

  /// No description provided for @settingsTempDirHelper.
  ///
  /// In fr, this message translates to:
  /// **'Où sont stockés les fichiers en cours (eMule « Répertoire temporaire » ; vide = dossier du daemon)'**
  String get settingsTempDirHelper;

  /// No description provided for @settingsIchTitle.
  ///
  /// In fr, this message translates to:
  /// **'ICH — vérification intelligente des parts'**
  String get settingsIchTitle;

  /// No description provided for @settingsIchSub.
  ///
  /// In fr, this message translates to:
  /// **'Vérifie chaque part complétée par MD4 (eMule « ICH » ; désactiver saute la vérification intermédiaire)'**
  String get settingsIchSub;

  /// No description provided for @settingsRememberCancelledTitle.
  ///
  /// In fr, this message translates to:
  /// **'Mémoriser les fichiers annulés'**
  String get settingsRememberCancelledTitle;

  /// No description provided for @settingsRememberCancelledSub.
  ///
  /// In fr, this message translates to:
  /// **'Ajoute une entrée d\'historique à l\'annulation (eMule « Remember cancelled files »)'**
  String get settingsRememberCancelledSub;

  /// No description provided for @settingsRememberDownloadedTitle.
  ///
  /// In fr, this message translates to:
  /// **'Mémoriser les fichiers téléchargés'**
  String get settingsRememberDownloadedTitle;

  /// No description provided for @settingsRememberDownloadedSub.
  ///
  /// In fr, this message translates to:
  /// **'Ajoute une entrée d\'historique à la complétion (eMule « Remember downloaded files »)'**
  String get settingsRememberDownloadedSub;

  /// No description provided for @settingsDirEmptyError.
  ///
  /// In fr, this message translates to:
  /// **'Le chemin du dossier ne peut pas être vide.'**
  String get settingsDirEmptyError;

  /// No description provided for @settingsDirSaved.
  ///
  /// In fr, this message translates to:
  /// **'Dossier de téléchargement par défaut mis à jour.'**
  String get settingsDirSaved;

  /// No description provided for @settingsDirProbeFailed.
  ///
  /// In fr, this message translates to:
  /// **'Le test d\'écriture dans le dossier a échoué.'**
  String get settingsDirProbeFailed;

  /// No description provided for @settingsDirResetDone.
  ///
  /// In fr, this message translates to:
  /// **'Dossier portable par défaut restauré.'**
  String get settingsDirResetDone;

  /// No description provided for @settingsDirResetFailed.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la réinitialisation du dossier de téléchargement.'**
  String get settingsDirResetFailed;

  /// No description provided for @settingsCapPortsTitle.
  ///
  /// In fr, this message translates to:
  /// **'Capacités & ports'**
  String get settingsCapPortsTitle;

  /// No description provided for @settingsCapDlLabel.
  ///
  /// In fr, this message translates to:
  /// **'Capacité réception (Ko/s)'**
  String get settingsCapDlLabel;

  /// No description provided for @settingsCapDlHint.
  ///
  /// In fr, this message translates to:
  /// **'Vitesse de ligne descendante — borne la limite de download à 90 % (0 = aucune)'**
  String get settingsCapDlHint;

  /// No description provided for @settingsCapUlLabel.
  ///
  /// In fr, this message translates to:
  /// **'Capacité émission (Ko/s)'**
  String get settingsCapUlLabel;

  /// No description provided for @settingsCapUlHint.
  ///
  /// In fr, this message translates to:
  /// **'Vitesse de ligne montante — référence USS, borne la limite d\'upload à 80 % (0 = aucune)'**
  String get settingsCapUlHint;

  /// No description provided for @settingsTcpPortLabel.
  ///
  /// In fr, this message translates to:
  /// **'Port TCP'**
  String get settingsTcpPortLabel;

  /// No description provided for @settingsTcpPortHint.
  ///
  /// In fr, this message translates to:
  /// **'Port client eD2K — rebind immédiat du listener'**
  String get settingsTcpPortHint;

  /// No description provided for @settingsUdpPortLabel.
  ///
  /// In fr, this message translates to:
  /// **'Port UDP'**
  String get settingsUdpPortLabel;

  /// No description provided for @settingsUdpPortHint.
  ///
  /// In fr, this message translates to:
  /// **'Port Kad/UDP — socket rebind à chaud'**
  String get settingsUdpPortHint;

  /// No description provided for @settingsUdpEnabledTitle.
  ///
  /// In fr, this message translates to:
  /// **'UDP activé'**
  String get settingsUdpEnabledTitle;

  /// No description provided for @settingsUdpEnabledSub.
  ///
  /// In fr, this message translates to:
  /// **'Kad UDP, requêtes serveur UDP et re-ask UDP (eMule « UDP désactivé »)'**
  String get settingsUdpEnabledSub;

  /// No description provided for @settingsUpnpToggleTitle.
  ///
  /// In fr, this message translates to:
  /// **'Configurer les ports grâce à l\'UPnP'**
  String get settingsUpnpToggleTitle;

  /// No description provided for @settingsUpnpToggleSub.
  ///
  /// In fr, this message translates to:
  /// **'Mappe/démappe automatiquement TCP+UDP sur la box'**
  String get settingsUpnpToggleSub;

  /// No description provided for @settingsProtocolTitle.
  ///
  /// In fr, this message translates to:
  /// **'Protocole & limites'**
  String get settingsProtocolTitle;

  /// No description provided for @settingsObfEnabledTitle.
  ///
  /// In fr, this message translates to:
  /// **'Connexions obfusquées supportées'**
  String get settingsObfEnabledTitle;

  /// No description provided for @settingsObfEnabledSub.
  ///
  /// In fr, this message translates to:
  /// **'Chiffrement de protocole eD2K/Kad (eMule « Support obfuscated connections »)'**
  String get settingsObfEnabledSub;

  /// No description provided for @settingsObfRequiredTitle.
  ///
  /// In fr, this message translates to:
  /// **'Obfuscation requise'**
  String get settingsObfRequiredTitle;

  /// No description provided for @settingsObfRequiredSub.
  ///
  /// In fr, this message translates to:
  /// **'Rejette les pairs sans chiffrement (eMule « Require obfuscated connections »)'**
  String get settingsObfRequiredSub;

  /// No description provided for @settingsUssTitle.
  ///
  /// In fr, this message translates to:
  /// **'USS — Détection de la vitesse d\'envoi'**
  String get settingsUssTitle;

  /// No description provided for @settingsUssSub.
  ///
  /// In fr, this message translates to:
  /// **'Ajuste dynamiquement la limite d\'upload selon la latence mesurée'**
  String get settingsUssSub;

  /// No description provided for @settingsMaxConnLabel.
  ///
  /// In fr, this message translates to:
  /// **'Connexions max'**
  String get settingsMaxConnLabel;

  /// No description provided for @settingsMaxConnHint.
  ///
  /// In fr, this message translates to:
  /// **'Budget global de sockets pairs, 1–65000 (eMule MaxConnections — non borné)'**
  String get settingsMaxConnHint;

  /// No description provided for @settingsConnPerMinLabel.
  ///
  /// In fr, this message translates to:
  /// **'Connexions / min'**
  String get settingsConnPerMinLabel;

  /// No description provided for @settingsConnPerMinHint.
  ///
  /// In fr, this message translates to:
  /// **'Nouvelles connexions sortantes par minute, 1–1200 (eMule ≈ 240)'**
  String get settingsConnPerMinHint;

  /// No description provided for @settingsServersTitle.
  ///
  /// In fr, this message translates to:
  /// **'Serveurs eD2K'**
  String get settingsServersTitle;

  /// No description provided for @settingsServerReconnectTitle.
  ///
  /// In fr, this message translates to:
  /// **'Reconnexion automatique'**
  String get settingsServerReconnectTitle;

  /// No description provided for @settingsServerReconnectSub.
  ///
  /// In fr, this message translates to:
  /// **'Reconnecte au dernier serveur après une perte de connexion (eMule « Reconnect »)'**
  String get settingsServerReconnectSub;

  /// No description provided for @settingsServerAutoconnectTitle.
  ///
  /// In fr, this message translates to:
  /// **'Connexion au démarrage'**
  String get settingsServerAutoconnectTitle;

  /// No description provided for @settingsServerAutoconnectSub.
  ///
  /// In fr, this message translates to:
  /// **'Se connecte automatiquement au meilleur serveur connu au lancement (eMule « Autoconnect »)'**
  String get settingsServerAutoconnectSub;

  /// No description provided for @settingsServerAutoremoveTitle.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer les serveurs morts'**
  String get settingsServerAutoremoveTitle;

  /// No description provided for @settingsServerAutoremoveSub.
  ///
  /// In fr, this message translates to:
  /// **'Retire un serveur de la liste dès le premier échec de reconnexion (eMule « DeadServerRetry » = 1)'**
  String get settingsServerAutoremoveSub;

  /// No description provided for @settingsSafeConnectTitle.
  ///
  /// In fr, this message translates to:
  /// **'Connexion sûre'**
  String get settingsSafeConnectTitle;

  /// No description provided for @settingsSafeConnectSub.
  ///
  /// In fr, this message translates to:
  /// **'Bloque la reprise des downloads tant que ni le serveur eD2K ni Kad ne sont disponibles'**
  String get settingsSafeConnectSub;

  /// No description provided for @settingsServerAutoupdateTitle.
  ///
  /// In fr, this message translates to:
  /// **'Mettre à jour la liste au démarrage'**
  String get settingsServerAutoupdateTitle;

  /// No description provided for @settingsServerAutoupdateSub.
  ///
  /// In fr, this message translates to:
  /// **'Importe le fichier server.met ci-dessous au lancement du daemon'**
  String get settingsServerAutoupdateSub;

  /// No description provided for @settingsServerMetLabel.
  ///
  /// In fr, this message translates to:
  /// **'Fichier server.met'**
  String get settingsServerMetLabel;

  /// No description provided for @settingsServerMetHint.
  ///
  /// In fr, this message translates to:
  /// **'Chemin local du fichier .met importé au démarrage (aucun fetch HTTP)'**
  String get settingsServerMetHint;

  /// No description provided for @upnpConfiguredMsg.
  ///
  /// In fr, this message translates to:
  /// **'UPnP configuré (IP WAN : {ip}).'**
  String upnpConfiguredMsg(String ip);

  /// No description provided for @upnpNoGateway.
  ///
  /// In fr, this message translates to:
  /// **'Passerelle UPnP non détectée ou désactivée sur votre box.'**
  String get upnpNoGateway;

  /// No description provided for @upnpRefreshFailed.
  ///
  /// In fr, this message translates to:
  /// **'Échec de rafraîchissement UPnP : {error}'**
  String upnpRefreshFailed(String error);

  /// No description provided for @settingsInvalidNumMin.
  ///
  /// In fr, this message translates to:
  /// **'{label} : « {value} » invalide (entier ≥ {min} attendu)'**
  String settingsInvalidNumMin(String label, String value, int min);

  /// No description provided for @settingsInvalidNumRange.
  ///
  /// In fr, this message translates to:
  /// **'{label} : « {value} » invalide (entier entre {min} et {max} attendu)'**
  String settingsInvalidNumRange(String label, String value, int min, int max);

  /// No description provided for @killSwitchConfirmTitle.
  ///
  /// In fr, this message translates to:
  /// **'Couper toutes les connexions ?'**
  String get killSwitchConfirmTitle;

  /// No description provided for @killSwitchConfirmBody.
  ///
  /// In fr, this message translates to:
  /// **'Le kill switch coupe immédiatement toutes les sockets réseau (pairs, serveurs, listeners) jusqu\'à réactivation manuelle.'**
  String get killSwitchConfirmBody;

  /// No description provided for @killSwitchConfirmAction.
  ///
  /// In fr, this message translates to:
  /// **'Tout déconnecter'**
  String get killSwitchConfirmAction;

  /// No description provided for @settingsMaxGlobalLabel.
  ///
  /// In fr, this message translates to:
  /// **'Sources maximum (global)'**
  String get settingsMaxGlobalLabel;

  /// No description provided for @settingsMaxGlobalHint.
  ///
  /// In fr, this message translates to:
  /// **'Quota de sources activées par l\'allocateur swarm, tous fichiers confondus — distinct du budget de sockets (Connexion)'**
  String get settingsMaxGlobalHint;

  /// No description provided for @settingsMaxPerFileLabel.
  ///
  /// In fr, this message translates to:
  /// **'Sources maximum par fichier'**
  String get settingsMaxPerFileLabel;

  /// No description provided for @settingsMaxPerFileHint.
  ///
  /// In fr, this message translates to:
  /// **'Plafond par défaut — surchargeable par fichier (FT_MAXSOURCES)'**
  String get settingsMaxPerFileHint;

  /// No description provided for @settingsMinPerFileLabel.
  ///
  /// In fr, this message translates to:
  /// **'Sources minimum par fichier'**
  String get settingsMinPerFileLabel;

  /// No description provided for @settingsMinPerFileHint.
  ///
  /// In fr, this message translates to:
  /// **'Quota de sources garanti par fichier même si le swarm global est saturé'**
  String get settingsMinPerFileHint;

  /// No description provided for @settingsMaxQueueLabel.
  ///
  /// In fr, this message translates to:
  /// **'File d\'attente upload max'**
  String get settingsMaxQueueLabel;

  /// No description provided for @settingsMaxQueueHint.
  ///
  /// In fr, this message translates to:
  /// **'Clients en attente d\'un slot upload, 10–5000 (eMule QueueSize, défaut 5000)'**
  String get settingsMaxQueueHint;

  /// No description provided for @settingsMinUploadLabel.
  ///
  /// In fr, this message translates to:
  /// **'Débit upload min par slot (Ko/s)'**
  String get settingsMinUploadLabel;

  /// No description provided for @settingsMinUploadHint.
  ///
  /// In fr, this message translates to:
  /// **'Pas de nouveau slot si la moyenne/slot passe sous ce seuil ; 0 = désactivé (eMule MinUpload, défaut 1)'**
  String get settingsMinUploadHint;

  /// No description provided for @settingsDailyQuotaLabel.
  ///
  /// In fr, this message translates to:
  /// **'Quota upload quotidien (Gio, 0 = illimité)'**
  String get settingsDailyQuotaLabel;

  /// No description provided for @settingsDailyQuotaHint.
  ///
  /// In fr, this message translates to:
  /// **'Option eMule-Rust : au-delà du quota, plus aucun slot d\'upload n\'est accordé. Remise à zéro chaque jour (UTC).'**
  String get settingsDailyQuotaHint;

  /// No description provided for @settingsAutoPriorityTitle.
  ///
  /// In fr, this message translates to:
  /// **'Priorité auto des nouveaux downloads'**
  String get settingsAutoPriorityTitle;

  /// No description provided for @settingsAutoPrioritySub.
  ///
  /// In fr, this message translates to:
  /// **'DAPPref eMule : les nouveaux téléchargements utilisent la priorité automatique (le nombre de sources détermine basse/normale/haute)'**
  String get settingsAutoPrioritySub;

  /// No description provided for @settingsAutoUpPriorityTitle.
  ///
  /// In fr, this message translates to:
  /// **'Priorité auto des uploads'**
  String get settingsAutoUpPriorityTitle;

  /// No description provided for @settingsAutoUpPrioritySub.
  ///
  /// In fr, this message translates to:
  /// **'UAPPref eMule : les nouveaux fichiers partagés utilisent la priorité upload automatique (le nombre de clients en file détermine basse/normale/haute)'**
  String get settingsAutoUpPrioritySub;

  /// No description provided for @settingsSwarmHealingTitle.
  ///
  /// In fr, this message translates to:
  /// **'Sauvetage de swarm (swarm healing)'**
  String get settingsSwarmHealingTitle;

  /// No description provided for @settingsSwarmHealingSub.
  ///
  /// In fr, this message translates to:
  /// **'Option eMule-Rust : télécharge en priorité les parts les plus rares du swarm (peu de sources restantes) pour devenir une source de secours avant leur disparition'**
  String get settingsSwarmHealingSub;

  /// No description provided for @settingsExclusiveTitle.
  ///
  /// In fr, this message translates to:
  /// **'Options exclusives eMule-Rust'**
  String get settingsExclusiveTitle;

  /// No description provided for @settingsParanoidTitle.
  ///
  /// In fr, this message translates to:
  /// **'Mode paranoid'**
  String get settingsParanoidTitle;

  /// No description provided for @settingsParanoidSub.
  ///
  /// In fr, this message translates to:
  /// **'Ne sert que les pairs obfusqués dont la signature Secure Ident a été vérifiée — les clients eMule classiques sont refusés (réduit fortement la compatibilité)'**
  String get settingsParanoidSub;

  /// No description provided for @settingsIdentityRotationLabel.
  ///
  /// In fr, this message translates to:
  /// **'Rotation d\'identité (heures, 0 = désactivée)'**
  String get settingsIdentityRotationLabel;

  /// No description provided for @settingsIdentityRotationHelper.
  ///
  /// In fr, this message translates to:
  /// **'Régénère la clé RSA, le user hash eD2K et le KadID périodiquement. Invalide les crédits accumulés — identité volontairement éphémère.'**
  String get settingsIdentityRotationHelper;

  /// No description provided for @settingsInvalidLimits.
  ///
  /// In fr, this message translates to:
  /// **'Limites invalides : {fields}'**
  String settingsInvalidLimits(String fields);

  /// No description provided for @settingsLimitDlField.
  ///
  /// In fr, this message translates to:
  /// **'limite de réception (Ko/s)'**
  String get settingsLimitDlField;

  /// No description provided for @settingsLimitUlField.
  ///
  /// In fr, this message translates to:
  /// **'limite d\'émission (Ko/s)'**
  String get settingsLimitUlField;

  /// No description provided for @settingsLimitConnField.
  ///
  /// In fr, this message translates to:
  /// **'connexions/minute'**
  String get settingsLimitConnField;

  /// No description provided for @settingsIdentityRotationInvalid.
  ///
  /// In fr, this message translates to:
  /// **'Rotation d\'identité : « {value} » invalide (entier entre 0 et 8760 attendu)'**
  String settingsIdentityRotationInvalid(String value);

  /// No description provided for @rateLimitsCapacityNote.
  ///
  /// In fr, this message translates to:
  /// **'Ces plafonds brident le trafic réel (token buckets), bornés à 90 % (réception) / 80 % (émission) des capacités de ligne déclarées (onglet Connexion).'**
  String get rateLimitsCapacityNote;

  /// No description provided for @rateLimitsUssActiveNote.
  ///
  /// In fr, this message translates to:
  /// **'USS est actif : l\'envoi est piloté automatiquement selon la latence mesurée. La limite d\'envoi ci-dessous borne son plafond — 0 = aucun plafond.'**
  String get rateLimitsUssActiveNote;

  /// No description provided for @limitsEffective.
  ///
  /// In fr, this message translates to:
  /// **'Effectif actuel : {value} Ko/s'**
  String limitsEffective(int value);

  /// No description provided for @limitsEffectiveRatio.
  ///
  /// In fr, this message translates to:
  /// **'Plafonné par le ratio eD2K — effectif : {value} Ko/s'**
  String limitsEffectiveRatio(int value);

  /// No description provided for @limitsUssDriven.
  ///
  /// In fr, this message translates to:
  /// **'Piloté par USS — limite dynamique actuelle : {value} Ko/s. La valeur saisie borne le plafond USS.'**
  String limitsUssDriven(int value);

  /// No description provided for @settingsDiagnosticTile.
  ///
  /// In fr, this message translates to:
  /// **'Diagnostic du daemon'**
  String get settingsDiagnosticTile;

  /// No description provided for @settingsDiagnosticSub.
  ///
  /// In fr, this message translates to:
  /// **'État de santé, connexions et statistiques internes'**
  String get settingsDiagnosticSub;

  /// No description provided for @settingsImportPartTile.
  ///
  /// In fr, this message translates to:
  /// **'Importer un .part.met'**
  String get settingsImportPartTile;

  /// No description provided for @settingsImportPartSub.
  ///
  /// In fr, this message translates to:
  /// **'Reprendre un téléchargement eMule existant'**
  String get settingsImportPartSub;

  /// No description provided for @settingsMigrationTile.
  ///
  /// In fr, this message translates to:
  /// **'Migration depuis eMule'**
  String get settingsMigrationTile;

  /// No description provided for @settingsMigrationSub.
  ///
  /// In fr, this message translates to:
  /// **'Importer les fichiers de config eMule (amis, crédits, identité…)'**
  String get settingsMigrationSub;

  /// No description provided for @migrationDialogTitle.
  ///
  /// In fr, this message translates to:
  /// **'Migration depuis eMule'**
  String get migrationDialogTitle;

  /// No description provided for @migrationConfigDirLabel.
  ///
  /// In fr, this message translates to:
  /// **'Dossier de configuration eMule'**
  String get migrationConfigDirLabel;

  /// No description provided for @migrationConfigDirHint.
  ///
  /// In fr, this message translates to:
  /// **'ex. C:\\Users\\…\\AppData\\Local\\eMule'**
  String get migrationConfigDirHint;

  /// No description provided for @migrationIncomingLabel.
  ///
  /// In fr, this message translates to:
  /// **'Dossier Incoming (pour known.met)'**
  String get migrationIncomingLabel;

  /// No description provided for @migrationIncomingHint.
  ///
  /// In fr, this message translates to:
  /// **'Laisser vide pour le dossier de téléchargement actuel'**
  String get migrationIncomingHint;

  /// No description provided for @migrationNoneDetected.
  ///
  /// In fr, this message translates to:
  /// **'Aucun fichier eMule détecté dans ce dossier'**
  String get migrationNoneDetected;

  /// No description provided for @migrationSelectAll.
  ///
  /// In fr, this message translates to:
  /// **'Tout sélectionner'**
  String get migrationSelectAll;

  /// No description provided for @migrationRunSelected.
  ///
  /// In fr, this message translates to:
  /// **'Importer la sélection'**
  String get migrationRunSelected;

  /// No description provided for @migrationRunning.
  ///
  /// In fr, this message translates to:
  /// **'Import en cours…'**
  String get migrationRunning;

  /// No description provided for @migrationReplaceIdentity.
  ///
  /// In fr, this message translates to:
  /// **'Remplacer l\'identité actuelle'**
  String get migrationReplaceIdentity;

  /// No description provided for @migrationReplaceIdentityWarn.
  ///
  /// In fr, this message translates to:
  /// **'Remplace la clé RSA et le hash utilisateur actuels — les crédits accumulés sous l\'identité courante seront perdus.'**
  String get migrationReplaceIdentityWarn;

  /// No description provided for @migrationSourceKnownMet.
  ///
  /// In fr, this message translates to:
  /// **'Fichiers connus (partages)'**
  String get migrationSourceKnownMet;

  /// No description provided for @migrationSourceServerMet.
  ///
  /// In fr, this message translates to:
  /// **'Liste de serveurs'**
  String get migrationSourceServerMet;

  /// No description provided for @migrationSourceNodesDat.
  ///
  /// In fr, this message translates to:
  /// **'Contacts Kad'**
  String get migrationSourceNodesDat;

  /// No description provided for @migrationSourceEmfriends.
  ///
  /// In fr, this message translates to:
  /// **'Amis'**
  String get migrationSourceEmfriends;

  /// No description provided for @migrationSourceCancelled.
  ///
  /// In fr, this message translates to:
  /// **'Fichiers annulés'**
  String get migrationSourceCancelled;

  /// No description provided for @migrationSourceClients.
  ///
  /// In fr, this message translates to:
  /// **'Crédits des pairs'**
  String get migrationSourceClients;

  /// No description provided for @migrationSourceIdentity.
  ///
  /// In fr, this message translates to:
  /// **'Identité (clé RSA + hash)'**
  String get migrationSourceIdentity;

  /// No description provided for @migrationSummaryImported.
  ///
  /// In fr, this message translates to:
  /// **'{imported}/{total} importés'**
  String migrationSummaryImported(Object imported, Object total);

  /// No description provided for @migrationSummaryImportedOnly.
  ///
  /// In fr, this message translates to:
  /// **'{n} importés'**
  String migrationSummaryImportedOnly(Object n);

  /// No description provided for @migrationSummaryRecords.
  ///
  /// In fr, this message translates to:
  /// **'{n} entrées'**
  String migrationSummaryRecords(Object n);

  /// No description provided for @migrationSummaryEnriched.
  ///
  /// In fr, this message translates to:
  /// **'{n} enrichis'**
  String migrationSummaryEnriched(Object n);

  /// No description provided for @migrationSummaryUnmatched.
  ///
  /// In fr, this message translates to:
  /// **'{n} sans correspondance'**
  String migrationSummaryUnmatched(Object n);

  /// No description provided for @migrationSummaryUpdated.
  ///
  /// In fr, this message translates to:
  /// **'{n} mis à jour'**
  String migrationSummaryUpdated(Object n);

  /// No description provided for @migrationSummarySkipped.
  ///
  /// In fr, this message translates to:
  /// **'{n} ignorés'**
  String migrationSummarySkipped(Object n);

  /// No description provided for @migrationSummaryExpired.
  ///
  /// In fr, this message translates to:
  /// **'{n} expirés'**
  String migrationSummaryExpired(Object n);

  /// No description provided for @migrationSummaryTotal.
  ///
  /// In fr, this message translates to:
  /// **'{n} au total'**
  String migrationSummaryTotal(Object n);

  /// No description provided for @migrationSummaryIdentityKey.
  ///
  /// In fr, this message translates to:
  /// **'clé RSA'**
  String get migrationSummaryIdentityKey;

  /// No description provided for @migrationSummaryIdentityHash.
  ///
  /// In fr, this message translates to:
  /// **'hash utilisateur'**
  String get migrationSummaryIdentityHash;

  /// No description provided for @migrationSummaryEmpty.
  ///
  /// In fr, this message translates to:
  /// **'Importé'**
  String get migrationSummaryEmpty;

  /// No description provided for @settingsLabGuideTile.
  ///
  /// In fr, this message translates to:
  /// **'Guide du mode laboratoire'**
  String get settingsLabGuideTile;

  /// No description provided for @settingsLabGuideSub.
  ///
  /// In fr, this message translates to:
  /// **'Scénarios de test loopback sans trafic WAN'**
  String get settingsLabGuideSub;

  /// No description provided for @settingsDebugBarTitle.
  ///
  /// In fr, this message translates to:
  /// **'Barre de simulation debug'**
  String get settingsDebugBarTitle;

  /// No description provided for @settingsDebugBarOn.
  ///
  /// In fr, this message translates to:
  /// **'Affiche la barre de simulation en bas de l\'écran'**
  String get settingsDebugBarOn;

  /// No description provided for @settingsDebugBarOff.
  ///
  /// In fr, this message translates to:
  /// **'Disponible uniquement avec les dev tools (ENABLE_DEV_TOOLS ou fake daemon)'**
  String get settingsDebugBarOff;

  /// No description provided for @settingsDaemonVersion.
  ///
  /// In fr, this message translates to:
  /// **'Daemon {version} — protocole {protocol}'**
  String settingsDaemonVersion(String version, String protocol);

  /// No description provided for @settingsAddFriendTitle.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter un ami'**
  String get settingsAddFriendTitle;

  /// No description provided for @settingsUserHashLabel.
  ///
  /// In fr, this message translates to:
  /// **'UserHash (hex)'**
  String get settingsUserHashLabel;

  /// No description provided for @settingsUserHashHint.
  ///
  /// In fr, this message translates to:
  /// **'32 caractères hexadécimaux'**
  String get settingsUserHashHint;

  /// No description provided for @settingsFriendNameLabel.
  ///
  /// In fr, this message translates to:
  /// **'Nom (optionnel)'**
  String get settingsFriendNameLabel;

  /// No description provided for @settingsNoFriends.
  ///
  /// In fr, this message translates to:
  /// **'Aucun ami configuré.'**
  String get settingsNoFriends;

  /// No description provided for @settingsRemoveFriendTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Retirer'**
  String get settingsRemoveFriendTooltip;

  /// No description provided for @settingsRetentionDaysCustom.
  ///
  /// In fr, this message translates to:
  /// **'{days} jours'**
  String settingsRetentionDaysCustom(int days);

  /// No description provided for @settingsDirCustomHelper.
  ///
  /// In fr, this message translates to:
  /// **'Dossier personnalisé (persisté en base, appliqué immédiatement)'**
  String get settingsDirCustomHelper;

  /// No description provided for @settingsDirPortableHelper.
  ///
  /// In fr, this message translates to:
  /// **'Dossier portable par défaut (<racine_du_bundle>/downloads)'**
  String get settingsDirPortableHelper;

  /// No description provided for @netBannerKillActive.
  ///
  /// In fr, this message translates to:
  /// **'Kill switch actif : toutes les sockets sont coupées, connexions entrantes et sortantes bloquées.'**
  String get netBannerKillActive;

  /// No description provided for @netBannerPolicyActive.
  ///
  /// In fr, this message translates to:
  /// **'Chaque socket est soumise aux contraintes du profil réseau actif (permis, budgets, filtre IP).'**
  String get netBannerPolicyActive;

  /// No description provided for @upnpGatewayConnected.
  ///
  /// In fr, this message translates to:
  /// **'Connectée'**
  String get upnpGatewayConnected;

  /// No description provided for @upnpGatewayAbsent.
  ///
  /// In fr, this message translates to:
  /// **'Non détectée / Désactivée'**
  String get upnpGatewayAbsent;

  /// No description provided for @upnpIpUnknown.
  ///
  /// In fr, this message translates to:
  /// **'Inconnue'**
  String get upnpIpUnknown;

  /// No description provided for @upnpMappedHighId.
  ///
  /// In fr, this message translates to:
  /// **'Mappé (HighID)'**
  String get upnpMappedHighId;

  /// No description provided for @upnpMappedOpen.
  ///
  /// In fr, this message translates to:
  /// **'Mappé (Ouvert)'**
  String get upnpMappedOpen;

  /// No description provided for @upnpNotMapped.
  ///
  /// In fr, this message translates to:
  /// **'Non mappé'**
  String get upnpNotMapped;

  /// No description provided for @ipFilterActive.
  ///
  /// In fr, this message translates to:
  /// **'Filtre actif'**
  String get ipFilterActive;

  /// No description provided for @ipFilterDisabled.
  ///
  /// In fr, this message translates to:
  /// **'Filtre désactivé'**
  String get ipFilterDisabled;

  /// No description provided for @networksCardTitle.
  ///
  /// In fr, this message translates to:
  /// **'Réseaux'**
  String get networksCardTitle;

  /// No description provided for @networkEd2kTitle.
  ///
  /// In fr, this message translates to:
  /// **'Réseau eD2K'**
  String get networkEd2kTitle;

  /// No description provided for @networkKadTitle.
  ///
  /// In fr, this message translates to:
  /// **'Réseau Kad'**
  String get networkKadTitle;

  /// No description provided for @netProfileEd2kDesc.
  ///
  /// In fr, this message translates to:
  /// **'TCP sortant vers des pairs/serveurs eD2K publics. Rejette le LAN privé & la boucle locale (anti-SSRF).'**
  String get netProfileEd2kDesc;

  /// No description provided for @netProfileKadDesc.
  ///
  /// In fr, this message translates to:
  /// **'DHT Kademlia — UDP sortant pour le routage WAN. Activer Kad active aussi eD2K.'**
  String get netProfileKadDesc;

  /// No description provided for @ipReportTotalLines.
  ///
  /// In fr, this message translates to:
  /// **'Lignes lues : {count}'**
  String ipReportTotalLines(int count);

  /// No description provided for @ipReportValidEntries.
  ///
  /// In fr, this message translates to:
  /// **'Entrées valides : {count}'**
  String ipReportValidEntries(int count);

  /// No description provided for @ipReportCoalesced.
  ///
  /// In fr, this message translates to:
  /// **'Plages fusionnées : {count}'**
  String ipReportCoalesced(int count);

  /// No description provided for @ipReportFinalRanges.
  ///
  /// In fr, this message translates to:
  /// **'Intervalles actifs finaux : {count}'**
  String ipReportFinalRanges(int count);

  /// No description provided for @connDisconnected.
  ///
  /// In fr, this message translates to:
  /// **'Déconnecté'**
  String get connDisconnected;

  /// No description provided for @connConnecting.
  ///
  /// In fr, this message translates to:
  /// **'Connexion'**
  String get connConnecting;

  /// No description provided for @connConnectingEllipsis.
  ///
  /// In fr, this message translates to:
  /// **'Connexion...'**
  String get connConnectingEllipsis;

  /// No description provided for @connConnected.
  ///
  /// In fr, this message translates to:
  /// **'Connecté'**
  String get connConnected;

  /// No description provided for @connDisconnecting.
  ///
  /// In fr, this message translates to:
  /// **'Déconnexion...'**
  String get connDisconnecting;

  /// No description provided for @connError.
  ///
  /// In fr, this message translates to:
  /// **'Erreur'**
  String get connError;

  /// No description provided for @peerHandshaking.
  ///
  /// In fr, this message translates to:
  /// **'Handshake eD2K...'**
  String get peerHandshaking;

  /// No description provided for @peerQueued.
  ///
  /// In fr, this message translates to:
  /// **'En file d\'attente'**
  String get peerQueued;

  /// No description provided for @peerTransferring.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargement'**
  String get peerTransferring;

  /// No description provided for @dlStatusQueued.
  ///
  /// In fr, this message translates to:
  /// **'En file'**
  String get dlStatusQueued;

  /// No description provided for @dlStatusWaitingForSources.
  ///
  /// In fr, this message translates to:
  /// **'Recherche de sources'**
  String get dlStatusWaitingForSources;

  /// No description provided for @dlStatusAllocating.
  ///
  /// In fr, this message translates to:
  /// **'Allocation'**
  String get dlStatusAllocating;

  /// No description provided for @dlStatusDownloading.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargement'**
  String get dlStatusDownloading;

  /// No description provided for @dlStatusPaused.
  ///
  /// In fr, this message translates to:
  /// **'En pause'**
  String get dlStatusPaused;

  /// No description provided for @dlStatusVerifying.
  ///
  /// In fr, this message translates to:
  /// **'Vérification'**
  String get dlStatusVerifying;

  /// No description provided for @dlStatusCompleting.
  ///
  /// In fr, this message translates to:
  /// **'Finalisation'**
  String get dlStatusCompleting;

  /// No description provided for @dlStatusCompleted.
  ///
  /// In fr, this message translates to:
  /// **'Terminé'**
  String get dlStatusCompleted;

  /// No description provided for @dlStatusFailed.
  ///
  /// In fr, this message translates to:
  /// **'Échoué'**
  String get dlStatusFailed;

  /// No description provided for @profileLocalLab.
  ///
  /// In fr, this message translates to:
  /// **'Lab local (loopback uniquement)'**
  String get profileLocalLab;

  /// No description provided for @profilePublicEd2k.
  ///
  /// In fr, this message translates to:
  /// **'eD2K public (preview)'**
  String get profilePublicEd2k;

  /// No description provided for @profilePublicEd2kKad.
  ///
  /// In fr, this message translates to:
  /// **'eD2K + Kad publics (preview)'**
  String get profilePublicEd2kKad;

  /// No description provided for @subsystemDisabled.
  ///
  /// In fr, this message translates to:
  /// **'Désactivé (kill switch)'**
  String get subsystemDisabled;

  /// No description provided for @subsystemReady.
  ///
  /// In fr, this message translates to:
  /// **'Prêt (preview)'**
  String get subsystemReady;

  /// No description provided for @subsystemShuttingDown.
  ///
  /// In fr, this message translates to:
  /// **'Arrêt en cours'**
  String get subsystemShuttingDown;

  /// No description provided for @daemonIdle.
  ///
  /// In fr, this message translates to:
  /// **'Inactif'**
  String get daemonIdle;

  /// No description provided for @daemonDiscovering.
  ///
  /// In fr, this message translates to:
  /// **'Recherche du daemon...'**
  String get daemonDiscovering;

  /// No description provided for @daemonStarting.
  ///
  /// In fr, this message translates to:
  /// **'Démarrage du daemon...'**
  String get daemonStarting;

  /// No description provided for @daemonProtocolMismatch.
  ///
  /// In fr, this message translates to:
  /// **'Version de protocole incompatible'**
  String get daemonProtocolMismatch;

  /// No description provided for @daemonAuthError.
  ///
  /// In fr, this message translates to:
  /// **'Échec d\'authentification'**
  String get daemonAuthError;

  /// No description provided for @navShares.
  ///
  /// In fr, this message translates to:
  /// **'Partages'**
  String get navShares;

  /// No description provided for @badgeEd2kHighId.
  ///
  /// In fr, this message translates to:
  /// **'eD2K · HighID'**
  String get badgeEd2kHighId;

  /// No description provided for @badgeEd2kLowId.
  ///
  /// In fr, this message translates to:
  /// **'eD2K · LowID'**
  String get badgeEd2kLowId;

  /// No description provided for @badgeEd2kConnecting.
  ///
  /// In fr, this message translates to:
  /// **'eD2K · Connexion…'**
  String get badgeEd2kConnecting;

  /// No description provided for @badgeEd2kDisconnecting.
  ///
  /// In fr, this message translates to:
  /// **'eD2K · Déconnexion…'**
  String get badgeEd2kDisconnecting;

  /// No description provided for @badgeEd2kDisconnected.
  ///
  /// In fr, this message translates to:
  /// **'eD2K · Déconnecté'**
  String get badgeEd2kDisconnected;

  /// No description provided for @badgeKadStopped.
  ///
  /// In fr, this message translates to:
  /// **'Kad · Arrêté'**
  String get badgeKadStopped;

  /// No description provided for @badgeKadConnected.
  ///
  /// In fr, this message translates to:
  /// **'Kad · Connecté'**
  String get badgeKadConnected;

  /// No description provided for @badgeKadBootstrapping.
  ///
  /// In fr, this message translates to:
  /// **'Kad · Amorçage'**
  String get badgeKadBootstrapping;

  /// No description provided for @daemonCardTooltipTitle.
  ///
  /// In fr, this message translates to:
  /// **'État du daemon'**
  String get daemonCardTooltipTitle;

  /// No description provided for @daemonCardTooltipServer.
  ///
  /// In fr, this message translates to:
  /// **'Serveur : {name}'**
  String daemonCardTooltipServer(Object name);

  /// No description provided for @daemonCardTooltipDiagnostics.
  ///
  /// In fr, this message translates to:
  /// **'Cliquer pour voir les diagnostics'**
  String get daemonCardTooltipDiagnostics;

  /// No description provided for @sharesTitle.
  ///
  /// In fr, this message translates to:
  /// **'Fichiers partagés'**
  String get sharesTitle;

  /// No description provided for @sharesFileCount.
  ///
  /// In fr, this message translates to:
  /// **'{count} fichier(s)'**
  String sharesFileCount(Object count);

  /// No description provided for @sharesRescanTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Rescanner le dossier Incoming'**
  String get sharesRescanTooltip;

  /// No description provided for @sharesEmptyState.
  ///
  /// In fr, this message translates to:
  /// **'Aucun fichier partagé.\nLes fichiers du dossier Incoming et les téléchargements en cours ou terminés sont partagés automatiquement.'**
  String get sharesEmptyState;

  /// No description provided for @sharesPartialParts.
  ///
  /// In fr, this message translates to:
  /// **' — partiel ({count} part(s))'**
  String sharesPartialParts(Object count);

  /// No description provided for @sharesTotalSentTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Total envoyé pour ce fichier'**
  String get sharesTotalSentTooltip;

  /// No description provided for @sharePrioVeryLow.
  ///
  /// In fr, this message translates to:
  /// **'Très basse'**
  String get sharePrioVeryLow;

  /// No description provided for @sharePrioVeryHigh.
  ///
  /// In fr, this message translates to:
  /// **'Très haute'**
  String get sharePrioVeryHigh;

  /// No description provided for @sharesEnabledTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Partagé'**
  String get sharesEnabledTooltip;

  /// No description provided for @sharesDisabledTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Suspendu'**
  String get sharesDisabledTooltip;

  /// No description provided for @sharesRenameDisabledTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargement en cours — renommez-le depuis l\'onglet Téléchargements'**
  String get sharesRenameDisabledTooltip;

  /// No description provided for @sharesRenameTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Renommer le fichier'**
  String get sharesRenameTooltip;

  /// No description provided for @sharesDeleteDisabledTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargement en cours — supprimez-le depuis l\'onglet Téléchargements'**
  String get sharesDeleteDisabledTooltip;

  /// No description provided for @sharesDeleteTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer le fichier du disque'**
  String get sharesDeleteTooltip;

  /// No description provided for @sharesDeleteTitle.
  ///
  /// In fr, this message translates to:
  /// **'Supprimer le fichier ?'**
  String get sharesDeleteTitle;

  /// No description provided for @sharesDeleteBody.
  ///
  /// In fr, this message translates to:
  /// **'{name} sera définitivement supprimé du disque et retiré des partages.'**
  String sharesDeleteBody(Object name);

  /// No description provided for @sourcesMaxDialogTitle.
  ///
  /// In fr, this message translates to:
  /// **'Sources max (FT_MAXSOURCES)'**
  String get sourcesMaxDialogTitle;

  /// No description provided for @sourcesMaxHint.
  ///
  /// In fr, this message translates to:
  /// **'Vide = limite globale'**
  String get sourcesMaxHint;

  /// No description provided for @sourcesMaxHelper.
  ///
  /// In fr, this message translates to:
  /// **'Nombre max de sources pour ce fichier (1-10000)'**
  String get sourcesMaxHelper;

  /// No description provided for @apply.
  ///
  /// In fr, this message translates to:
  /// **'Appliquer'**
  String get apply;

  /// No description provided for @srcNoHash.
  ///
  /// In fr, this message translates to:
  /// **'Pas de hash eD2K pour ce fichier.'**
  String get srcNoHash;

  /// No description provided for @srcLoading.
  ///
  /// In fr, this message translates to:
  /// **'Chargement des sources...'**
  String get srcLoading;

  /// No description provided for @srcNone.
  ///
  /// In fr, this message translates to:
  /// **'Aucune source enregistrée pour ce fichier.'**
  String get srcNone;

  /// No description provided for @srcConnected.
  ///
  /// In fr, this message translates to:
  /// **'Connectés ({count})'**
  String srcConnected(Object count);

  /// No description provided for @srcKnown.
  ///
  /// In fr, this message translates to:
  /// **'Sources connues ({count})'**
  String srcKnown(Object count);

  /// No description provided for @srcCount.
  ///
  /// In fr, this message translates to:
  /// **'Sources ({count})'**
  String srcCount(Object count);

  /// No description provided for @srcPriorityLabel.
  ///
  /// In fr, this message translates to:
  /// **'Priorité : {priority}'**
  String srcPriorityLabel(Object priority);

  /// No description provided for @srcMaxTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Sources max pour ce fichier (FT_MAXSOURCES)'**
  String get srcMaxTooltip;

  /// No description provided for @srcMaxShort.
  ///
  /// In fr, this message translates to:
  /// **'Src max : {value}'**
  String srcMaxShort(Object value);

  /// No description provided for @peerRetry.
  ///
  /// In fr, this message translates to:
  /// **'Réessayer'**
  String get peerRetry;

  /// No description provided for @peerConnect.
  ///
  /// In fr, this message translates to:
  /// **'Connecter'**
  String get peerConnect;

  /// No description provided for @pendingDestination.
  ///
  /// In fr, this message translates to:
  /// **'Destination en attente...'**
  String get pendingDestination;

  /// No description provided for @pendingCalculation.
  ///
  /// In fr, this message translates to:
  /// **'Calcul en cours...'**
  String get pendingCalculation;

  /// No description provided for @tooltipDownloadPriority.
  ///
  /// In fr, this message translates to:
  /// **'Priorité du téléchargement'**
  String get tooltipDownloadPriority;

  /// No description provided for @tooltipAssignCategory.
  ///
  /// In fr, this message translates to:
  /// **'Assigner une catégorie'**
  String get tooltipAssignCategory;

  /// No description provided for @tooltipCategoryName.
  ///
  /// In fr, this message translates to:
  /// **'Catégorie : {name}'**
  String tooltipCategoryName(Object name);

  /// No description provided for @peerSrcAllowed.
  ///
  /// In fr, this message translates to:
  /// **'Autorisé'**
  String get peerSrcAllowed;

  /// No description provided for @peerSrcFiltered.
  ///
  /// In fr, this message translates to:
  /// **'Filtré IP'**
  String get peerSrcFiltered;

  /// No description provided for @peerSrcRejected.
  ///
  /// In fr, this message translates to:
  /// **'Rejeté profil'**
  String get peerSrcRejected;

  /// No description provided for @peerSrcTried.
  ///
  /// In fr, this message translates to:
  /// **'Essayé'**
  String get peerSrcTried;

  /// No description provided for @peerSrcFailed.
  ///
  /// In fr, this message translates to:
  /// **'Échec'**
  String get peerSrcFailed;

  /// No description provided for @peerSrcCandidate.
  ///
  /// In fr, this message translates to:
  /// **'Candidat'**
  String get peerSrcCandidate;

  /// No description provided for @importPartOnDisk.
  ///
  /// In fr, this message translates to:
  /// **'Fichier .part sur le disque :'**
  String get importPartOnDisk;

  /// No description provided for @importPartPresent.
  ///
  /// In fr, this message translates to:
  /// **'Présent ({size})'**
  String importPartPresent(Object size);

  /// No description provided for @importPartMissing.
  ///
  /// In fr, this message translates to:
  /// **'MANQUANT (fichier .part introuvable)'**
  String get importPartMissing;

  /// No description provided for @openFileExplorerTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Ouvrir le fichier terminé dans l\'explorateur'**
  String get openFileExplorerTooltip;

  /// No description provided for @openFileDisabledTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Disponible une fois le téléchargement et la vérification terminés'**
  String get openFileDisabledTooltip;

  /// No description provided for @addLinkDuplicate.
  ///
  /// In fr, this message translates to:
  /// **'Ce fichier est déjà dans la liste des téléchargements (\"{name}\")'**
  String addLinkDuplicate(Object name);

  /// No description provided for @pickDownloadDir.
  ///
  /// In fr, this message translates to:
  /// **'Sélectionner le dossier de téléchargement'**
  String get pickDownloadDir;

  /// No description provided for @addLinkGenericError.
  ///
  /// In fr, this message translates to:
  /// **'Erreur lors de l\'ajout du téléchargement'**
  String get addLinkGenericError;

  /// No description provided for @catPickDir.
  ///
  /// In fr, this message translates to:
  /// **'Sélectionner le dossier de téléchargement par défaut de la catégorie'**
  String get catPickDir;

  /// No description provided for @catNameRequired.
  ///
  /// In fr, this message translates to:
  /// **'Le nom de la catégorie ne peut pas être vide'**
  String get catNameRequired;

  /// No description provided for @catEditTitle.
  ///
  /// In fr, this message translates to:
  /// **'Modifier la catégorie'**
  String get catEditTitle;

  /// No description provided for @catNewTitle.
  ///
  /// In fr, this message translates to:
  /// **'Nouvelle catégorie'**
  String get catNewTitle;

  /// No description provided for @catNameLabel.
  ///
  /// In fr, this message translates to:
  /// **'Nom de la catégorie *'**
  String get catNameLabel;

  /// No description provided for @catColorLabel.
  ///
  /// In fr, this message translates to:
  /// **'Étiquette de couleur'**
  String get catColorLabel;

  /// No description provided for @catDestLabel.
  ///
  /// In fr, this message translates to:
  /// **'Destination par défaut (optionnel)'**
  String get catDestLabel;

  /// No description provided for @catPrioLabel.
  ///
  /// In fr, this message translates to:
  /// **'Priorité par défaut (optionnel)'**
  String get catPrioLabel;

  /// No description provided for @catSaveChanges.
  ///
  /// In fr, this message translates to:
  /// **'Enregistrer les modifications'**
  String get catSaveChanges;

  /// No description provided for @catCreate.
  ///
  /// In fr, this message translates to:
  /// **'Créer la catégorie'**
  String get catCreate;

  /// No description provided for @opFailed.
  ///
  /// In fr, this message translates to:
  /// **'L\'opération a échoué'**
  String get opFailed;

  /// No description provided for @uploadsScore.
  ///
  /// In fr, this message translates to:
  /// **'Score'**
  String get uploadsScore;

  /// No description provided for @uploadsCredit.
  ///
  /// In fr, this message translates to:
  /// **'Crédit'**
  String get uploadsCredit;

  /// No description provided for @uploadsBanned.
  ///
  /// In fr, this message translates to:
  /// **'Client banni (score 0)'**
  String get uploadsBanned;

  /// No description provided for @kadVerified.
  ///
  /// In fr, this message translates to:
  /// **'Vérifié'**
  String get kadVerified;

  /// No description provided for @kadNotVerified.
  ///
  /// In fr, this message translates to:
  /// **'Non vérifié'**
  String get kadNotVerified;

  /// No description provided for @kadBootstrapDescPublic.
  ///
  /// In fr, this message translates to:
  /// **'Entrez l\'adresse IP publique et le port UDP d\'un nœud Kad pour amorcer la table de routage (ex : 212.83.184.225).'**
  String get kadBootstrapDescPublic;

  /// No description provided for @kadBootstrapDescLab.
  ///
  /// In fr, this message translates to:
  /// **'Entrez l\'adresse IP et le port UDP d\'un nœud d\'amorçage (en profil LocalLab, seul 127.0.0.1 est permis).'**
  String get kadBootstrapDescLab;

  /// No description provided for @kadStartUdpPublic.
  ///
  /// In fr, this message translates to:
  /// **'Démarrer UDP Kad'**
  String get kadStartUdpPublic;

  /// No description provided for @kadStartUdpLab.
  ///
  /// In fr, this message translates to:
  /// **'Démarrer UDP Loopback'**
  String get kadStartUdpLab;

  /// No description provided for @kadUdpActive.
  ///
  /// In fr, this message translates to:
  /// **'Actif ({addr})'**
  String kadUdpActive(Object addr);

  /// No description provided for @kadUdpInactive.
  ///
  /// In fr, this message translates to:
  /// **'Inactif'**
  String get kadUdpInactive;

  /// No description provided for @kadTimeNever.
  ///
  /// In fr, this message translates to:
  /// **'Jamais'**
  String get kadTimeNever;

  /// No description provided for @kadTimeMinutesAgo.
  ///
  /// In fr, this message translates to:
  /// **'Il y a {min}m'**
  String kadTimeMinutesAgo(Object min);

  /// No description provided for @kadTimeHoursAgo.
  ///
  /// In fr, this message translates to:
  /// **'Il y a {hours}h'**
  String kadTimeHoursAgo(Object hours);

  /// No description provided for @kadTimeDaysAgo.
  ///
  /// In fr, this message translates to:
  /// **'Il y a {days}j'**
  String kadTimeDaysAgo(Object days);

  /// No description provided for @kadBannerPublic.
  ///
  /// In fr, this message translates to:
  /// **'Réseau Kad DHT mondial actif sur {addr} | Routage Kademlia ouvert | Protection anti-SSRF active.'**
  String kadBannerPublic(Object addr);

  /// No description provided for @kadBannerLab.
  ///
  /// In fr, this message translates to:
  /// **'Socket UDP actif : {addr} (mode laboratoire confiné) | Kill switch instantané.'**
  String kadBannerLab(Object addr);

  /// No description provided for @kadBannerStartPublic.
  ///
  /// In fr, this message translates to:
  /// **'Transport UDP Kad : cliquez sur « Démarrer UDP » pour vous connecter au réseau Kademlia mondial.'**
  String get kadBannerStartPublic;

  /// No description provided for @kadBannerStartLab.
  ///
  /// In fr, this message translates to:
  /// **'Transport UDP Kad confiné : seul le trafic loopback (127.0.0.1) est autorisé dans le profil LocalLab.'**
  String get kadBannerStartLab;

  /// No description provided for @searchExplainKad.
  ///
  /// In fr, this message translates to:
  /// **'Les requêtes KADEMLIA2_SEARCH_KEYWORD interrogent les nœuds Kad stockant l\'index du mot-clé MD4.'**
  String get searchExplainKad;

  /// No description provided for @searchExplainGlobal.
  ///
  /// In fr, this message translates to:
  /// **'Les requêtes OP_GLOBSEARCHREQ3/REQ2 sont envoyées en UDP à tous les serveurs connus (port TCP+4). REQ3 est utilisé quand le serveur annonce EXT_GETFILES + LARGEFILES.'**
  String get searchExplainGlobal;

  /// No description provided for @searchExplainServer.
  ///
  /// In fr, this message translates to:
  /// **'Les requêtes OP_SEARCHREQUEST interrogent directement l\'index du serveur eD2K connecté.'**
  String get searchExplainServer;

  /// No description provided for @searchNoResults.
  ///
  /// In fr, this message translates to:
  /// **'Aucun résultat trouvé pour « {term} »'**
  String searchNoResults(Object term);

  /// No description provided for @searchPromptKeywords.
  ///
  /// In fr, this message translates to:
  /// **'Entrez des mots-clés ci-dessus pour lancer une recherche'**
  String get searchPromptKeywords;

  /// No description provided for @searchEnableTarget.
  ///
  /// In fr, this message translates to:
  /// **'Activez {target} pour commencer à chercher des fichiers'**
  String searchEnableTarget(Object target);

  /// No description provided for @searchTryOther.
  ///
  /// In fr, this message translates to:
  /// **'Essayez avec d\'autres termes ou élargissez vos filtres de taille'**
  String get searchTryOther;

  /// No description provided for @searchHideFilters.
  ///
  /// In fr, this message translates to:
  /// **'Masquer les filtres'**
  String get searchHideFilters;

  /// No description provided for @searchShowFilters.
  ///
  /// In fr, this message translates to:
  /// **'Afficher les filtres avancés'**
  String get searchShowFilters;

  /// No description provided for @searchHintKad.
  ///
  /// In fr, this message translates to:
  /// **'Mots-clés Kad (min. 3 lettres)...'**
  String get searchHintKad;

  /// No description provided for @searchHintServer.
  ///
  /// In fr, this message translates to:
  /// **'Nom de fichier ou mots-clés...'**
  String get searchHintServer;

  /// No description provided for @searchButtonSearching.
  ///
  /// In fr, this message translates to:
  /// **'Recherche...'**
  String get searchButtonSearching;

  /// No description provided for @searchButton.
  ///
  /// In fr, this message translates to:
  /// **'Rechercher'**
  String get searchButton;

  /// No description provided for @searchCancelledBadge.
  ///
  /// In fr, this message translates to:
  /// **'Annulé'**
  String get searchCancelledBadge;

  /// No description provided for @searchTargetServer.
  ///
  /// In fr, this message translates to:
  /// **'Serveur eD2K'**
  String get searchTargetServer;

  /// No description provided for @searchTargetKad.
  ///
  /// In fr, this message translates to:
  /// **'Réseau Kad DHT'**
  String get searchTargetKad;

  /// No description provided for @searchTargetLabelKad.
  ///
  /// In fr, this message translates to:
  /// **'le réseau Kad DHT'**
  String get searchTargetLabelKad;

  /// No description provided for @searchTargetLabelGlobal.
  ///
  /// In fr, this message translates to:
  /// **'les serveurs eD2K (UDP)'**
  String get searchTargetLabelGlobal;

  /// No description provided for @searchTargetLabelServer.
  ///
  /// In fr, this message translates to:
  /// **'le serveur eD2K'**
  String get searchTargetLabelServer;

  /// No description provided for @searchTargetEnableGlobal.
  ///
  /// In fr, this message translates to:
  /// **'la liste de serveurs eD2K'**
  String get searchTargetEnableGlobal;

  /// No description provided for @searchTargetEnableServer.
  ///
  /// In fr, this message translates to:
  /// **'un serveur eD2K'**
  String get searchTargetEnableServer;

  /// No description provided for @kadActiveBanner.
  ///
  /// In fr, this message translates to:
  /// **'Kad DHT actif • Contacts : {contacts} ({buckets} k-buckets) • Bind : {addr} • Profil : {profile}'**
  String kadActiveBanner(
    Object addr,
    Object buckets,
    Object contacts,
    Object profile,
  );

  /// No description provided for @unknown.
  ///
  /// In fr, this message translates to:
  /// **'Inconnu'**
  String get unknown;

  /// No description provided for @kadInactiveBanner.
  ///
  /// In fr, this message translates to:
  /// **'Le réseau Kad DHT n\'est pas encore actif. Démarrez le socket Kad ou effectuez un bootstrap.'**
  String get kadInactiveBanner;

  /// No description provided for @searchGlobalBanner.
  ///
  /// In fr, this message translates to:
  /// **'Recherche globale UDP — interroge tous les serveurs connus (REQ3 si le serveur supporte EXT_GETFILES + LARGEFILES).'**
  String get searchGlobalBanner;

  /// No description provided for @searchConnectedBanner.
  ///
  /// In fr, this message translates to:
  /// **'Connecté à {name} ({addr}) • Client ID : {id} ({idType})'**
  String searchConnectedBanner(
    Object addr,
    Object id,
    Object idType,
    Object name,
  );

  /// No description provided for @serverUnnamed.
  ///
  /// In fr, this message translates to:
  /// **'Sans nom'**
  String get serverUnnamed;

  /// No description provided for @serverDisconnectBtn.
  ///
  /// In fr, this message translates to:
  /// **'Déconnecter'**
  String get serverDisconnectBtn;

  /// No description provided for @serverBannerConnected.
  ///
  /// In fr, this message translates to:
  /// **'Connexions eD2K disponibles. Vérifiez votre profil réseau (LocalLab vs PublicEd2kPreview) avant de vous connecter. Les adresses dans votre filtre IP sont automatiquement signalées.'**
  String get serverBannerConnected;

  /// No description provided for @serverBannerDisconnected.
  ///
  /// In fr, this message translates to:
  /// **'Avertissement réseau : importer ou gérer une liste de serveurs n\'établit aucune connexion réseau sortante. Les adresses figurant dans votre filtre IP actif sont automatiquement signalées.'**
  String get serverBannerDisconnected;

  /// No description provided for @serverBlockedByFilter.
  ///
  /// In fr, this message translates to:
  /// **'Bloqué par le filtre IP'**
  String get serverBlockedByFilter;

  /// No description provided for @serverDisabledBadge.
  ///
  /// In fr, this message translates to:
  /// **'Désactivé'**
  String get serverDisabledBadge;

  /// No description provided for @historyIntegrityErrors.
  ///
  /// In fr, this message translates to:
  /// **'Erreurs d\'intégrité détectées'**
  String get historyIntegrityErrors;

  /// No description provided for @labStep2Desc.
  ///
  /// In fr, this message translates to:
  /// **'Utilise le hash pour créer une tâche de téléchargement suivie dans le client.'**
  String get labStep2Desc;

  /// No description provided for @labAddedToDownloads.
  ///
  /// In fr, this message translates to:
  /// **'Ajouté aux téléchargements !'**
  String get labAddedToDownloads;

  /// No description provided for @labAddToDownloads.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter aux téléchargements'**
  String get labAddToDownloads;

  /// No description provided for @labStep4Desc.
  ///
  /// In fr, this message translates to:
  /// **'Ordonne au client de se connecter à la source 127.0.0.1 pour transférer les blocs.'**
  String get labStep4Desc;

  /// No description provided for @labDlDone.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargement terminé et vérifié ({size})'**
  String labDlDone(Object size);

  /// No description provided for @labDlTransferring.
  ///
  /// In fr, this message translates to:
  /// **'Transfert : {received} / {total} ({speed})'**
  String labDlTransferring(Object received, Object speed, Object total);

  /// No description provided for @labDlReady.
  ///
  /// In fr, this message translates to:
  /// **'État : {state} — prêt à se connecter au pair loopback.'**
  String labDlReady(Object state);

  /// No description provided for @labAlreadyCompleted.
  ///
  /// In fr, this message translates to:
  /// **'Déjà terminé'**
  String get labAlreadyCompleted;

  /// No description provided for @labConnectToSource.
  ///
  /// In fr, this message translates to:
  /// **'Connecter à la source'**
  String get labConnectToSource;

  /// No description provided for @labStep1Desc.
  ///
  /// In fr, this message translates to:
  /// **'Crée un fichier déterministe dans <workdir>/fixtures/ et calcule son hash eD2K.'**
  String get labStep1Desc;

  /// No description provided for @labGenerating.
  ///
  /// In fr, this message translates to:
  /// **'Génération...'**
  String get labGenerating;

  /// No description provided for @labGenerateFixture.
  ///
  /// In fr, this message translates to:
  /// **'Générer la fixture'**
  String get labGenerateFixture;

  /// No description provided for @labStep3Desc.
  ///
  /// In fr, this message translates to:
  /// **'Lance un pair eD2K local hébergeant les fixtures de test.'**
  String get labStep3Desc;

  /// No description provided for @labStopPeer.
  ///
  /// In fr, this message translates to:
  /// **'Arrêter le pair loopback'**
  String get labStopPeer;

  /// No description provided for @labStartPeer.
  ///
  /// In fr, this message translates to:
  /// **'Démarrer le pair loopback'**
  String get labStartPeer;

  /// No description provided for @labPeerActive.
  ///
  /// In fr, this message translates to:
  /// **'Actif sur {addr} (sert {count} fichier(s))'**
  String labPeerActive(Object addr, Object count);

  /// No description provided for @labPeerOffline.
  ///
  /// In fr, this message translates to:
  /// **'Pair hors ligne'**
  String get labPeerOffline;

  /// No description provided for @labStep5Desc.
  ///
  /// In fr, this message translates to:
  /// **'Purge les fichiers générés dans <workdir>/fixtures et les partiels dans <workdir>/temp. Les téléchargements terminés dans downloads/ sont préservés.'**
  String get labStep5Desc;

  /// No description provided for @labCleanupSafe.
  ///
  /// In fr, this message translates to:
  /// **'Sans risque en mode portable.'**
  String get labCleanupSafe;

  /// No description provided for @labCleanupRestricted.
  ///
  /// In fr, this message translates to:
  /// **'Le nettoyage est restreint en mode installé pour protéger les fichiers utilisateur.'**
  String get labCleanupRestricted;

  /// No description provided for @labErrCreateFixture.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la création de la fixture : {error}'**
  String labErrCreateFixture(Object error);

  /// No description provided for @labErrAddDownload.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'ajout du téléchargement : {error}'**
  String labErrAddDownload(Object error);

  /// No description provided for @labErrPeerOp.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'opération sur le pair : {error}'**
  String labErrPeerOp(Object error);

  /// No description provided for @labErrConnectDownload.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la connexion du téléchargement : {error}'**
  String labErrConnectDownload(Object error);

  /// No description provided for @labErrCleanup.
  ///
  /// In fr, this message translates to:
  /// **'Échec du nettoyage : {error}'**
  String labErrCleanup(Object error);

  /// No description provided for @notAvailable.
  ///
  /// In fr, this message translates to:
  /// **'Non disponible'**
  String get notAvailable;

  /// No description provided for @copyFullPath.
  ///
  /// In fr, this message translates to:
  /// **'Copier le chemin complet'**
  String get copyFullPath;

  /// No description provided for @copiedPathInternal.
  ///
  /// In fr, this message translates to:
  /// **'Chemin des données internes copié'**
  String get copiedPathInternal;

  /// No description provided for @copyDownloadsPath.
  ///
  /// In fr, this message translates to:
  /// **'Copier le chemin du dossier de téléchargements'**
  String get copyDownloadsPath;

  /// No description provided for @copiedDownloadsPath.
  ///
  /// In fr, this message translates to:
  /// **'Chemin des téléchargements copié'**
  String get copiedDownloadsPath;

  /// No description provided for @copyDaemonLogPath.
  ///
  /// In fr, this message translates to:
  /// **'Copier le chemin du journal du daemon'**
  String get copyDaemonLogPath;

  /// No description provided for @copiedDaemonLogPath.
  ///
  /// In fr, this message translates to:
  /// **'Chemin du journal du daemon copié'**
  String get copiedDaemonLogPath;

  /// No description provided for @diagNetworkProfile.
  ///
  /// In fr, this message translates to:
  /// **'Profil réseau : {profile}'**
  String diagNetworkProfile(Object profile);

  /// No description provided for @msgDownloadAdded.
  ///
  /// In fr, this message translates to:
  /// **'Fichier \"{name}\" ajouté aux téléchargements'**
  String msgDownloadAdded(Object name);

  /// No description provided for @msgDownloadDuplicate.
  ///
  /// In fr, this message translates to:
  /// **'Ce fichier est déjà dans la liste des téléchargements (\"{name}\", statut : {status})'**
  String msgDownloadDuplicate(Object name, Object status);

  /// No description provided for @msgImportedFile.
  ///
  /// In fr, this message translates to:
  /// **'\"{name}\" importé ({verified}/{parts} parts vérifiées)'**
  String msgImportedFile(Object name, Object parts, Object verified);

  /// No description provided for @msgErrImportPartMet.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'import .part.met : {error}'**
  String msgErrImportPartMet(Object error);

  /// No description provided for @msgErrPause.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la pause : {error}'**
  String msgErrPause(Object error);

  /// No description provided for @msgErrResume.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la reprise : {error}'**
  String msgErrResume(Object error);

  /// No description provided for @msgErrRename.
  ///
  /// In fr, this message translates to:
  /// **'Échec du renommage : {error}'**
  String msgErrRename(Object error);

  /// No description provided for @msgErrRemove.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la suppression : {error}'**
  String msgErrRemove(Object error);

  /// No description provided for @msgErrFetchDetails.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la récupération des détails : {error}'**
  String msgErrFetchDetails(Object error);

  /// No description provided for @msgDestinationChanged.
  ///
  /// In fr, this message translates to:
  /// **'Destination changée : {dir}'**
  String msgDestinationChanged(Object dir);

  /// No description provided for @msgErrChangeDestination.
  ///
  /// In fr, this message translates to:
  /// **'Échec du changement de destination : {error}'**
  String msgErrChangeDestination(Object error);

  /// No description provided for @msgRetryLoopback.
  ///
  /// In fr, this message translates to:
  /// **'Nouvelle tentative loopback pour \"{name}\"'**
  String msgRetryLoopback(Object name);

  /// No description provided for @msgErrRetryConn.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la nouvelle tentative : {error}'**
  String msgErrRetryConn(Object error);

  /// No description provided for @msgErrOpenFile.
  ///
  /// In fr, this message translates to:
  /// **'Impossible d\'ouvrir le fichier terminé : {error}'**
  String msgErrOpenFile(Object error);

  /// No description provided for @msgDownloadCompleted.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargement terminé : {name}'**
  String msgDownloadCompleted(Object name);

  /// No description provided for @msgDownloadFailed.
  ///
  /// In fr, this message translates to:
  /// **'Échec du téléchargement : {name}'**
  String msgDownloadFailed(Object name);

  /// No description provided for @msgCategoryCreated.
  ///
  /// In fr, this message translates to:
  /// **'Catégorie \"{name}\" créée'**
  String msgCategoryCreated(Object name);

  /// No description provided for @msgErrCategoryCreate.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la création de la catégorie : {error}'**
  String msgErrCategoryCreate(Object error);

  /// No description provided for @msgCategoryUpdated.
  ///
  /// In fr, this message translates to:
  /// **'Catégorie \"{name}\" mise à jour'**
  String msgCategoryUpdated(Object name);

  /// No description provided for @msgErrCategoryUpdate.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la mise à jour de la catégorie : {error}'**
  String msgErrCategoryUpdate(Object error);

  /// No description provided for @msgCategoryDeleted.
  ///
  /// In fr, this message translates to:
  /// **'Catégorie \"{name}\" supprimée'**
  String msgCategoryDeleted(Object name);

  /// No description provided for @msgErrCategoryDelete.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la suppression de la catégorie : {error}'**
  String msgErrCategoryDelete(Object error);

  /// No description provided for @msgErrCategoryReorder.
  ///
  /// In fr, this message translates to:
  /// **'Échec du réordonnancement des catégories : {error}'**
  String msgErrCategoryReorder(Object error);

  /// No description provided for @msgAssignedToCategory.
  ///
  /// In fr, this message translates to:
  /// **'Assigné à la catégorie \"{name}\"'**
  String msgAssignedToCategory(Object name);

  /// No description provided for @categoryFallback.
  ///
  /// In fr, this message translates to:
  /// **'Catégorie'**
  String get categoryFallback;

  /// No description provided for @msgProfileSwitched.
  ///
  /// In fr, this message translates to:
  /// **'Profil réseau basculé : {profile}'**
  String msgProfileSwitched(Object profile);

  /// No description provided for @msgErrProfileSwitch.
  ///
  /// In fr, this message translates to:
  /// **'Échec du changement de profil réseau : {error}'**
  String msgErrProfileSwitch(Object error);

  /// No description provided for @msgKillSwitchOn.
  ///
  /// In fr, this message translates to:
  /// **'Kill switch activé : toutes les connexions réseau ont été coupées'**
  String get msgKillSwitchOn;

  /// No description provided for @msgErrKillSwitch.
  ///
  /// In fr, this message translates to:
  /// **'Échec du kill switch : {error}'**
  String msgErrKillSwitch(Object error);

  /// No description provided for @msgNetworkReactivated.
  ///
  /// In fr, this message translates to:
  /// **'Réseau réactivé'**
  String get msgNetworkReactivated;

  /// No description provided for @msgErrNetworkReactivate.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la réactivation du réseau : {error}'**
  String msgErrNetworkReactivate(Object error);

  /// No description provided for @msgBudgetsUpdated.
  ///
  /// In fr, this message translates to:
  /// **'Budgets de ressources mis à jour'**
  String get msgBudgetsUpdated;

  /// No description provided for @msgErrBudgets.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la mise à jour des budgets de ressources : {error}'**
  String msgErrBudgets(Object error);

  /// No description provided for @msgIpFilterImported.
  ///
  /// In fr, this message translates to:
  /// **'Filtre IP importé : {count} plages'**
  String msgIpFilterImported(Object count);

  /// No description provided for @msgErrIpFilterImport.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'import du filtre IP : {error}'**
  String msgErrIpFilterImport(Object error);

  /// No description provided for @msgIpFilterEnabled.
  ///
  /// In fr, this message translates to:
  /// **'Filtre IP activé'**
  String get msgIpFilterEnabled;

  /// No description provided for @msgIpFilterDisabled.
  ///
  /// In fr, this message translates to:
  /// **'Filtre IP désactivé'**
  String get msgIpFilterDisabled;

  /// No description provided for @msgErrIpFilterToggle.
  ///
  /// In fr, this message translates to:
  /// **'Échec du basculement du filtre IP : {error}'**
  String msgErrIpFilterToggle(Object error);

  /// No description provided for @msgIpFilterCleared.
  ///
  /// In fr, this message translates to:
  /// **'Filtre IP vidé'**
  String get msgIpFilterCleared;

  /// No description provided for @msgErrIpFilterClear.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la purge du filtre IP : {error}'**
  String msgErrIpFilterClear(Object error);

  /// No description provided for @msgRateLimitsUpdated.
  ///
  /// In fr, this message translates to:
  /// **'Limites de débit mises à jour'**
  String get msgRateLimitsUpdated;

  /// No description provided for @msgErrRateLimits.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la mise à jour des limites de débit : {error}'**
  String msgErrRateLimits(Object error);

  /// No description provided for @msgErrLoadSources.
  ///
  /// In fr, this message translates to:
  /// **'Échec du chargement des sources : {error}'**
  String msgErrLoadSources(Object error);

  /// No description provided for @msgSourceAdded.
  ///
  /// In fr, this message translates to:
  /// **'Source {address} ajoutée'**
  String msgSourceAdded(Object address);

  /// No description provided for @msgErrAddSource.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'ajout de la source : {error}'**
  String msgErrAddSource(Object error);

  /// No description provided for @msgConnInitiated.
  ///
  /// In fr, this message translates to:
  /// **'Connexion à {address} initialisée'**
  String msgConnInitiated(Object address);

  /// No description provided for @msgErrConnectSource.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la connexion à la source : {error}'**
  String msgErrConnectSource(Object error);

  /// No description provided for @msgPeerDisconnected.
  ///
  /// In fr, this message translates to:
  /// **'Peer {address} déconnecté'**
  String msgPeerDisconnected(Object address);

  /// No description provided for @msgSessionsDisconnected.
  ///
  /// In fr, this message translates to:
  /// **'Session(s) peer déconnectée(s)'**
  String get msgSessionsDisconnected;

  /// No description provided for @msgErrDisconnectPeer.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la déconnexion peer : {error}'**
  String msgErrDisconnectPeer(Object error);

  /// No description provided for @msgPriorityChanged.
  ///
  /// In fr, this message translates to:
  /// **'Priorité changée : {priority}'**
  String msgPriorityChanged(Object priority);

  /// No description provided for @msgErrPriority.
  ///
  /// In fr, this message translates to:
  /// **'Échec du changement de priorité : {error}'**
  String msgErrPriority(Object error);

  /// No description provided for @msgSrcLimitReset.
  ///
  /// In fr, this message translates to:
  /// **'Limite de sources réinitialisée'**
  String get msgSrcLimitReset;

  /// No description provided for @msgSrcLimitSet.
  ///
  /// In fr, this message translates to:
  /// **'Limite de sources : {max}'**
  String msgSrcLimitSet(Object max);

  /// No description provided for @msgErrSrcLimit.
  ///
  /// In fr, this message translates to:
  /// **'Échec du changement de limite : {error}'**
  String msgErrSrcLimit(Object error);

  /// No description provided for @msgSourcesCleared.
  ///
  /// In fr, this message translates to:
  /// **'Sources effacées'**
  String get msgSourcesCleared;

  /// No description provided for @msgErrClearSources.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'effacement des sources : {error}'**
  String msgErrClearSources(Object error);

  /// No description provided for @msgErrKadContacts.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la récupération des contacts Kad : {error}'**
  String msgErrKadContacts(Object error);

  /// No description provided for @msgErrNodesDatInspect.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'inspection de nodes.dat : {error}'**
  String msgErrNodesDatInspect(Object error);

  /// No description provided for @msgKadContactsImported.
  ///
  /// In fr, this message translates to:
  /// **'{count} contacts Kad importés dans la table K-buckets'**
  String msgKadContactsImported(Object count);

  /// No description provided for @msgErrNodesDatImport.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'importation de nodes.dat'**
  String get msgErrNodesDatImport;

  /// No description provided for @msgKadContactsCleared.
  ///
  /// In fr, this message translates to:
  /// **'{count} contacts Kad effacés'**
  String msgKadContactsCleared(Object count);

  /// No description provided for @msgErrKadClear.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'effacement des contacts Kad : {error}'**
  String msgErrKadClear(Object error);

  /// No description provided for @msgKadUdpStarted.
  ///
  /// In fr, this message translates to:
  /// **'Socket UDP Kad démarré sur {addr}'**
  String msgKadUdpStarted(Object addr);

  /// No description provided for @msgErrKadUdpStart.
  ///
  /// In fr, this message translates to:
  /// **'Échec du démarrage UDP Kad'**
  String get msgErrKadUdpStart;

  /// No description provided for @msgKadUdpStopped.
  ///
  /// In fr, this message translates to:
  /// **'Socket UDP Kad arrêté'**
  String get msgKadUdpStopped;

  /// No description provided for @msgErrKadUdpStop.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'arrêt UDP Kad'**
  String get msgErrKadUdpStop;

  /// No description provided for @msgKadPong.
  ///
  /// In fr, this message translates to:
  /// **'Pong reçu de {addr} en {latency} ms'**
  String msgKadPong(Object addr, Object latency);

  /// No description provided for @msgErrKadPing.
  ///
  /// In fr, this message translates to:
  /// **'Échec du ping Kad ({addr}) : {error}'**
  String msgErrKadPing(Object addr, Object error);

  /// No description provided for @msgKadBootstrapOk.
  ///
  /// In fr, this message translates to:
  /// **'Amorçage Kad réussi ({count} contacts reçus)'**
  String msgKadBootstrapOk(Object count);

  /// No description provided for @msgErrKadBootstrap.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'amorçage Kad'**
  String get msgErrKadBootstrap;

  /// No description provided for @msgKadBootstrapUrlOk.
  ///
  /// In fr, this message translates to:
  /// **'Amorçage Kad depuis URL réussi ({count} nœuds importés)'**
  String msgKadBootstrapUrlOk(Object count);

  /// No description provided for @msgErrKadBootstrapUrl.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'amorçage Kad depuis URL'**
  String get msgErrKadBootstrapUrl;

  /// No description provided for @msgKadSearchFound.
  ///
  /// In fr, this message translates to:
  /// **'Recherche Kad : {count} nœuds les plus proches trouvés'**
  String msgKadSearchFound(Object count);

  /// No description provided for @msgErrKadSearch.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la recherche Kad'**
  String get msgErrKadSearch;

  /// No description provided for @msgKadSourcesFound.
  ///
  /// In fr, this message translates to:
  /// **'Kad : {count} sources trouvées pour {hash}…'**
  String msgKadSourcesFound(Object count, Object hash);

  /// No description provided for @msgErrKadSources.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la recherche de sources Kad'**
  String get msgErrKadSources;

  /// No description provided for @msgKadPublished.
  ///
  /// In fr, this message translates to:
  /// **'Kad : publié sur {count} nœuds pour {hash}…'**
  String msgKadPublished(Object count, Object hash);

  /// No description provided for @msgErrKadPublish.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la publication Kad'**
  String get msgErrKadPublish;

  /// No description provided for @msgSearchGlobalSent.
  ///
  /// In fr, this message translates to:
  /// **'Recherche globale \"{term}\" envoyée{udp}'**
  String msgSearchGlobalSent(Object term, Object udp);

  /// No description provided for @msgSearchSent.
  ///
  /// In fr, this message translates to:
  /// **'Recherche \"{term}\" envoyée au serveur{udp}'**
  String msgSearchSent(Object term, Object udp);

  /// No description provided for @msgSearchNoResponse.
  ///
  /// In fr, this message translates to:
  /// **'Recherche \"{term}\" terminée sans réponse du serveur'**
  String msgSearchNoResponse(Object term);

  /// No description provided for @msgErrSearch.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la recherche : {error}'**
  String msgErrSearch(Object error);

  /// No description provided for @msgKadSearchDone.
  ///
  /// In fr, this message translates to:
  /// **'Recherche Kad \"{term}\" terminée ({count} résultat(s))'**
  String msgKadSearchDone(Object count, Object term);

  /// No description provided for @msgErrKadSearchRun.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la recherche Kad : {error}'**
  String msgErrKadSearchRun(Object error);

  /// No description provided for @msgSourceReqSent.
  ///
  /// In fr, this message translates to:
  /// **'Demande de sources envoyée pour {hash}'**
  String msgSourceReqSent(Object hash);

  /// No description provided for @msgErrSourceReq.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la demande de sources : {error}'**
  String msgErrSourceReq(Object error);

  /// No description provided for @msgErrListServers.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la liste des serveurs : {error}'**
  String msgErrListServers(Object error);

  /// No description provided for @msgErrInspectServerMet.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'inspection du server.met : {error}'**
  String msgErrInspectServerMet(Object error);

  /// No description provided for @msgServersImported.
  ///
  /// In fr, this message translates to:
  /// **'{imported} serveurs importés ({allowed} autorisés, {blocked} bloqués par le filtre IP)'**
  String msgServersImported(Object allowed, Object blocked, Object imported);

  /// No description provided for @msgErrImportServerMet.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'import du server.met : {error}'**
  String msgErrImportServerMet(Object error);

  /// No description provided for @msgServersUpdated.
  ///
  /// In fr, this message translates to:
  /// **'Mise à jour réussie : {count} serveurs traités depuis l\'URL'**
  String msgServersUpdated(Object count);

  /// No description provided for @msgErrServersUpdate.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la mise à jour des serveurs depuis l\'URL : {error}'**
  String msgErrServersUpdate(Object error);

  /// No description provided for @msgErrToggleServer.
  ///
  /// In fr, this message translates to:
  /// **'Échec du basculement du serveur : {error}'**
  String msgErrToggleServer(Object error);

  /// No description provided for @msgErrDeleteServer.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la suppression du serveur : {error}'**
  String msgErrDeleteServer(Object error);

  /// No description provided for @msgServersCleared.
  ///
  /// In fr, this message translates to:
  /// **'{count} serveurs effacés'**
  String msgServersCleared(Object count);

  /// No description provided for @msgErrClearServers.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'effacement des serveurs : {error}'**
  String msgErrClearServers(Object error);

  /// No description provided for @msgConnectingToServer.
  ///
  /// In fr, this message translates to:
  /// **'Connexion au serveur {name}...'**
  String msgConnectingToServer(Object name);

  /// No description provided for @msgErrConnectServer.
  ///
  /// In fr, this message translates to:
  /// **'Connexion impossible : {error}'**
  String msgErrConnectServer(Object error);

  /// No description provided for @msgDisconnectedFromServer.
  ///
  /// In fr, this message translates to:
  /// **'Déconnecté du serveur eD2K'**
  String get msgDisconnectedFromServer;

  /// No description provided for @msgErrDisconnectServer.
  ///
  /// In fr, this message translates to:
  /// **'Erreur de déconnexion : {error}'**
  String msgErrDisconnectServer(Object error);

  /// No description provided for @msgSettingsUpdated.
  ///
  /// In fr, this message translates to:
  /// **'Paramètres mis à jour'**
  String get msgSettingsUpdated;

  /// No description provided for @msgErrSettingsUpdate.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la mise à jour des paramètres : {error}'**
  String msgErrSettingsUpdate(Object error);

  /// No description provided for @msgDownloadDirUpdated.
  ///
  /// In fr, this message translates to:
  /// **'Dossier de téléchargement par défaut mis à jour'**
  String get msgDownloadDirUpdated;

  /// No description provided for @msgErrDownloadDir.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la définition du dossier de téléchargement : {error}'**
  String msgErrDownloadDir(Object error);

  /// No description provided for @msgDownloadDirReset.
  ///
  /// In fr, this message translates to:
  /// **'Dossier de téléchargement réinitialisé (défaut portable)'**
  String get msgDownloadDirReset;

  /// No description provided for @msgErrDownloadDirReset.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la réinitialisation du dossier de téléchargement : {error}'**
  String msgErrDownloadDirReset(Object error);

  /// No description provided for @msgErrFetchHistory.
  ///
  /// In fr, this message translates to:
  /// **'Échec du chargement de l\'historique : {error}'**
  String msgErrFetchHistory(Object error);

  /// No description provided for @msgHistoryCleared.
  ///
  /// In fr, this message translates to:
  /// **'{count} enregistrement(s) d\'historique effacé(s)'**
  String msgHistoryCleared(Object count);

  /// No description provided for @msgErrClearHistory.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'effacement de l\'historique : {error}'**
  String msgErrClearHistory(Object error);

  /// No description provided for @msgErrExportHistory.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'export de l\'historique : {error}'**
  String msgErrExportHistory(Object error);

  /// No description provided for @msgRetentionUnlimited.
  ///
  /// In fr, this message translates to:
  /// **'Rétention de l\'historique illimitée'**
  String get msgRetentionUnlimited;

  /// No description provided for @msgRetentionDays.
  ///
  /// In fr, this message translates to:
  /// **'Rétention de l\'historique : {days} jours'**
  String msgRetentionDays(Object days);

  /// No description provided for @msgErrRetention.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la mise à jour de la rétention : {error}'**
  String msgErrRetention(Object error);

  /// No description provided for @msgBytesInjected.
  ///
  /// In fr, this message translates to:
  /// **'{count} octets injectés à l\'offset {offset}'**
  String msgBytesInjected(Object count, Object offset);

  /// No description provided for @msgErrInject.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'injection : {error}'**
  String msgErrInject(Object error);

  /// No description provided for @msgFixtureCreated.
  ///
  /// In fr, this message translates to:
  /// **'Fixture {name} créée'**
  String msgFixtureCreated(Object name);

  /// No description provided for @msgPeerStarted.
  ///
  /// In fr, this message translates to:
  /// **'Pair loopback démarré sur {addr}'**
  String msgPeerStarted(Object addr);

  /// No description provided for @msgPeerStopped.
  ///
  /// In fr, this message translates to:
  /// **'Pair de test loopback arrêté'**
  String get msgPeerStopped;

  /// No description provided for @msgConnectedToPeer.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargement connecté à {addr}'**
  String msgConnectedToPeer(Object addr);

  /// No description provided for @msgCatSetError.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'assignation de la catégorie : {e}'**
  String msgCatSetError(Object e);

  /// No description provided for @msgSourcesFound.
  ///
  /// In fr, this message translates to:
  /// **'{n} source(s) trouvée(s) pour {hash}'**
  String msgSourcesFound(Object hash, Object n);

  /// No description provided for @kadTimeJustNow.
  ///
  /// In fr, this message translates to:
  /// **'À l\'instant'**
  String get kadTimeJustNow;

  /// No description provided for @labCreateFixtureError.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la création de la fixture : {e}'**
  String labCreateFixtureError(Object e);

  /// No description provided for @labAddDownloadError.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'ajout du téléchargement : {e}'**
  String labAddDownloadError(Object e);

  /// No description provided for @labPeerError.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'opération sur le pair : {e}'**
  String labPeerError(Object e);

  /// No description provided for @labConnectError.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la connexion du téléchargement : {e}'**
  String labConnectError(Object e);

  /// No description provided for @labCleanupError.
  ///
  /// In fr, this message translates to:
  /// **'Échec du nettoyage : {e}'**
  String labCleanupError(Object e);

  /// No description provided for @labDownloadComplete.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargement terminé et vérifié ({size})'**
  String labDownloadComplete(Object size);

  /// No description provided for @labTransferring.
  ///
  /// In fr, this message translates to:
  /// **'Transfert : {recv} / {total} ({rate})'**
  String labTransferring(Object rate, Object recv, Object total);

  /// No description provided for @labReadyToConnect.
  ///
  /// In fr, this message translates to:
  /// **'État : {state} — prêt à se connecter au pair loopback.'**
  String labReadyToConnect(Object state);

  /// No description provided for @labCleanupPortableOk.
  ///
  /// In fr, this message translates to:
  /// **'Sûr à exécuter en mode portable.'**
  String get labCleanupPortableOk;

  /// No description provided for @labCleanupPortableNo.
  ///
  /// In fr, this message translates to:
  /// **'Le nettoyage est restreint en mode installé pour protéger les fichiers utilisateur.'**
  String get labCleanupPortableNo;

  /// No description provided for @histFilesCount.
  ///
  /// In fr, this message translates to:
  /// **'{n} fichiers'**
  String histFilesCount(Object n);

  /// No description provided for @histSessionCount.
  ///
  /// In fr, this message translates to:
  /// **'{n} cette session ({size})'**
  String histSessionCount(Object n, Object size);

  /// No description provided for @histResumesCount.
  ///
  /// In fr, this message translates to:
  /// **'{n} reprises'**
  String histResumesCount(Object n);

  /// No description provided for @histHashMismatchCount.
  ///
  /// In fr, this message translates to:
  /// **'{n} erreur(s) de hash'**
  String histHashMismatchCount(Object n);

  /// No description provided for @histHashVerified.
  ///
  /// In fr, this message translates to:
  /// **'100 % hash vérifié'**
  String get histHashVerified;

  /// No description provided for @histIntegrityErrors.
  ///
  /// In fr, this message translates to:
  /// **'Erreurs d\'intégrité détectées'**
  String get histIntegrityErrors;

  /// No description provided for @peerReceivedBytes.
  ///
  /// In fr, this message translates to:
  /// **'Reçus : {bytes}'**
  String peerReceivedBytes(Object bytes);

  /// No description provided for @searchUdpFailed.
  ///
  /// In fr, this message translates to:
  /// **', {n} échec(s)'**
  String searchUdpFailed(Object n);

  /// No description provided for @kadIdGenerated.
  ///
  /// In fr, this message translates to:
  /// **'(Généré à l\'initialisation)'**
  String get kadIdGenerated;

  /// No description provided for @ed2kErrPrefix.
  ///
  /// In fr, this message translates to:
  /// **'Le lien doit commencer par \"ed2k://|file|\"'**
  String get ed2kErrPrefix;

  /// No description provided for @ed2kErrIncomplete.
  ///
  /// In fr, this message translates to:
  /// **'Format de lien eD2K incomplet'**
  String get ed2kErrIncomplete;

  /// No description provided for @ed2kErrInvalid.
  ///
  /// In fr, this message translates to:
  /// **'Nom de fichier, taille ou hash hexadécimal de 32 caractères invalide'**
  String get ed2kErrInvalid;

  /// No description provided for @partsCount.
  ///
  /// In fr, this message translates to:
  /// **'{n} partie(s)'**
  String partsCount(Object n);

  /// No description provided for @sourcesLoopback.
  ///
  /// In fr, this message translates to:
  /// **'{n} loopback'**
  String sourcesLoopback(Object n);

  /// No description provided for @diagLoopbackTitle.
  ///
  /// In fr, this message translates to:
  /// **'Profil réseau : LocalLab (loopback)'**
  String get diagLoopbackTitle;

  /// No description provided for @diagLoopbackSubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Trafic confiné à 127.0.0.1 — aucune donnée ne quitte la machine.'**
  String get diagLoopbackSubtitle;

  /// No description provided for @diagPublicSubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Connexions WAN autorisées — protection anti-SSRF active.'**
  String get diagPublicSubtitle;

  /// No description provided for @navAssistant.
  ///
  /// In fr, this message translates to:
  /// **'Assistant IA'**
  String get navAssistant;

  /// No description provided for @assistantTitle.
  ///
  /// In fr, this message translates to:
  /// **'Assistant local'**
  String get assistantTitle;

  /// No description provided for @assistantEmpty.
  ///
  /// In fr, this message translates to:
  /// **'Posez une question — le modèle local peut consulter et piloter le client via des outils validés.'**
  String get assistantEmpty;

  /// No description provided for @assistantInputHint.
  ///
  /// In fr, this message translates to:
  /// **'Message pour l\'assistant…'**
  String get assistantInputHint;

  /// No description provided for @assistantSend.
  ///
  /// In fr, this message translates to:
  /// **'Envoyer'**
  String get assistantSend;

  /// No description provided for @assistantCancel.
  ///
  /// In fr, this message translates to:
  /// **'Annuler le tour'**
  String get assistantCancel;

  /// No description provided for @assistantConfirmTitle.
  ///
  /// In fr, this message translates to:
  /// **'L\'assistant demande confirmation'**
  String get assistantConfirmTitle;

  /// No description provided for @assistantConfirmAllow.
  ///
  /// In fr, this message translates to:
  /// **'Autoriser'**
  String get assistantConfirmAllow;

  /// No description provided for @assistantConfirmDeny.
  ///
  /// In fr, this message translates to:
  /// **'Refuser'**
  String get assistantConfirmDeny;

  /// No description provided for @assistantToolCall.
  ///
  /// In fr, this message translates to:
  /// **'Appel d\'outil'**
  String get assistantToolCall;

  /// No description provided for @assistantToolResult.
  ///
  /// In fr, this message translates to:
  /// **'Résultat d\'outil'**
  String get assistantToolResult;

  /// No description provided for @assistantStatusConnected.
  ///
  /// In fr, this message translates to:
  /// **'Connecté · {tools} outils'**
  String assistantStatusConnected(Object tools);

  /// No description provided for @assistantStatusConnecting.
  ///
  /// In fr, this message translates to:
  /// **'Connexion…'**
  String get assistantStatusConnecting;

  /// No description provided for @assistantStatusDisconnected.
  ///
  /// In fr, this message translates to:
  /// **'Déconnecté'**
  String get assistantStatusDisconnected;

  /// No description provided for @assistantStatusError.
  ///
  /// In fr, this message translates to:
  /// **'Erreur de connexion'**
  String get assistantStatusError;

  /// No description provided for @assistantUnavailableTitle.
  ///
  /// In fr, this message translates to:
  /// **'Assistant indisponible'**
  String get assistantUnavailableTitle;

  /// No description provided for @assistantUnavailableHint.
  ///
  /// In fr, this message translates to:
  /// **'Lancez « mule-assistant serve » à côté du daemon pour activer le chat local (Ollama requis).'**
  String get assistantUnavailableHint;

  /// No description provided for @assistantTokenUnavailable.
  ///
  /// In fr, this message translates to:
  /// **'Jeton de session introuvable — l\'assistant nécessite un daemon local supervisé.'**
  String get assistantTokenUnavailable;

  /// No description provided for @assistantConnectError.
  ///
  /// In fr, this message translates to:
  /// **'Connexion à l\'assistant impossible : {error}'**
  String assistantConnectError(Object error);

  /// No description provided for @assistantRetry.
  ///
  /// In fr, this message translates to:
  /// **'Réessayer'**
  String get assistantRetry;

  /// No description provided for @kadNotesTitle.
  ///
  /// In fr, this message translates to:
  /// **'Commentaires Kad'**
  String get kadNotesTitle;

  /// No description provided for @kadNotesTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Commentaires Kad (notes et avis)'**
  String get kadNotesTooltip;

  /// No description provided for @kadNotesLoading.
  ///
  /// In fr, this message translates to:
  /// **'Recherche des commentaires sur Kad…'**
  String get kadNotesLoading;

  /// No description provided for @kadNotesEmpty.
  ///
  /// In fr, this message translates to:
  /// **'Aucun commentaire trouvé sur Kad pour ce fichier.'**
  String get kadNotesEmpty;

  /// No description provided for @kadNotesLoadError.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la recherche de commentaires : {error}'**
  String kadNotesLoadError(Object error);

  /// No description provided for @kadNotesPublisher.
  ///
  /// In fr, this message translates to:
  /// **'Éditeur {id}…'**
  String kadNotesPublisher(Object id);

  /// No description provided for @kadNotesYourComment.
  ///
  /// In fr, this message translates to:
  /// **'Votre commentaire'**
  String get kadNotesYourComment;

  /// No description provided for @kadNotesCommentHint.
  ///
  /// In fr, this message translates to:
  /// **'Commentaire visible par les pairs Kad (512 octets max)…'**
  String get kadNotesCommentHint;

  /// No description provided for @kadNotesCommentRequired.
  ///
  /// In fr, this message translates to:
  /// **'Un commentaire est requis'**
  String get kadNotesCommentRequired;

  /// No description provided for @kadNotesRating.
  ///
  /// In fr, this message translates to:
  /// **'Note'**
  String get kadNotesRating;

  /// No description provided for @kadNotesNoRating.
  ///
  /// In fr, this message translates to:
  /// **'Sans note'**
  String get kadNotesNoRating;

  /// No description provided for @kadNotesPublish.
  ///
  /// In fr, this message translates to:
  /// **'Publier'**
  String get kadNotesPublish;

  /// No description provided for @kadNotesPublished.
  ///
  /// In fr, this message translates to:
  /// **'Commentaire publié sur {count} nœud(s)'**
  String kadNotesPublished(Object count);

  /// No description provided for @kadNotesPublishError.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la publication du commentaire : {error}'**
  String kadNotesPublishError(Object error);

  /// No description provided for @sharePublishKadTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Publier sur Kad (source + mots-clés)'**
  String get sharePublishKadTooltip;

  /// No description provided for @shareNotesTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Commentaires Kad'**
  String get shareNotesTooltip;

  /// No description provided for @msgKadFilePublished.
  ///
  /// In fr, this message translates to:
  /// **'Kad : fichier publié ({sources} source(s), {keywords} mot(s)-clé(s))'**
  String msgKadFilePublished(Object keywords, Object sources);

  /// No description provided for @msgErrKadPublishFile.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la publication Kad du fichier'**
  String get msgErrKadPublishFile;

  /// No description provided for @sharesAddTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter un partage manuel'**
  String get sharesAddTooltip;

  /// No description provided for @sharesAddTitle.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter un fichier partagé'**
  String get sharesAddTitle;

  /// No description provided for @sharesAddHashLabel.
  ///
  /// In fr, this message translates to:
  /// **'Hash eD2K (hexadécimal, 32 caractères)'**
  String get sharesAddHashLabel;

  /// No description provided for @sharesAddNameLabel.
  ///
  /// In fr, this message translates to:
  /// **'Nom du fichier'**
  String get sharesAddNameLabel;

  /// No description provided for @sharesAddSizeLabel.
  ///
  /// In fr, this message translates to:
  /// **'Taille (octets)'**
  String get sharesAddSizeLabel;

  /// No description provided for @sharesAddPathLabel.
  ///
  /// In fr, this message translates to:
  /// **'Chemin du fichier'**
  String get sharesAddPathLabel;

  /// No description provided for @sharesAddInvalid.
  ///
  /// In fr, this message translates to:
  /// **'Champs invalides : hash hex de 32 caractères, nom et taille > 0 requis'**
  String get sharesAddInvalid;

  /// No description provided for @sharesAddAction.
  ///
  /// In fr, this message translates to:
  /// **'Ajouter'**
  String get sharesAddAction;

  /// No description provided for @msgShareAdded.
  ///
  /// In fr, this message translates to:
  /// **'Fichier ajouté aux partages'**
  String get msgShareAdded;

  /// No description provided for @msgErrShareAdd.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'ajout du partage : {error}'**
  String msgErrShareAdd(Object error);

  /// No description provided for @listenerTitle.
  ///
  /// In fr, this message translates to:
  /// **'Écoute entrante eD2K'**
  String get listenerTitle;

  /// No description provided for @listenerSubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Socket TCP qui accepte les connexions pairs entrantes (uploads, échanges de sources).'**
  String get listenerSubtitle;

  /// No description provided for @listenerListening.
  ///
  /// In fr, this message translates to:
  /// **'En écoute sur {addr}'**
  String listenerListening(Object addr);

  /// No description provided for @listenerStopped.
  ///
  /// In fr, this message translates to:
  /// **'Arrêté'**
  String get listenerStopped;

  /// No description provided for @listenerActiveUploads.
  ///
  /// In fr, this message translates to:
  /// **'Uploads actifs'**
  String get listenerActiveUploads;

  /// No description provided for @listenerQueued.
  ///
  /// In fr, this message translates to:
  /// **'En file'**
  String get listenerQueued;

  /// No description provided for @listenerRate.
  ///
  /// In fr, this message translates to:
  /// **'Débit'**
  String get listenerRate;

  /// No description provided for @listenerStart.
  ///
  /// In fr, this message translates to:
  /// **'Démarrer'**
  String get listenerStart;

  /// No description provided for @listenerStop.
  ///
  /// In fr, this message translates to:
  /// **'Arrêter'**
  String get listenerStop;

  /// No description provided for @listenerError.
  ///
  /// In fr, this message translates to:
  /// **'Échec de l\'opération sur le listener : {error}'**
  String listenerError(Object error);

  /// No description provided for @identityTitle.
  ///
  /// In fr, this message translates to:
  /// **'Identité sécurisée (SUI)'**
  String get identityTitle;

  /// No description provided for @identitySubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Clé RSA signant nos échanges eD2K — les crédits pairs sont liés à cette identité.'**
  String get identitySubtitle;

  /// No description provided for @identityNoKey.
  ///
  /// In fr, this message translates to:
  /// **'Aucune clé utilisable — les pairs ne peuvent pas vérifier notre identité.'**
  String get identityNoKey;

  /// No description provided for @identityKeyBits.
  ///
  /// In fr, this message translates to:
  /// **'Clé RSA-{bits}'**
  String identityKeyBits(Object bits);

  /// No description provided for @identityPubkeyHash.
  ///
  /// In fr, this message translates to:
  /// **'Hash de la clé publique'**
  String get identityPubkeyHash;

  /// No description provided for @identityQuarantined.
  ///
  /// In fr, this message translates to:
  /// **'Clé corrompue mise en quarantaine : {path}'**
  String identityQuarantined(Object path);

  /// No description provided for @identityRotate.
  ///
  /// In fr, this message translates to:
  /// **'Rotation'**
  String get identityRotate;

  /// No description provided for @identityRotateTitle.
  ///
  /// In fr, this message translates to:
  /// **'Rotation de l\'identité ?'**
  String get identityRotateTitle;

  /// No description provided for @identityRotateBody.
  ///
  /// In fr, this message translates to:
  /// **'Une nouvelle paire de clés sera générée : les crédits accordés par les pairs à l\'ancienne identité seront perdus.'**
  String get identityRotateBody;

  /// No description provided for @identityRotateAction.
  ///
  /// In fr, this message translates to:
  /// **'Rotation'**
  String get identityRotateAction;

  /// No description provided for @identitySuiStats.
  ///
  /// In fr, this message translates to:
  /// **'{verified} vérifiée(s) · {rejected} rejetée(s) · {malformed} malformée(s)'**
  String identitySuiStats(Object malformed, Object rejected, Object verified);

  /// No description provided for @identityRefresh.
  ///
  /// In fr, this message translates to:
  /// **'Actualiser'**
  String get identityRefresh;

  /// No description provided for @msgIdentityRotated.
  ///
  /// In fr, this message translates to:
  /// **'Identité régénérée (nouvelle paire de clés)'**
  String get msgIdentityRotated;

  /// No description provided for @msgErrIdentityRotate.
  ///
  /// In fr, this message translates to:
  /// **'Échec de la rotation de l\'identité : {error}'**
  String msgErrIdentityRotate(Object error);

  /// No description provided for @settingsViewSharedTitle.
  ///
  /// In fr, this message translates to:
  /// **'Visibilité de mes fichiers partagés'**
  String get settingsViewSharedTitle;

  /// No description provided for @settingsViewSharedSub.
  ///
  /// In fr, this message translates to:
  /// **'Pairs autorisés à lister nos partages (requête eD2K « View Shared Files »).'**
  String get settingsViewSharedSub;

  /// No description provided for @settingsViewSharedEverybody.
  ///
  /// In fr, this message translates to:
  /// **'Tout le monde'**
  String get settingsViewSharedEverybody;

  /// No description provided for @settingsViewSharedFriends.
  ///
  /// In fr, this message translates to:
  /// **'Amis uniquement'**
  String get settingsViewSharedFriends;

  /// No description provided for @settingsViewSharedNobody.
  ///
  /// In fr, this message translates to:
  /// **'Personne'**
  String get settingsViewSharedNobody;

  /// No description provided for @creditsTitle.
  ///
  /// In fr, this message translates to:
  /// **'Crédits pairs'**
  String get creditsTitle;

  /// No description provided for @creditsSubtitle.
  ///
  /// In fr, this message translates to:
  /// **'Ratio upload/download par pair identifié — réduit le temps d\'attente des bons uploadeurs.'**
  String get creditsSubtitle;

  /// No description provided for @creditsEmpty.
  ///
  /// In fr, this message translates to:
  /// **'Aucun crédit enregistré — importez un clients.met ou uploadez pour accumuler des crédits.'**
  String get creditsEmpty;

  /// No description provided for @creditsPeersCount.
  ///
  /// In fr, this message translates to:
  /// **'{count} pair(s)'**
  String creditsPeersCount(Object count);

  /// No description provided for @creditsActiveCount.
  ///
  /// In fr, this message translates to:
  /// **'{count} actif(s)'**
  String creditsActiveCount(Object count);

  /// No description provided for @creditsColPeer.
  ///
  /// In fr, this message translates to:
  /// **'Pair (hash pubkey)'**
  String get creditsColPeer;

  /// No description provided for @creditsColUploaded.
  ///
  /// In fr, this message translates to:
  /// **'Uploadé'**
  String get creditsColUploaded;

  /// No description provided for @creditsColDownloaded.
  ///
  /// In fr, this message translates to:
  /// **'Téléchargé'**
  String get creditsColDownloaded;

  /// No description provided for @creditsColMultiplier.
  ///
  /// In fr, this message translates to:
  /// **'Multiplicateur'**
  String get creditsColMultiplier;

  /// No description provided for @creditsColLastSeen.
  ///
  /// In fr, this message translates to:
  /// **'Dernière activité'**
  String get creditsColLastSeen;

  /// No description provided for @creditsTotalUploaded.
  ///
  /// In fr, this message translates to:
  /// **'Total uploadé'**
  String get creditsTotalUploaded;

  /// No description provided for @creditsTotalDownloaded.
  ///
  /// In fr, this message translates to:
  /// **'Total téléchargé'**
  String get creditsTotalDownloaded;

  /// No description provided for @creditsRefreshError.
  ///
  /// In fr, this message translates to:
  /// **'Échec du chargement des crédits : {error}'**
  String creditsRefreshError(Object error);

  /// No description provided for @kadFindNodeTooltip.
  ///
  /// In fr, this message translates to:
  /// **'Rechercher un nœud Kad'**
  String get kadFindNodeTooltip;

  /// No description provided for @kadFindNodeTitle.
  ///
  /// In fr, this message translates to:
  /// **'Recherche de nœud Kad'**
  String get kadFindNodeTitle;

  /// No description provided for @kadFindNodeIdLabel.
  ///
  /// In fr, this message translates to:
  /// **'KadID cible (hexadécimal)'**
  String get kadFindNodeIdLabel;

  /// No description provided for @kadFindNodeAction.
  ///
  /// In fr, this message translates to:
  /// **'Rechercher'**
  String get kadFindNodeAction;

  /// No description provided for @kadFindNodeEmpty.
  ///
  /// In fr, this message translates to:
  /// **'Aucun contact trouvé pour cette cible.'**
  String get kadFindNodeEmpty;

  /// No description provided for @kadFindNodeInvalid.
  ///
  /// In fr, this message translates to:
  /// **'KadID hexadécimal de 32 caractères requis'**
  String get kadFindNodeInvalid;

  /// No description provided for @kadFindNodeResults.
  ///
  /// In fr, this message translates to:
  /// **'{count} contact(s) le(s) plus proche(s)'**
  String kadFindNodeResults(Object count);
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
