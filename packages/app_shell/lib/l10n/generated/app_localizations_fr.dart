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
  String get appTagline => 'Client eD2K moderne';

  @override
  String get loopbackLabMode => 'Mode Lab Loopback';

  @override
  String get tooltipLoopbackLabMode =>
      'Mode laboratoire.\nLes connexions sont limitées à 127.0.0.1.\n\nAucun serveur eD2K public, nœud Kad ou pair externe.';

  @override
  String get devModeBadge => 'DEV';

  @override
  String get sectionDownloads => 'TÉLÉCHARGEMENTS';

  @override
  String get sectionCategories => 'CATÉGORIES';

  @override
  String get sectionAuditActivity => 'AUDIT & ACTIVITÉ';

  @override
  String get navHistoryStats => 'Historique & Statistiques';

  @override
  String get navServers => 'Serveurs eD2K';

  @override
  String get navSearch => 'Recherche eD2K';

  @override
  String get navKad => 'Kad DHT';

  @override
  String get navUploads => 'Envois';

  @override
  String get filterAll => 'Tous les téléchargements';

  @override
  String get filterActive => 'Actifs';

  @override
  String get filterPaused => 'En pause';

  @override
  String get filterCompleted => 'Terminés';

  @override
  String get filterFailed => 'Échoués';

  @override
  String get uncategorized => 'Sans catégorie';

  @override
  String get searchPlaceholder => 'Rechercher dans les téléchargements...';

  @override
  String get btnAddLink => 'Ajouter un lien';

  @override
  String get btnAddEd2kLink => 'Ajouter un lien eD2K';

  @override
  String get btnAddDownload => 'Ajouter un téléchargement';

  @override
  String get btnCancel => 'Annuler';

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
  String get actionSettings => 'Paramètres';

  @override
  String get actionImportPartMet => 'Importer un .part.met';

  @override
  String get actionGuidedLabTest => 'Test local guidé';

  @override
  String get tooltipAddCategory => 'Ajouter une catégorie';

  @override
  String get tooltipRefresh => 'Actualiser les téléchargements';

  @override
  String get tooltipDevTools => 'Outils de simulation & débogage';

  @override
  String get tooltipResume => 'Reprendre';

  @override
  String get tooltipPause => 'Mettre en pause';

  @override
  String get tooltipDelete => 'Supprimer';

  @override
  String get emptyStateTitle => 'Aucun téléchargement dans cette vue';

  @override
  String get emptyStateSubtitle =>
      'Ajoutez un lien eD2K pour créer un téléchargement local.\nCe build lab fonctionne en loopback uniquement.';

  @override
  String get zeroSpeedDual => '↓ 0 B/s   ↑ 0 B/s';

  @override
  String get daemonConnected => 'Daemon connecté';

  @override
  String get daemonConnecting => 'Connexion au daemon...';

  @override
  String get daemonReconnecting => 'Reconnexion...';

  @override
  String get daemonError => 'Erreur du daemon';

  @override
  String get daemonDisconnected => 'Daemon déconnecté';

  @override
  String daemonVersionLine(String version, String protocol) {
    return 'Daemon v$version (proto v$protocol)';
  }

  @override
  String get dialogAddTitle => 'Ajouter un téléchargement eD2K';

  @override
  String get dialogFieldLabel => 'URL du lien eD2K';

  @override
  String get dialogFieldHint => 'ed2k://|file|name|size|hash|/';

  @override
  String get dialogValidationPrefix => 'Doit commencer par « ed2k://|file| »';

  @override
  String get dialogValidationParts => 'Doit contenir : nom, taille et hash';

  @override
  String get diagnosticsBadge => 'Diagnostics';

  @override
  String get tooltipDiagnostics =>
      'Cliquer pour inspecter les diagnostics système & répertoire de travail.';

  @override
  String get tooltipSystemDiagnostics =>
      'Diagnostics système & confinement (127.0.0.1)';

  @override
  String get tooltipOpenDrawer => 'Ouvrir le menu de navigation';

  @override
  String get tooltipLabGuide => 'Flux de test local guidé';

  @override
  String get killSwitchBadgeDisabled => 'Désactivé';

  @override
  String get tooltipPolicyKill =>
      'Le sous-système réseau est DÉSACTIVÉ (kill switch engagé). Cliquer pour voir la politique et réactiver.';

  @override
  String get tooltipSessionTotals =>
      'Totaux de session — octets téléchargés / envoyés depuis le démarrage du daemon.\nCliquer pour ouvrir Historique & Statistiques.';

  @override
  String get tooltipLifetimeTotals =>
      'Totaux cumulés depuis l\'installation — téléchargé / envoyé.\nCliquer pour ouvrir Historique & Statistiques. Remise à zéro dans Paramètres → Avancé.';

  @override
  String get settingsLifetimeResetTile => 'Remise à zéro des totaux cumulés';

  @override
  String get settingsLifetimeResetSub =>
      'Remet à zéro les compteurs téléchargé / envoyé depuis l\'installation affichés dans la barre du haut. Les totaux de session sont conservés.';

  @override
  String get lifetimeResetConfirmTitle => 'Remettre à zéro les totaux ?';

  @override
  String get lifetimeResetConfirmBody =>
      'Les compteurs cumulés de téléchargement et d\'envoi depuis l\'installation seront remis à zéro. Les totaux de la session en cours ne sont pas modifiés.';

  @override
  String get lifetimeResetConfirmButton => 'Remettre à zéro';

  @override
  String get renameFileTooltip => 'Renommer le fichier';

  @override
  String get renameFileTitle => 'Renommer le fichier';

  @override
  String get renameFileLabel => 'Nouveau nom de fichier';

  @override
  String get renameFileInvalid =>
      'Nom invalide — non vide, pas \'.\'/\'..\', ne finit pas par \'.\' et sans < > : \" / \\ | ? *';

  @override
  String get renameFileFailed =>
      'Le renommage a échoué côté daemon (nom déjà pris ou fichier verrouillé)';

  @override
  String get renameFileApply => 'Renommer';

  @override
  String get tooltipKillSwitchEnable => 'Réactiver le sous-système réseau';

  @override
  String get tooltipKillSwitchDisable =>
      'Kill switch : déconnecter immédiatement toutes les sockets';

  @override
  String get tooltipIpFilter => 'Filtre IP & limites de débit';

  @override
  String get addEd2kDownload => 'Ajouter un téléchargement eD2K';

  @override
  String get ed2kLink => 'Lien eD2K';

  @override
  String get category => 'Catégorie';

  @override
  String get targetDirectory => 'Dossier de destination';

  @override
  String get defaultDownloadsFolder => 'Dossier de téléchargements par défaut';

  @override
  String get browse => 'Parcourir...';

  @override
  String get diskAllocation => 'Allocation disque';

  @override
  String get cancel => 'Annuler';

  @override
  String get addDownload => 'Ajouter le téléchargement';

  @override
  String get eGMoviesMusicLinux => 'ex : Films, Musique, ISOs Linux';

  @override
  String get inheritGlobalDestination => 'Hériter de la destination globale';

  @override
  String get devSimulationTools => 'Outils de simulation dev :';

  @override
  String selected(String fileName) {
    return 'Sélectionné : \"$fileName\"';
  }

  @override
  String get inject256KbChunk => 'Injecter un bloc de 256 Ko';

  @override
  String get inject1MbChunk => 'Injecter un bloc de 1 Mo';

  @override
  String get clickAnyDownloadRowAbove =>
      'Cliquez sur une ligne de téléchargement ci-dessus pour la sélectionner pour les tests d\'injection.';

  @override
  String get closeDebugBar => 'Fermer la barre de débogage';

  @override
  String get systemLabDiagnostics => 'Diagnostics système & lab';

  @override
  String get workingDirectoryMode => 'Répertoire de travail & mode';

  @override
  String get mode => 'Mode';

  @override
  String get isolatedDataFolder => '(Dossier de données isolé)';

  @override
  String get internalData => 'Données internes';

  @override
  String get downloads => 'Téléchargements';

  @override
  String get daemonProcess => 'Daemon & processus';

  @override
  String get processPid => 'PID du processus';

  @override
  String get rpcEndpoint => 'Endpoint RPC';

  @override
  String get daemonVersion => 'Version du daemon';

  @override
  String get instanceId => 'ID d\'instance';

  @override
  String get sessionId => 'ID de session';

  @override
  String get daemonLog => 'Journal du daemon';

  @override
  String get capabilities => 'Capacités';

  @override
  String get none => 'Aucune';

  @override
  String get copyShareableSanitized =>
      'Copier version partageable (anonymisée)';

  @override
  String get sanitizedShareableDiagnosticsCopied =>
      'Diagnostics anonymisés partageables copiés';

  @override
  String get copyFullLocalDev => 'Copier version complète (dev local)';

  @override
  String get fullDiagnosticsCopiedToClipboard =>
      'Diagnostics complets copiés dans le presse-papiers';

  @override
  String get openLabGuide => 'Ouvrir le guide lab';

  @override
  String get close => 'Fermer';

  @override
  String details(String fileName) {
    return 'Détails : $fileName';
  }

  @override
  String get inspectFullDetails => 'Inspecter tous les détails';

  @override
  String get downloadId => 'ID du téléchargement';

  @override
  String get totalSize => 'Taille totale';

  @override
  String get verifiedBytes => 'Octets vérifiés';

  @override
  String get status => 'Statut';

  @override
  String get sources => 'Sources';

  @override
  String get downloadNotFound => 'Téléchargement introuvable';

  @override
  String get theRequestedDownloadIsNo =>
      'Le téléchargement demandé n\'est plus disponible.';

  @override
  String get importPartMetExperimental =>
      'Importer un .part.met (expérimental)';

  @override
  String get importAnExistingPartialOr =>
      'Importer un téléchargement eMule partiel ou terminé en toute sécurité.';

  @override
  String get pathToPartMetFile => 'Chemin du fichier .part.met';

  @override
  String get inspect => 'Inspecter';

  @override
  String get associatedPartFilePathOptional =>
      'Chemin du fichier .part associé (optionnel, auto-détecté)';

  @override
  String get autoDetectedNextToMet => 'Auto-détecté à côté du fichier .met';

  @override
  String get targetDownloadDirectory =>
      'Dossier de destination du téléchargement';

  @override
  String get targetDirectory2 => 'Dossier de destination';

  @override
  String get categoryOptional => 'Catégorie (optionnel)';

  @override
  String get importDownload => 'Importer le téléchargement';

  @override
  String get compatibilityAlert => 'Alerte de compatibilité';

  @override
  String get retry => 'Réessayer';

  @override
  String get ipFilterRateLimiters => 'Filtre IP & limiteurs de débit';

  @override
  String get localDefensiveIpFilterIpfilter =>
      'Filtre IP défensif local (ipfilter.dat) et quotas de bande passante par seaux de jetons';

  @override
  String connectedDownloadToLoopbackSource(String peerAddr) {
    return 'Téléchargement connecté à la source loopback ($peerAddr)';
  }

  @override
  String cleanupCompletedFileSDeleted(
    String deletedFilesCount,
    String folders,
    String freedBytes,
  ) {
    return 'Nettoyage terminé : $deletedFilesCount fichier(s) supprimé(s) dans $folders ($freedBytes libérés).';
  }

  @override
  String get guidedLocalLoopbackTestFlow => 'Flux de test loopback local guidé';

  @override
  String get fullEndToEndEd2k =>
      'Test eD2K complet de bout en bout sur 127.0.0.1 sans toucher à un terminal.';

  @override
  String get networkPolicyDefenseGuard => 'Politique réseau & garde défensive';

  @override
  String get defensiveSocketAuthorizationAntiSsrf =>
      'Autorisation défensive des sockets, isolation anti-SSRF et quotas de ressources';

  @override
  String get redactedNetworkAuditLogRecent =>
      'Journal d\'audit réseau anonymisé (décisions récentes)';

  @override
  String get sparseInstantGrowsDynamically =>
      'Sparse (instantané, croît dynamiquement)';

  @override
  String get preallocateFullSizeAvoidsFragmentation =>
      'Pré-allouer (taille complète, évite la fragmentation)';

  @override
  String get noneMinimalInitialDiskUse =>
      'Aucune (usage disque initial minimal)';

  @override
  String get sansCategorieNone => 'Sans catégorie (None)';

  @override
  String hash(String hash) {
    return 'Hash : $hash';
  }

  @override
  String get rrggbb => '#RRGGBB';

  @override
  String get normalDefault => 'Normale (défaut)';

  @override
  String get low => 'Basse';

  @override
  String get normal => 'Normal';

  @override
  String get high => 'Haute';

  @override
  String get changeTargetDestination => 'Changer la destination';

  @override
  String selectTargetDestinationFolderFor(String fileName) {
    return 'Choisir le dossier de destination pour \"$fileName\" :';
  }

  @override
  String get targetDirectoryPath => 'Chemin du dossier de destination';

  @override
  String get eGDownloadsOrC => 'ex : downloads ou C:\\TargetDir';

  @override
  String get theFolderMustExistAnd =>
      'Le dossier doit exister et être accessible en écriture. Les données .part actives restent dans data/temp/ jusqu\'à la fin.';

  @override
  String get saveDestination => 'Enregistrer la destination';

  @override
  String copiedToClipboard(String label) {
    return '$label copié dans le presse-papiers';
  }

  @override
  String get resume => 'Reprendre';

  @override
  String get pause => 'Pause';

  @override
  String get retryLoopbackSource => 'Réessayer la source loopback';

  @override
  String get openCompletedFile => 'Ouvrir le fichier terminé';

  @override
  String get remove => 'Supprimer...';

  @override
  String get diagnosticsRedactedExport => 'Diagnostics & export anonymisé';

  @override
  String get exportSanitizedDiagnosticsZeroAuth =>
      'Exporter les diagnostics anonymisés (zéro secret de jeton d\'auth, vérification du confinement loopback 127.0.0.1, noms d\'utilisateur et chemins masqués).';

  @override
  String get copyRedactedDiagnostic => 'Copier le diagnostic anonymisé';

  @override
  String get downloadDiagnosticCopiedToClipboard =>
      'Diagnostic du téléchargement copié (anonymisé : aucun secret)';

  @override
  String get identityEd2kLinks => 'Identité & liens eD2K';

  @override
  String get downloadUuid => 'UUID du téléchargement';

  @override
  String get ed2kHashMd4 => 'Hash eD2K (MD4)';

  @override
  String get canonicalEd2kLink => 'Lien eD2K canonique';

  @override
  String get transferMetricsSpeed => 'Métriques de transfert & vitesse';

  @override
  String get received => 'Reçu';

  @override
  String get remaining => 'Restant';

  @override
  String get verified => 'Vérifié';

  @override
  String get currentRate => 'Débit actuel';

  @override
  String get averageRate => 'Débit moyen';

  @override
  String get eta => 'ETA';

  @override
  String get attempts => 'Tentatives';

  @override
  String lastError(String lastError) {
    return 'Dernière erreur : $lastError';
  }

  @override
  String ed2kPartMapPartsOf(String length) {
    return 'Carte des parts eD2K ($length parts de 9,28 Mo)';
  }

  @override
  String get partBreakdownIsInitializing =>
      'Initialisation de la répartition des parts...';

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
    return 'Part #$partIndex Plage : $startOffset - $endOffset Taille : $size Reçu : $receivedBytes Statut : $status';
  }

  @override
  String verified2(String verifiedCount) {
    return 'Vérifiées ($verifiedCount)';
  }

  @override
  String completed(String completedCount) {
    return 'Terminées ($completedCount)';
  }

  @override
  String downloading(String downloadingCount) {
    return 'En téléchargement ($downloadingCount)';
  }

  @override
  String missing(String missingCount) {
    return 'Manquantes ($missingCount)';
  }

  @override
  String noSource(String unavailableCount) {
    return 'Aucune source ($unavailableCount)';
  }

  @override
  String get storageTargetDestination => 'Stockage & destination';

  @override
  String get temporaryPartPathConfined => 'Chemin .part temporaire (confiné)';

  @override
  String get finalTargetDestination => 'Destination finale';

  @override
  String get changeDestination => 'Changer la destination';

  @override
  String get noteChangingDestinationRedirectsWhere =>
      'Note : changer la destination redirige l\'endroit où le fichier terminé est assemblé. Les blocs .part en cours restent en sécurité dans le dossier temporaire.';

  @override
  String get removeDownload => 'Supprimer le téléchargement ?';

  @override
  String removeFromTheDownloadsManager(String fileName) {
    return 'Retirer \"$fileName\" du gestionnaire de téléchargements ?';
  }

  @override
  String get theDownloadEntryInSqlite =>
      '• L\'entrée du téléchargement dans SQLite sera définitivement supprimée.\n• Les fichiers temporaires (.part et .part.met) dans data/temp/ seront supprimés.\n• Si le téléchargement est terminé, une option ci-dessous permet aussi de supprimer le fichier final dans le dossier de destination.';

  @override
  String get deleteTemporaryPartFileFrom =>
      'Supprimer le fichier .part temporaire de data/temp/';

  @override
  String get confirmRemove => 'Confirmer la suppression';

  @override
  String get sourcesEd2kSessionPeer => 'SOURCES ED2K & PAIRS DE SESSION';

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
  String get noCategory => 'Sans catégorie';

  @override
  String get totalSize2 => 'Taille totale :';

  @override
  String get estAcquiredGapsComplement =>
      'Acquis est. / complément des trous :';

  @override
  String get partCount => 'Nombre de parts :';

  @override
  String get ed2kHash => 'Hash eD2K';

  @override
  String get importSuccessfulTheDownloadIs =>
      'Import réussi ! Le téléchargement est en pause et les parts vérifiées ont été préparées.';

  @override
  String get auditReport => 'Rapport d\'audit';

  @override
  String get auditReportCopiedToClipboard =>
      'Rapport d\'audit copié dans le presse-papiers';

  @override
  String get copyReport => 'Copier le rapport';

  @override
  String get done => 'Terminé';

  @override
  String get theDownloadWillBeStaged =>
      'Le téléchargement sera préparé dans data/temp/ et initialisé en pause. L\'import ne contacte aucun pair eD2K.';

  @override
  String get confirmCleanup => 'Confirmer le nettoyage';

  @override
  String thisWillRemoveFixtureFile(String fixtureCount) {
    return 'Cela supprimera $fixtureCount fichier(s) fixture dans <workdir>/fixtures/ et nettoiera les fichiers .part temporaires.';
  }

  @override
  String get deleteTestData => 'Supprimer les données de test';

  @override
  String get storageDataInternalStateVs =>
      'Stockage : data/ (état interne) vs downloads/ (fichiers terminés)';

  @override
  String get openFolder => 'Ouvrir le dossier';

  @override
  String internalDataDbTempPart(String workdir) {
    return '• Données internes (DB, .part temporaires, fixtures) : $workdir';
  }

  @override
  String completedDownloads(String downloadsDir) {
    return '• Téléchargements terminés : $downloadsDir';
  }

  @override
  String get cleanTestDataFixturesIncomplete =>
      'Nettoyer les données de test (fixtures & parts incomplètes)';

  @override
  String get cleanupTestData => 'Nettoyer les données de test';

  @override
  String get connectDownloadToLoopbackSource =>
      'Connecter le téléchargement à la source loopback';

  @override
  String get noDownloadsInQueueAdd =>
      'Aucun téléchargement en file. Ajoutez un lien eD2K à l\'étape 2.';

  @override
  String get targetDownloadTask => 'Tâche de téléchargement cible';

  @override
  String warningDownloadSize(String totalBytes, String fixtureSize) {
    return 'Attention : la taille du téléchargement ($totalBytes) ne correspond pas à la fixture ($fixtureSize).';
  }

  @override
  String get startLoopbackPeerInStep =>
      'Démarrez le pair loopback à l\'étape 3 avant de connecter.';

  @override
  String finalFile(String fileName, String downloadsDir) {
    return 'Fichier final : $downloadsDir\\$fileName';
  }

  @override
  String get generateLocalTestFixture => 'Générer une fixture de test locale';

  @override
  String get obtainCanonicalEd2kLinkAdd =>
      'Obtenir le lien eD2K canonique & ajouter aux téléchargements';

  @override
  String get copyLink => 'Copier le lien';

  @override
  String get ed2kLinkCopiedToClipboard =>
      'Lien eD2K copié dans le presse-papiers';

  @override
  String get downloadTaskCreatedInUi =>
      'Tâche de téléchargement créée dans l\'UI';

  @override
  String get completeStep1ToGenerate =>
      'Terminez l\'étape 1 pour générer d\'abord une fixture eD2K.';

  @override
  String get loopbackTestPeer1270 => 'Pair de test loopback (127.0.0.1)';

  @override
  String get lastImportReport => 'Dernier rapport d\'import';

  @override
  String invalidLinesSkipped(String invalidLines) {
    return 'Lignes invalides ignorées : $invalidLines';
  }

  @override
  String rangesLoaded(String rangeCount) {
    return '$rangeCount plages chargées';
  }

  @override
  String totalBlockedConnectionAttempts(String hitsCount) {
    return 'Connexions bloquées au total : $hitsCount';
  }

  @override
  String get importIpfilterDat => 'Importer un ipfilter.dat';

  @override
  String get loadALocalPeerguardianEmule =>
      'Charge un fichier ipfilter.dat local au format PeerGuardian / eMule. Le téléchargement depuis une URL distante est interdit par sécurité (anti-SSRF).';

  @override
  String get import => 'Importer';

  @override
  String get clear => 'Effacer';

  @override
  String get noNetworkEventsRecordedYet =>
      'Aucun événement réseau enregistré pour le moment.';

  @override
  String get activeConnectionsLimits => 'Connexions actives & limites';

  @override
  String get peerSockets => 'Sockets pairs';

  @override
  String get serverSockets => 'Sockets serveur';

  @override
  String get listeners => 'Listeners d\'écoute';

  @override
  String get connectTimeout => 'Timeout de connexion';

  @override
  String get maxFrameSize => 'Taille max d\'une trame';

  @override
  String get maxBufferMemory => 'Mémoire tampon max';

  @override
  String status2(String label) {
    return 'Statut : $label';
  }

  @override
  String get reEnable => 'Réactiver le réseau';

  @override
  String get disconnectAll => 'Tout déconnecter (kill switch)';

  @override
  String get bandwidthTokenBucketQuotas => 'Limites de débit (Token Buckets)';

  @override
  String get limitsAreEnforcedAsynchronouslyUsing =>
      'Les limites sont appliquées de façon asynchrone par des Token Buckets et persistées au redémarrage. 0 = illimité.';

  @override
  String get downloadLimitKbS0 =>
      'Limite de téléchargement (Ko/s) [0 = illimité]';

  @override
  String get uploadLimitKbS0 => 'Limite d\'envoi (Ko/s) [0 = illimité]';

  @override
  String get maxNewConnectionsMinute => 'Nouvelles connexions max / minute';

  @override
  String get saveLimits => 'Enregistrer les limites';

  @override
  String get inspectDetailsPartMap => 'Inspecter détails & carte des parts';

  @override
  String get remove2 => 'Retirer';

  @override
  String removeFromDownloads(String fileName) {
    return 'Retirer \"$fileName\" des téléchargements ?';
  }

  @override
  String get deleteDownloadedPartTempFiles =>
      'Supprimer les fichiers .part/temp téléchargés du disque';

  @override
  String get deleteSharedCompletedFile =>
      'Supprimer aussi le fichier partagé dans le dossier de destination';

  @override
  String id(String id) {
    return 'ID : $id';
  }

  @override
  String src(String sourceCount) {
    return '$sourceCount src';
  }

  @override
  String fullId(String id) {
    return 'ID complet : $id';
  }

  @override
  String get downloadIdCopiedToClipboard =>
      'ID du téléchargement copié dans le presse-papiers';

  @override
  String sources2(String sourceCount) {
    return '$sourceCount sources';
  }

  @override
  String get downloadSettingsStorage =>
      'Paramètres de téléchargement & stockage';

  @override
  String get globalDefaultDownloadDirectory =>
      'Dossier de téléchargement par défaut';

  @override
  String get thisPathIsUsedAs =>
      'Ce chemin est la destination par défaut des nouveaux téléchargements. Les fichiers .part restent dans le dossier temporaire configuré jusqu\'à complétion (eMule « Répertoire »).';

  @override
  String get destinationDirectoryPath => 'Chemin du dossier de destination';

  @override
  String get eGCDownloadsOr => 'ex. C:\\Downloads ou downloads';

  @override
  String get save => 'Enregistrer';

  @override
  String get resetToPortableDefault => 'Réinitialiser au dossier portable';

  @override
  String get modeReseauTraverseeBoxUpnp => 'Mode Réseau & Traversée Box (UPnP)';

  @override
  String get basculeEntreLaboratoireLocalEt =>
      'Bascule entre laboratoire local et réseau public mondial.';

  @override
  String get configurer => 'Configurer';

  @override
  String get historyRetentionPolicy => 'Rétention de l\'historique';

  @override
  String get historicalTransitionEventsOlderThan =>
      'Les événements d\'historique plus anciens que la durée choisie sont purgés au démarrage du daemon et à chaque modification.';

  @override
  String get keepHistoryFor => 'Conserver l\'historique pendant : ';

  @override
  String get indefinitelyNoPruning => 'Indéfiniment (aucune purge)';

  @override
  String delete(String name) {
    return 'Supprimer \"$name\" ?';
  }

  @override
  String get deletingThisCategoryWillMove =>
      'Deleting this category will move all associated downloads to \"Sans catégorie\" (Uncategorized).\n\nNo downloads or files will be deleted.';

  @override
  String get deleteCategory => 'Supprimer la catégorie';

  @override
  String get categoryOptions => 'Options de catégorie';

  @override
  String get editCategory => 'Modifier la catégorie';

  @override
  String get clearDownloadHistory =>
      'Effacer l\'historique des téléchargements ?';

  @override
  String get areYouSureYouWant =>
      'Voulez-vous vraiment supprimer définitivement tout l\'historique et les statistiques de téléchargement ? Cette action est irréversible.';

  @override
  String get clearAllHistory => 'Effacer tout l\'historique';

  @override
  String get noDownloadHistoryRecordedYet =>
      'Aucun historique de téléchargement enregistré';

  @override
  String get completedResumedFailedAndRemoved =>
      'Les événements de transfert terminés, repris, échoués et supprimés apparaîtront ici.';

  @override
  String eventDetail(String fileName) {
    return 'Détail de l\'événement : $fileName';
  }

  @override
  String get viewDetails => 'Voir les détails';

  @override
  String of2(String transferredBytes, String totalBytes) {
    return '$transferredBytes sur $totalBytes';
  }

  @override
  String hash2(String ed2kHash) {
    return 'Hash : $ed2kHash';
  }

  @override
  String of3(String totalBytes) {
    return 'sur $totalBytes';
  }

  @override
  String res(String resumeCount, String connectionAttempts) {
    return '$resumeCount res. / $connectionAttempts att.';
  }

  @override
  String exportHistory(String toUpperCase) {
    return 'Exporter l\'historique ($toUpperCase)';
  }

  @override
  String get reportIsSanitizedAllLocal =>
      'Le rapport est anonymisé : tous les noms d\'utilisateur et chemins locaux sont masqués.';

  @override
  String get copyToClipboard => 'Copier dans le presse-papiers';

  @override
  String get exportCopiedToClipboard => 'Export copié dans le presse-papiers';

  @override
  String get filterByEvent => 'Filtrer par événement :';

  @override
  String get allEvents => 'Tous les événements';

  @override
  String get completed2 => 'Terminés';

  @override
  String get failed => 'Échoués';

  @override
  String get resumed => 'Repris';

  @override
  String get verifying => 'Vérification';

  @override
  String get removed => 'Supprimés';

  @override
  String recordS(String length) {
    return '$length enregistrement(s)';
  }

  @override
  String get totalCompleted => 'Total terminés';

  @override
  String get activityWindows => 'Fenêtres d\'activité';

  @override
  String get resumesRetries => 'Reprises & tentatives';

  @override
  String failuresLogged(String totalFailedCount) {
    return '$totalFailedCount échecs enregistrés';
  }

  @override
  String get integrityHealth => 'Intégrité';

  @override
  String get historyLocalStatistics => 'Historique & statistiques locales';

  @override
  String get exportHistoryReport => 'Exporter le rapport d\'historique';

  @override
  String get exportAsJsonSanitized => 'Exporter en JSON (anonymisé)';

  @override
  String get exportAsCsvSanitized => 'Exporter en CSV (anonymisé)';

  @override
  String get exportReport => 'Exporter le rapport';

  @override
  String get clearHistory => 'Effacer l\'historique';

  @override
  String get refresh => 'Actualiser';

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
  String get kadId128BitXor => 'ID Kad (XOR 128 bits)';

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
  String get envoyerKadPing0x60 => 'Envoyer un ping Kad (0x60)';

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
  String get extensionExIsoZipAvi => 'Extension (ex : iso, zip, avi)';

  @override
  String get tailleMinMo => 'Taille Min (Mo)';

  @override
  String get tailleMaxMo => 'Taille Max (Mo)';

  @override
  String get effacerFiltres => 'Effacer les filtres';

  @override
  String get serveurEd2k => 'Serveur eD2K';

  @override
  String get globalUdp => 'Global (UDP)';

  @override
  String get nomDuFichier => 'Nom du fichier';

  @override
  String get taille => 'Taille';

  @override
  String get hachageEd2k => 'Hash eD2K';

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
  String get copierLienEd2k => 'Copier le lien eD2K';

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
  String get selectionnerServerMet => 'Sélectionner un server.met';

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
  String get etatFiltre => 'État / Filtre';

  @override
  String get nomDuServeur => 'Nom du Serveur';

  @override
  String get ipPort => 'IP : Port';

  @override
  String get ping => 'Ping';

  @override
  String get description => 'Description';

  @override
  String get preference => 'Préférence';

  @override
  String get seConnecter => 'Se connecter';

  @override
  String get supprimer => 'Supprimer';

  @override
  String get gestionLocaleDesServeursEd2k => 'Gestion Locale des Serveurs eD2K';

  @override
  String get importerServerMet => 'Importer un server.met';

  @override
  String get mettreAJourDepuisUrl => 'Mettre à jour depuis URL';

  @override
  String get mettreAJourDepuisUne => 'Mettre à jour depuis une URL';

  @override
  String get indiquezLAdresseHttpOu =>
      'Indiquez l\'adresse HTTP ou HTTPS d\'un fichier server.met distant pour mettre à jour automatiquement votre annuaire de serveurs.';

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
  String get autorises => 'Autorisés';

  @override
  String get bloquesFiltreIp => 'Bloqués (filtre IP)';

  @override
  String get lesServeursMarquesEnRouge =>
      'Les serveurs marques en rouge correspondent a des plages de votre filtre IP local. Ils seront importes avec le statut bloque.';

  @override
  String get bloque => 'Bloqué';

  @override
  String get autorise => 'Autorisé';

  @override
  String get confirmerImportation => 'Confirmer l\'importation';

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
  String get sevenDays => '7 jours';

  @override
  String get thirtyDaysDefault => '30 jours (défaut)';

  @override
  String get ninetyDays => '90 jours';

  @override
  String twentyFourHours(
    String last24hCompletedCount,
    String last7dCompletedCount,
  ) {
    return '24 h : $last24hCompletedCount | 7 j : $last7dCompletedCount';
  }

  @override
  String receivedOfTotal(
    String receivedBytes,
    String totalBytes,
    String progress,
  ) {
    return '$receivedBytes sur $totalBytes ($progress)';
  }

  @override
  String get serverEnable => 'Activer';

  @override
  String get serverDisable => 'Désactiver';

  @override
  String get eventType => 'Type d\'événement';

  @override
  String get statusLabel => 'Statut';

  @override
  String get transferred => 'Transféré';

  @override
  String get duration => 'Durée';

  @override
  String get resumes => 'Reprises';

  @override
  String get verificationResult => 'Résultat de la vérification';

  @override
  String get sanitizedPath => 'Chemin anonymisé';

  @override
  String get timestamp => 'Horodatage';

  @override
  String get errorMessage => 'Message d\'erreur';

  @override
  String get language => 'Langue';

  @override
  String get languageDescription => 'Langue de l\'interface utilisateur';

  @override
  String get french => 'Français';

  @override
  String get english => 'English';

  @override
  String get daemonConnectTitle => 'Connexion au daemon';

  @override
  String get daemonConnectDescription =>
      'Emule-Sion Web pilote un mule-daemon distant. Indiquez l\'adresse WebSocket du daemon.';

  @override
  String get daemonUrlLabel => 'Adresse du daemon';

  @override
  String get daemonUrlHint => 'ws://192.168.1.10:4660/rpc';

  @override
  String get daemonUrlInvalid =>
      'Adresse invalide — format attendu : ws://hôte:port/rpc';

  @override
  String get connect => 'Se connecter';

  @override
  String get changeDaemon => 'Changer de daemon';

  @override
  String get sectionUploads => 'Envois';

  @override
  String get uploadsActive => 'UPLOADS ACTIFS';

  @override
  String get uploadsQueue => 'FILE D\'ATTENTE';

  @override
  String get uploadsActiveEmpty => 'Aucun upload actif';

  @override
  String get uploadsQueueEmpty => 'File d\'attente vide';

  @override
  String get uploadsClient => 'Client';

  @override
  String get uploadsFile => 'Fichier';

  @override
  String get uploadsRate => 'Débit';

  @override
  String get uploadsRank => 'Rang';

  @override
  String get uploadsCancelTooltip => 'Annuler l\'upload';

  @override
  String get settingsSectionGeneral => 'Général';

  @override
  String get settingsSectionConnection => 'Connexion';

  @override
  String get settingsSectionTransfers => 'Transferts';

  @override
  String get settingsSectionSecurity => 'Sécurité';

  @override
  String get settingsSectionFriends => 'Amis';

  @override
  String get settingsSectionAdvanced => 'Avancé';

  @override
  String get settingsGeneralSubtitle =>
      'Identité, dossiers, partage automatique, historique et langue';

  @override
  String get settingsConnectionSubtitle =>
      'Profil réseau, isolation, budgets de sockets et UPnP';

  @override
  String get settingsTransfersSubtitle =>
      'Limites de connexions aux sources du swarm';

  @override
  String get settingsSecuritySubtitle =>
      'Filtre IP, limites de débit et politique anti-SSRF';

  @override
  String get settingsFriendsSubtitle =>
      'Slot d\'upload dédié sans passer par la file (friend slot eMule)';

  @override
  String get settingsAdvancedSubtitle =>
      'Diagnostic, outils de laboratoire et informations daemon';

  @override
  String get settingsApply => 'Appliquer';

  @override
  String get settingsSaving => 'Enregistrement…';

  @override
  String get settingsNicknameLabel => 'Pseudo (nickname)';

  @override
  String get settingsNicknameHelper =>
      'Annoncé aux serveurs eD2K et aux pairs (eMule « nick » ; vide = défaut)';

  @override
  String get settingsShareDownloadDirTitle =>
      'Partager automatiquement le dossier de téléchargement';

  @override
  String get settingsShareDownloadDirSub =>
      'Les fichiers du dossier Incoming sont hashés et servis aux pairs au démarrage du daemon (comme le dossier Incoming d\'eMule).';

  @override
  String get settingsTempDirLabel => 'Dossier temporaire (.part)';

  @override
  String get settingsTempDirHelper =>
      'Où sont stockés les fichiers en cours (eMule « Répertoire temporaire » ; vide = dossier du daemon)';

  @override
  String get settingsIchTitle => 'ICH — vérification intelligente des parts';

  @override
  String get settingsIchSub =>
      'Vérifie chaque part complétée par MD4 (eMule « ICH » ; désactiver saute la vérification intermédiaire)';

  @override
  String get settingsRememberCancelledTitle => 'Mémoriser les fichiers annulés';

  @override
  String get settingsRememberCancelledSub =>
      'Ajoute une entrée d\'historique à l\'annulation (eMule « Remember cancelled files »)';

  @override
  String get settingsRememberDownloadedTitle =>
      'Mémoriser les fichiers téléchargés';

  @override
  String get settingsRememberDownloadedSub =>
      'Ajoute une entrée d\'historique à la complétion (eMule « Remember downloaded files »)';

  @override
  String get settingsDirEmptyError =>
      'Le chemin du dossier ne peut pas être vide.';

  @override
  String get settingsDirSaved =>
      'Dossier de téléchargement par défaut mis à jour.';

  @override
  String get settingsDirProbeFailed =>
      'Le test d\'écriture dans le dossier a échoué.';

  @override
  String get settingsDirResetDone => 'Dossier portable par défaut restauré.';

  @override
  String get settingsDirResetFailed =>
      'Échec de la réinitialisation du dossier de téléchargement.';

  @override
  String get settingsCapPortsTitle => 'Capacités & ports';

  @override
  String get settingsCapDlLabel => 'Capacité réception (Ko/s)';

  @override
  String get settingsCapDlHint =>
      'Vitesse de ligne descendante — borne la limite de download à 90 % (0 = aucune)';

  @override
  String get settingsCapUlLabel => 'Capacité émission (Ko/s)';

  @override
  String get settingsCapUlHint =>
      'Vitesse de ligne montante — référence USS, borne la limite d\'upload à 80 % (0 = aucune)';

  @override
  String get settingsTcpPortLabel => 'Port TCP';

  @override
  String get settingsTcpPortHint =>
      'Port client eD2K — rebind immédiat du listener';

  @override
  String get settingsUdpPortLabel => 'Port UDP';

  @override
  String get settingsUdpPortHint => 'Port Kad/UDP — socket rebind à chaud';

  @override
  String get settingsUdpEnabledTitle => 'UDP activé';

  @override
  String get settingsUdpEnabledSub =>
      'Kad UDP, requêtes serveur UDP et re-ask UDP (eMule « UDP désactivé »)';

  @override
  String get settingsUpnpToggleTitle => 'Configurer les ports grâce à l\'UPnP';

  @override
  String get settingsUpnpToggleSub =>
      'Mappe/démappe automatiquement TCP+UDP sur la box';

  @override
  String get settingsProtocolTitle => 'Protocole & limites';

  @override
  String get settingsObfEnabledTitle => 'Connexions obfusquées supportées';

  @override
  String get settingsObfEnabledSub =>
      'Chiffrement de protocole eD2K/Kad (eMule « Support obfuscated connections »)';

  @override
  String get settingsObfRequiredTitle => 'Obfuscation requise';

  @override
  String get settingsObfRequiredSub =>
      'Rejette les pairs sans chiffrement (eMule « Require obfuscated connections »)';

  @override
  String get settingsUssTitle => 'USS — Détection de la vitesse d\'envoi';

  @override
  String get settingsUssSub =>
      'Ajuste dynamiquement la limite d\'upload selon la latence mesurée';

  @override
  String get settingsMaxConnLabel => 'Connexions max';

  @override
  String get settingsMaxConnHint =>
      'Budget global de sockets pairs (eMule MaxConnections)';

  @override
  String get settingsConnPerMinLabel => 'Connexions / min';

  @override
  String get settingsConnPerMinHint =>
      'Nouvelles connexions sortantes par minute (eMule ≈ 240)';

  @override
  String get settingsServersTitle => 'Serveurs eD2K';

  @override
  String get settingsServerReconnectTitle => 'Reconnexion automatique';

  @override
  String get settingsServerReconnectSub =>
      'Reconnecte au dernier serveur après une perte de connexion (eMule « Reconnect »)';

  @override
  String get settingsServerAutoconnectTitle => 'Connexion au démarrage';

  @override
  String get settingsServerAutoconnectSub =>
      'Se connecte automatiquement au meilleur serveur connu au lancement (eMule « Autoconnect »)';

  @override
  String get settingsServerAutoremoveTitle => 'Supprimer les serveurs morts';

  @override
  String get settingsServerAutoremoveSub =>
      'Retire un serveur de la liste dès le premier échec de reconnexion (eMule « DeadServerRetry » = 1)';

  @override
  String get settingsSafeConnectTitle => 'Connexion sûre';

  @override
  String get settingsSafeConnectSub =>
      'Bloque la reprise des downloads tant que ni le serveur eD2K ni Kad ne sont disponibles';

  @override
  String get settingsServerAutoupdateTitle =>
      'Mettre à jour la liste au démarrage';

  @override
  String get settingsServerAutoupdateSub =>
      'Importe le fichier server.met ci-dessous au lancement du daemon';

  @override
  String get settingsServerMetLabel => 'Fichier server.met';

  @override
  String get settingsServerMetHint =>
      'Chemin local du fichier .met importé au démarrage (aucun fetch HTTP)';

  @override
  String upnpConfiguredMsg(String ip) {
    return 'UPnP configuré (IP WAN : $ip).';
  }

  @override
  String get upnpNoGateway =>
      'Passerelle UPnP non détectée ou désactivée sur votre box.';

  @override
  String upnpRefreshFailed(String error) {
    return 'Échec de rafraîchissement UPnP : $error';
  }

  @override
  String settingsInvalidNumMin(String label, String value, int min) {
    return '$label : « $value » invalide (entier ≥ $min attendu)';
  }

  @override
  String settingsInvalidNumRange(String label, String value, int min, int max) {
    return '$label : « $value » invalide (entier entre $min et $max attendu)';
  }

  @override
  String get killSwitchConfirmTitle => 'Couper toutes les connexions ?';

  @override
  String get killSwitchConfirmBody =>
      'Le kill switch coupe immédiatement toutes les sockets réseau (pairs, serveurs, listeners) jusqu\'à réactivation manuelle.';

  @override
  String get killSwitchConfirmAction => 'Tout déconnecter';

  @override
  String get settingsMaxGlobalLabel => 'Sources maximum (global)';

  @override
  String get settingsMaxGlobalHint =>
      'Nombre total de sources eD2K/Kad simultanées, tous fichiers confondus';

  @override
  String get settingsMaxPerFileLabel => 'Sources maximum par fichier';

  @override
  String get settingsMaxPerFileHint =>
      'Plafond par défaut — surchargeable par fichier (FT_MAXSOURCES)';

  @override
  String get settingsMinPerFileLabel => 'Sources minimum par fichier';

  @override
  String get settingsMinPerFileHint =>
      'Quota de sources garanti par fichier même si le swarm global est saturé';

  @override
  String get settingsMaxQueueLabel => 'File d\'attente upload max';

  @override
  String get settingsMaxQueueHint =>
      'Clients en attente d\'un slot upload (eMule QueueSize, défaut 5000)';

  @override
  String get settingsMinUploadLabel => 'Débit upload min par slot (Ko/s)';

  @override
  String get settingsMinUploadHint =>
      'N\'ouvre pas de nouveau slot sous ce seuil (eMule MinUpload, défaut 1)';

  @override
  String get settingsDailyQuotaLabel =>
      'Quota upload quotidien (Gio, 0 = illimité)';

  @override
  String get settingsDailyQuotaHint =>
      'Option Emule-Sion : au-delà du quota, plus aucun slot d\'upload n\'est accordé. Remise à zéro chaque jour (UTC).';

  @override
  String get settingsAutoPriorityTitle =>
      'Priorité auto des nouveaux downloads';

  @override
  String get settingsAutoPrioritySub =>
      'Les fichiers ≥ 1 part eD2K (9,28 Mio) démarrent en priorité « high »';

  @override
  String get settingsSwarmHealingTitle => 'Sauvetage de swarm (swarm healing)';

  @override
  String get settingsSwarmHealingSub =>
      'Option Emule-Sion : télécharge en priorité les parts les plus rares du swarm (peu de sources restantes) pour devenir une source de secours avant leur disparition';

  @override
  String get settingsExclusiveTitle => 'Options exclusives Emule-Sion';

  @override
  String get settingsParanoidTitle => 'Mode paranoid';

  @override
  String get settingsParanoidSub =>
      'Ne sert que les pairs obfusqués dont la signature Secure Ident a été vérifiée — les clients eMule classiques sont refusés (réduit fortement la compatibilité)';

  @override
  String get settingsIdentityRotationLabel =>
      'Rotation d\'identité (heures, 0 = désactivée)';

  @override
  String get settingsIdentityRotationHelper =>
      'Régénère la clé RSA, le user hash eD2K et le KadID périodiquement. Invalide les crédits accumulés — identité volontairement éphémère.';

  @override
  String settingsInvalidLimits(String fields) {
    return 'Limites invalides : $fields';
  }

  @override
  String get settingsLimitDlField => 'limite de réception (Ko/s)';

  @override
  String get settingsLimitUlField => 'limite d\'émission (Ko/s)';

  @override
  String get settingsLimitConnField => 'connexions/minute';

  @override
  String settingsIdentityRotationInvalid(String value) {
    return 'Rotation d\'identité : « $value » invalide (entier entre 0 et 8760 attendu)';
  }

  @override
  String get rateLimitsCapacityNote =>
      'Les limites sont bornées à 90 % (réception) / 80 % (émission) des capacités de ligne déclarées (onglet Connexion).';

  @override
  String get settingsDiagnosticTile => 'Diagnostic du daemon';

  @override
  String get settingsDiagnosticSub =>
      'État de santé, connexions et statistiques internes';

  @override
  String get settingsImportPartTile => 'Importer un .part.met';

  @override
  String get settingsImportPartSub =>
      'Reprendre un téléchargement eMule existant';

  @override
  String get settingsLabGuideTile => 'Guide du mode laboratoire';

  @override
  String get settingsLabGuideSub =>
      'Scénarios de test loopback sans trafic WAN';

  @override
  String get settingsDebugBarTitle => 'Barre de simulation debug';

  @override
  String get settingsDebugBarOn =>
      'Affiche la barre de simulation en bas de l\'écran';

  @override
  String get settingsDebugBarOff =>
      'Disponible uniquement avec les dev tools (ENABLE_DEV_TOOLS ou fake daemon)';

  @override
  String settingsDaemonVersion(String version, String protocol) {
    return 'Daemon $version — protocole $protocol';
  }

  @override
  String get settingsAddFriendTitle => 'Ajouter un ami';

  @override
  String get settingsUserHashLabel => 'UserHash (hex)';

  @override
  String get settingsUserHashHint => '32 caractères hexadécimaux';

  @override
  String get settingsFriendNameLabel => 'Nom (optionnel)';

  @override
  String get settingsNoFriends => 'Aucun ami configuré.';

  @override
  String get settingsRemoveFriendTooltip => 'Retirer';

  @override
  String settingsRetentionDaysCustom(int days) {
    return '$days jours';
  }

  @override
  String get settingsDirCustomHelper =>
      'Dossier personnalisé (persisté en base, appliqué immédiatement)';

  @override
  String get settingsDirPortableHelper =>
      'Dossier portable par défaut (<racine_du_bundle>/downloads)';

  @override
  String get netBannerKillActive =>
      'Kill switch actif : toutes les sockets sont coupées, connexions entrantes et sortantes bloquées.';

  @override
  String get netBannerPolicyActive =>
      'Chaque socket est soumise aux contraintes du profil réseau actif (permis, budgets, filtre IP).';

  @override
  String get upnpGatewayConnected => 'Connectée';

  @override
  String get upnpGatewayAbsent => 'Non détectée / Désactivée';

  @override
  String get upnpIpUnknown => 'Inconnue';

  @override
  String get upnpMappedHighId => 'Mappé (HighID)';

  @override
  String get upnpMappedOpen => 'Mappé (Ouvert)';

  @override
  String get upnpNotMapped => 'Non mappé';

  @override
  String get ipFilterActive => 'Filtre actif';

  @override
  String get ipFilterDisabled => 'Filtre désactivé';

  @override
  String get networksCardTitle => 'Réseaux';

  @override
  String get networkEd2kTitle => 'Réseau eD2K';

  @override
  String get networkKadTitle => 'Réseau Kad';

  @override
  String get netProfileEd2kDesc =>
      'TCP sortant vers des pairs/serveurs eD2K publics. Rejette le LAN privé & la boucle locale (anti-SSRF).';

  @override
  String get netProfileKadDesc =>
      'DHT Kademlia — UDP sortant pour le routage WAN. Activer Kad active aussi eD2K.';

  @override
  String ipReportTotalLines(int count) {
    return 'Lignes lues : $count';
  }

  @override
  String ipReportValidEntries(int count) {
    return 'Entrées valides : $count';
  }

  @override
  String ipReportCoalesced(int count) {
    return 'Plages fusionnées : $count';
  }

  @override
  String ipReportFinalRanges(int count) {
    return 'Intervalles actifs finaux : $count';
  }

  @override
  String get connDisconnected => 'Déconnecté';

  @override
  String get connConnecting => 'Connexion';

  @override
  String get connConnectingEllipsis => 'Connexion...';

  @override
  String get connConnected => 'Connecté';

  @override
  String get connDisconnecting => 'Déconnexion...';

  @override
  String get connError => 'Erreur';

  @override
  String get peerHandshaking => 'Handshake eD2K...';

  @override
  String get peerQueued => 'En file d\'attente';

  @override
  String get peerTransferring => 'Téléchargement';

  @override
  String get dlStatusQueued => 'En file';

  @override
  String get dlStatusWaitingForSources => 'Recherche de sources';

  @override
  String get dlStatusAllocating => 'Allocation';

  @override
  String get dlStatusDownloading => 'Téléchargement';

  @override
  String get dlStatusPaused => 'En pause';

  @override
  String get dlStatusVerifying => 'Vérification';

  @override
  String get dlStatusCompleting => 'Finalisation';

  @override
  String get dlStatusCompleted => 'Terminé';

  @override
  String get dlStatusFailed => 'Échoué';

  @override
  String get profileLocalLab => 'Lab local (loopback uniquement)';

  @override
  String get profilePublicEd2k => 'eD2K public (preview)';

  @override
  String get profilePublicEd2kKad => 'eD2K + Kad publics (preview)';

  @override
  String get subsystemDisabled => 'Désactivé (kill switch)';

  @override
  String get subsystemReady => 'Prêt (preview)';

  @override
  String get subsystemShuttingDown => 'Arrêt en cours';

  @override
  String get daemonIdle => 'Inactif';

  @override
  String get daemonDiscovering => 'Recherche du daemon...';

  @override
  String get daemonStarting => 'Démarrage du daemon...';

  @override
  String get daemonProtocolMismatch => 'Version de protocole incompatible';

  @override
  String get daemonAuthError => 'Échec d\'authentification';

  @override
  String get navShares => 'Partages';

  @override
  String get badgeEd2kHighId => 'eD2K · HighID';

  @override
  String get badgeEd2kLowId => 'eD2K · LowID';

  @override
  String get badgeEd2kConnecting => 'eD2K · Connexion…';

  @override
  String get badgeEd2kDisconnecting => 'eD2K · Déconnexion…';

  @override
  String get badgeEd2kDisconnected => 'eD2K · Déconnecté';

  @override
  String get badgeKadStopped => 'Kad · Arrêté';

  @override
  String get badgeKadConnected => 'Kad · Connecté';

  @override
  String get badgeKadBootstrapping => 'Kad · Amorçage';

  @override
  String get daemonCardTooltipTitle => 'État du daemon';

  @override
  String daemonCardTooltipServer(Object name) {
    return 'Serveur : $name';
  }

  @override
  String get daemonCardTooltipDiagnostics =>
      'Cliquer pour voir les diagnostics';

  @override
  String get sharesTitle => 'Fichiers partagés';

  @override
  String sharesFileCount(Object count) {
    return '$count fichier(s)';
  }

  @override
  String get sharesRescanTooltip => 'Rescanner le dossier Incoming';

  @override
  String get sharesEmptyState =>
      'Aucun fichier partagé.\nLes fichiers du dossier Incoming et les téléchargements en cours ou terminés sont partagés automatiquement.';

  @override
  String sharesPartialParts(Object count) {
    return ' — partiel ($count part(s))';
  }

  @override
  String get sharesTotalSentTooltip => 'Total envoyé pour ce fichier';

  @override
  String get sharePrioVeryLow => 'Très basse';

  @override
  String get sharePrioVeryHigh => 'Très haute';

  @override
  String get sharesEnabledTooltip => 'Partagé';

  @override
  String get sharesDisabledTooltip => 'Suspendu';

  @override
  String get sharesRenameDisabledTooltip =>
      'Téléchargement en cours — renommez-le depuis l\'onglet Téléchargements';

  @override
  String get sharesRenameTooltip => 'Renommer le fichier';

  @override
  String get sharesDeleteDisabledTooltip =>
      'Téléchargement en cours — supprimez-le depuis l\'onglet Téléchargements';

  @override
  String get sharesDeleteTooltip => 'Supprimer le fichier du disque';

  @override
  String get sharesDeleteTitle => 'Supprimer le fichier ?';

  @override
  String sharesDeleteBody(Object name) {
    return '$name sera définitivement supprimé du disque et retiré des partages.';
  }

  @override
  String get sourcesMaxDialogTitle => 'Sources max (FT_MAXSOURCES)';

  @override
  String get sourcesMaxHint => 'Vide = limite globale';

  @override
  String get sourcesMaxHelper =>
      'Nombre max de sources pour ce fichier (1-10000)';

  @override
  String get apply => 'Appliquer';

  @override
  String get srcNoHash => 'Pas de hash eD2K pour ce fichier.';

  @override
  String get srcLoading => 'Chargement des sources...';

  @override
  String get srcNone => 'Aucune source enregistrée pour ce fichier.';

  @override
  String srcConnected(Object count) {
    return 'Connectés ($count)';
  }

  @override
  String srcKnown(Object count) {
    return 'Sources connues ($count)';
  }

  @override
  String srcCount(Object count) {
    return 'Sources ($count)';
  }

  @override
  String srcPriorityLabel(Object priority) {
    return 'Priorité : $priority';
  }

  @override
  String get srcMaxTooltip => 'Sources max pour ce fichier (FT_MAXSOURCES)';

  @override
  String srcMaxShort(Object value) {
    return 'Src max : $value';
  }

  @override
  String get peerRetry => 'Réessayer';

  @override
  String get peerConnect => 'Connecter';

  @override
  String get pendingDestination => 'Destination en attente...';

  @override
  String get pendingCalculation => 'Calcul en cours...';

  @override
  String get tooltipDownloadPriority => 'Priorité du téléchargement';

  @override
  String get tooltipAssignCategory => 'Assigner une catégorie';

  @override
  String tooltipCategoryName(Object name) {
    return 'Catégorie : $name';
  }

  @override
  String get peerSrcAllowed => 'Autorisé';

  @override
  String get peerSrcFiltered => 'Filtré IP';

  @override
  String get peerSrcRejected => 'Rejeté profil';

  @override
  String get peerSrcTried => 'Essayé';

  @override
  String get peerSrcFailed => 'Échec';

  @override
  String get peerSrcCandidate => 'Candidat';

  @override
  String get importPartOnDisk => 'Fichier .part sur le disque :';

  @override
  String importPartPresent(Object size) {
    return 'Présent ($size)';
  }

  @override
  String get importPartMissing => 'MANQUANT (fichier .part introuvable)';

  @override
  String get openFileExplorerTooltip =>
      'Ouvrir le fichier terminé dans l\'explorateur';

  @override
  String get openFileDisabledTooltip =>
      'Disponible une fois le téléchargement et la vérification terminés';

  @override
  String addLinkDuplicate(Object name) {
    return 'Ce fichier est déjà dans la liste des téléchargements (\"$name\")';
  }

  @override
  String get pickDownloadDir => 'Sélectionner le dossier de téléchargement';

  @override
  String get addLinkGenericError => 'Erreur lors de l\'ajout du téléchargement';

  @override
  String get catPickDir =>
      'Sélectionner le dossier de téléchargement par défaut de la catégorie';

  @override
  String get catNameRequired => 'Le nom de la catégorie ne peut pas être vide';

  @override
  String get catEditTitle => 'Modifier la catégorie';

  @override
  String get catNewTitle => 'Nouvelle catégorie';

  @override
  String get catNameLabel => 'Nom de la catégorie *';

  @override
  String get catColorLabel => 'Étiquette de couleur';

  @override
  String get catDestLabel => 'Destination par défaut (optionnel)';

  @override
  String get catPrioLabel => 'Priorité par défaut (optionnel)';

  @override
  String get catSaveChanges => 'Enregistrer les modifications';

  @override
  String get catCreate => 'Créer la catégorie';

  @override
  String get opFailed => 'L\'opération a échoué';

  @override
  String get uploadsScore => 'Score';

  @override
  String get uploadsCredit => 'Crédit';

  @override
  String get uploadsBanned => 'Client banni (score 0)';

  @override
  String get kadVerified => 'Vérifié';

  @override
  String get kadNotVerified => 'Non vérifié';

  @override
  String get kadBootstrapDescPublic =>
      'Entrez l\'adresse IP publique et le port UDP d\'un nœud Kad pour amorcer la table de routage (ex : 212.83.184.225).';

  @override
  String get kadBootstrapDescLab =>
      'Entrez l\'adresse IP et le port UDP d\'un nœud d\'amorçage (en profil LocalLab, seul 127.0.0.1 est permis).';

  @override
  String get kadStartUdpPublic => 'Démarrer UDP Kad';

  @override
  String get kadStartUdpLab => 'Démarrer UDP Loopback';

  @override
  String kadUdpActive(Object addr) {
    return 'Actif ($addr)';
  }

  @override
  String get kadUdpInactive => 'Inactif';

  @override
  String get kadTimeNever => 'Jamais';

  @override
  String kadTimeMinutesAgo(Object min) {
    return 'Il y a ${min}m';
  }

  @override
  String kadTimeHoursAgo(Object hours) {
    return 'Il y a ${hours}h';
  }

  @override
  String kadTimeDaysAgo(Object days) {
    return 'Il y a ${days}j';
  }

  @override
  String kadBannerPublic(Object addr) {
    return 'Réseau Kad DHT mondial actif sur $addr | Routage Kademlia ouvert | Protection anti-SSRF active.';
  }

  @override
  String kadBannerLab(Object addr) {
    return 'Socket UDP actif : $addr (mode laboratoire confiné) | Kill switch instantané.';
  }

  @override
  String get kadBannerStartPublic =>
      'Transport UDP Kad : cliquez sur « Démarrer UDP » pour vous connecter au réseau Kademlia mondial.';

  @override
  String get kadBannerStartLab =>
      'Transport UDP Kad confiné : seul le trafic loopback (127.0.0.1) est autorisé dans le profil LocalLab.';

  @override
  String get searchExplainKad =>
      'Les requêtes KADEMLIA2_SEARCH_KEYWORD interrogent les nœuds Kad stockant l\'index du mot-clé MD4.';

  @override
  String get searchExplainGlobal =>
      'Les requêtes OP_GLOBSEARCHREQ3/REQ2 sont envoyées en UDP à tous les serveurs connus (port TCP+4). REQ3 est utilisé quand le serveur annonce EXT_GETFILES + LARGEFILES.';

  @override
  String get searchExplainServer =>
      'Les requêtes OP_SEARCHREQUEST interrogent directement l\'index du serveur eD2K connecté.';

  @override
  String searchNoResults(Object term) {
    return 'Aucun résultat trouvé pour « $term »';
  }

  @override
  String get searchPromptKeywords =>
      'Entrez des mots-clés ci-dessus pour lancer une recherche';

  @override
  String searchEnableTarget(Object target) {
    return 'Activez $target pour commencer à chercher des fichiers';
  }

  @override
  String get searchTryOther =>
      'Essayez avec d\'autres termes ou élargissez vos filtres de taille';

  @override
  String get searchHideFilters => 'Masquer les filtres';

  @override
  String get searchShowFilters => 'Afficher les filtres avancés';

  @override
  String get searchHintKad => 'Mots-clés Kad (min. 3 lettres)...';

  @override
  String get searchHintServer => 'Nom de fichier ou mots-clés...';

  @override
  String get searchButtonSearching => 'Recherche...';

  @override
  String get searchButton => 'Rechercher';

  @override
  String get searchTargetServer => 'Serveur eD2K';

  @override
  String get searchTargetKad => 'Réseau Kad DHT';

  @override
  String get searchTargetLabelKad => 'le réseau Kad DHT';

  @override
  String get searchTargetLabelGlobal => 'les serveurs eD2K (UDP)';

  @override
  String get searchTargetLabelServer => 'le serveur eD2K';

  @override
  String get searchTargetEnableGlobal => 'la liste de serveurs eD2K';

  @override
  String get searchTargetEnableServer => 'un serveur eD2K';

  @override
  String kadActiveBanner(
    Object addr,
    Object buckets,
    Object contacts,
    Object profile,
  ) {
    return 'Kad DHT actif • Contacts : $contacts ($buckets k-buckets) • Bind : $addr • Profil : $profile';
  }

  @override
  String get unknown => 'Inconnu';

  @override
  String get kadInactiveBanner =>
      'Le réseau Kad DHT n\'est pas encore actif. Démarrez le socket Kad ou effectuez un bootstrap.';

  @override
  String get searchGlobalBanner =>
      'Recherche globale UDP — interroge tous les serveurs connus (REQ3 si le serveur supporte EXT_GETFILES + LARGEFILES).';

  @override
  String searchConnectedBanner(
    Object addr,
    Object id,
    Object idType,
    Object name,
  ) {
    return 'Connecté à $name ($addr) • Client ID : $id ($idType)';
  }

  @override
  String get serverUnnamed => 'Sans nom';

  @override
  String get serverDisconnectBtn => 'Déconnecter';

  @override
  String get serverBannerConnected =>
      'Connexions eD2K disponibles. Vérifiez votre profil réseau (LocalLab vs PublicEd2kPreview) avant de vous connecter. Les adresses dans votre filtre IP sont automatiquement signalées.';

  @override
  String get serverBannerDisconnected =>
      'Avertissement réseau : importer ou gérer une liste de serveurs n\'établit aucune connexion réseau sortante. Les adresses figurant dans votre filtre IP actif sont automatiquement signalées.';

  @override
  String get serverBlockedByFilter => 'Bloqué par le filtre IP';

  @override
  String get serverDisabledBadge => 'Désactivé';

  @override
  String get historyIntegrityErrors => 'Erreurs d\'intégrité détectées';

  @override
  String get labStep2Desc =>
      'Utilise le hash pour créer une tâche de téléchargement suivie dans le client.';

  @override
  String get labAddedToDownloads => 'Ajouté aux téléchargements !';

  @override
  String get labAddToDownloads => 'Ajouter aux téléchargements';

  @override
  String get labStep4Desc =>
      'Ordonne au client de se connecter à la source 127.0.0.1 pour transférer les blocs.';

  @override
  String labDlDone(Object size) {
    return 'Téléchargement terminé et vérifié ($size)';
  }

  @override
  String labDlTransferring(Object received, Object speed, Object total) {
    return 'Transfert : $received / $total ($speed)';
  }

  @override
  String labDlReady(Object state) {
    return 'État : $state — prêt à se connecter au pair loopback.';
  }

  @override
  String get labAlreadyCompleted => 'Déjà terminé';

  @override
  String get labConnectToSource => 'Connecter à la source';

  @override
  String get labStep1Desc =>
      'Crée un fichier déterministe dans <workdir>/fixtures/ et calcule son hash eD2K.';

  @override
  String get labGenerating => 'Génération...';

  @override
  String get labGenerateFixture => 'Générer la fixture';

  @override
  String get labStep3Desc =>
      'Lance un pair eD2K local hébergeant les fixtures de test.';

  @override
  String get labStopPeer => 'Arrêter le pair loopback';

  @override
  String get labStartPeer => 'Démarrer le pair loopback';

  @override
  String labPeerActive(Object addr, Object count) {
    return 'Actif sur $addr (sert $count fichier(s))';
  }

  @override
  String get labPeerOffline => 'Pair hors ligne';

  @override
  String get labStep5Desc =>
      'Purge les fichiers générés dans <workdir>/fixtures et les partiels dans <workdir>/temp. Les téléchargements terminés dans downloads/ sont préservés.';

  @override
  String get labCleanupSafe => 'Sans risque en mode portable.';

  @override
  String get labCleanupRestricted =>
      'Le nettoyage est restreint en mode installé pour protéger les fichiers utilisateur.';

  @override
  String labErrCreateFixture(Object error) {
    return 'Échec de la création de la fixture : $error';
  }

  @override
  String labErrAddDownload(Object error) {
    return 'Échec de l\'ajout du téléchargement : $error';
  }

  @override
  String labErrPeerOp(Object error) {
    return 'Échec de l\'opération sur le pair : $error';
  }

  @override
  String labErrConnectDownload(Object error) {
    return 'Échec de la connexion du téléchargement : $error';
  }

  @override
  String labErrCleanup(Object error) {
    return 'Échec du nettoyage : $error';
  }

  @override
  String get notAvailable => 'Non disponible';

  @override
  String get copyFullPath => 'Copier le chemin complet';

  @override
  String get copiedPathInternal => 'Chemin des données internes copié';

  @override
  String get copyDownloadsPath =>
      'Copier le chemin du dossier de téléchargements';

  @override
  String get copiedDownloadsPath => 'Chemin des téléchargements copié';

  @override
  String get copyDaemonLogPath => 'Copier le chemin du journal du daemon';

  @override
  String get copiedDaemonLogPath => 'Chemin du journal du daemon copié';

  @override
  String diagNetworkProfile(Object profile) {
    return 'Profil réseau : $profile';
  }

  @override
  String msgDownloadAdded(Object name) {
    return 'Fichier \"$name\" ajouté aux téléchargements';
  }

  @override
  String msgDownloadDuplicate(Object name, Object status) {
    return 'Ce fichier est déjà dans la liste des téléchargements (\"$name\", statut : $status)';
  }

  @override
  String msgImportedFile(Object name, Object parts, Object verified) {
    return '\"$name\" importé ($verified/$parts parts vérifiées)';
  }

  @override
  String msgErrImportPartMet(Object error) {
    return 'Échec de l\'import .part.met : $error';
  }

  @override
  String msgErrPause(Object error) {
    return 'Échec de la pause : $error';
  }

  @override
  String msgErrResume(Object error) {
    return 'Échec de la reprise : $error';
  }

  @override
  String msgErrRename(Object error) {
    return 'Échec du renommage : $error';
  }

  @override
  String msgErrRemove(Object error) {
    return 'Échec de la suppression : $error';
  }

  @override
  String msgErrFetchDetails(Object error) {
    return 'Échec de la récupération des détails : $error';
  }

  @override
  String msgDestinationChanged(Object dir) {
    return 'Destination changée : $dir';
  }

  @override
  String msgErrChangeDestination(Object error) {
    return 'Échec du changement de destination : $error';
  }

  @override
  String msgRetryLoopback(Object name) {
    return 'Nouvelle tentative loopback pour \"$name\"';
  }

  @override
  String msgErrRetryConn(Object error) {
    return 'Échec de la nouvelle tentative : $error';
  }

  @override
  String msgErrOpenFile(Object error) {
    return 'Impossible d\'ouvrir le fichier terminé : $error';
  }

  @override
  String msgDownloadCompleted(Object name) {
    return 'Téléchargement terminé : $name';
  }

  @override
  String msgDownloadFailed(Object name) {
    return 'Échec du téléchargement : $name';
  }

  @override
  String msgCategoryCreated(Object name) {
    return 'Catégorie \"$name\" créée';
  }

  @override
  String msgErrCategoryCreate(Object error) {
    return 'Échec de la création de la catégorie : $error';
  }

  @override
  String msgCategoryUpdated(Object name) {
    return 'Catégorie \"$name\" mise à jour';
  }

  @override
  String msgErrCategoryUpdate(Object error) {
    return 'Échec de la mise à jour de la catégorie : $error';
  }

  @override
  String msgCategoryDeleted(Object name) {
    return 'Catégorie \"$name\" supprimée';
  }

  @override
  String msgErrCategoryDelete(Object error) {
    return 'Échec de la suppression de la catégorie : $error';
  }

  @override
  String msgErrCategoryReorder(Object error) {
    return 'Échec du réordonnancement des catégories : $error';
  }

  @override
  String msgAssignedToCategory(Object name) {
    return 'Assigné à la catégorie \"$name\"';
  }

  @override
  String get categoryFallback => 'Catégorie';

  @override
  String msgProfileSwitched(Object profile) {
    return 'Profil réseau basculé : $profile';
  }

  @override
  String msgErrProfileSwitch(Object error) {
    return 'Échec du changement de profil réseau : $error';
  }

  @override
  String get msgKillSwitchOn =>
      'Kill switch activé : toutes les connexions réseau ont été coupées';

  @override
  String msgErrKillSwitch(Object error) {
    return 'Échec du kill switch : $error';
  }

  @override
  String get msgNetworkReactivated => 'Réseau réactivé';

  @override
  String msgErrNetworkReactivate(Object error) {
    return 'Échec de la réactivation du réseau : $error';
  }

  @override
  String get msgBudgetsUpdated => 'Budgets de ressources mis à jour';

  @override
  String msgErrBudgets(Object error) {
    return 'Échec de la mise à jour des budgets de ressources : $error';
  }

  @override
  String msgIpFilterImported(Object count) {
    return 'Filtre IP importé : $count plages';
  }

  @override
  String msgErrIpFilterImport(Object error) {
    return 'Échec de l\'import du filtre IP : $error';
  }

  @override
  String get msgIpFilterEnabled => 'Filtre IP activé';

  @override
  String get msgIpFilterDisabled => 'Filtre IP désactivé';

  @override
  String msgErrIpFilterToggle(Object error) {
    return 'Échec du basculement du filtre IP : $error';
  }

  @override
  String get msgIpFilterCleared => 'Filtre IP vidé';

  @override
  String msgErrIpFilterClear(Object error) {
    return 'Échec de la purge du filtre IP : $error';
  }

  @override
  String get msgRateLimitsUpdated => 'Limites de débit mises à jour';

  @override
  String msgErrRateLimits(Object error) {
    return 'Échec de la mise à jour des limites de débit : $error';
  }

  @override
  String msgErrLoadSources(Object error) {
    return 'Échec du chargement des sources : $error';
  }

  @override
  String msgSourceAdded(Object address) {
    return 'Source $address ajoutée';
  }

  @override
  String msgErrAddSource(Object error) {
    return 'Échec de l\'ajout de la source : $error';
  }

  @override
  String msgConnInitiated(Object address) {
    return 'Connexion à $address initialisée';
  }

  @override
  String msgErrConnectSource(Object error) {
    return 'Échec de la connexion à la source : $error';
  }

  @override
  String msgPeerDisconnected(Object address) {
    return 'Peer $address déconnecté';
  }

  @override
  String get msgSessionsDisconnected => 'Session(s) peer déconnectée(s)';

  @override
  String msgErrDisconnectPeer(Object error) {
    return 'Échec de la déconnexion peer : $error';
  }

  @override
  String msgPriorityChanged(Object priority) {
    return 'Priorité changée : $priority';
  }

  @override
  String msgErrPriority(Object error) {
    return 'Échec du changement de priorité : $error';
  }

  @override
  String get msgSrcLimitReset => 'Limite de sources réinitialisée';

  @override
  String msgSrcLimitSet(Object max) {
    return 'Limite de sources : $max';
  }

  @override
  String msgErrSrcLimit(Object error) {
    return 'Échec du changement de limite : $error';
  }

  @override
  String get msgSourcesCleared => 'Sources effacées';

  @override
  String msgErrClearSources(Object error) {
    return 'Échec de l\'effacement des sources : $error';
  }

  @override
  String msgErrKadContacts(Object error) {
    return 'Échec de la récupération des contacts Kad : $error';
  }

  @override
  String msgErrNodesDatInspect(Object error) {
    return 'Échec de l\'inspection de nodes.dat : $error';
  }

  @override
  String msgKadContactsImported(Object count) {
    return '$count contacts Kad importés dans la table K-buckets';
  }

  @override
  String get msgErrNodesDatImport => 'Échec de l\'importation de nodes.dat';

  @override
  String msgKadContactsCleared(Object count) {
    return '$count contacts Kad effacés';
  }

  @override
  String msgErrKadClear(Object error) {
    return 'Échec de l\'effacement des contacts Kad : $error';
  }

  @override
  String msgKadUdpStarted(Object addr) {
    return 'Socket UDP Kad démarré sur $addr';
  }

  @override
  String get msgErrKadUdpStart => 'Échec du démarrage UDP Kad';

  @override
  String get msgKadUdpStopped => 'Socket UDP Kad arrêté';

  @override
  String get msgErrKadUdpStop => 'Échec de l\'arrêt UDP Kad';

  @override
  String msgKadPong(Object addr, Object latency) {
    return 'Pong reçu de $addr en $latency ms';
  }

  @override
  String msgErrKadPing(Object addr, Object error) {
    return 'Échec du ping Kad ($addr) : $error';
  }

  @override
  String msgKadBootstrapOk(Object count) {
    return 'Amorçage Kad réussi ($count contacts reçus)';
  }

  @override
  String get msgErrKadBootstrap => 'Échec de l\'amorçage Kad';

  @override
  String msgKadBootstrapUrlOk(Object count) {
    return 'Amorçage Kad depuis URL réussi ($count nœuds importés)';
  }

  @override
  String get msgErrKadBootstrapUrl => 'Échec de l\'amorçage Kad depuis URL';

  @override
  String msgKadSearchFound(Object count) {
    return 'Recherche Kad : $count nœuds les plus proches trouvés';
  }

  @override
  String get msgErrKadSearch => 'Échec de la recherche Kad';

  @override
  String msgKadSourcesFound(Object count, Object hash) {
    return 'Kad : $count sources trouvées pour $hash…';
  }

  @override
  String get msgErrKadSources => 'Échec de la recherche de sources Kad';

  @override
  String msgKadPublished(Object count, Object hash) {
    return 'Kad : publié sur $count nœuds pour $hash…';
  }

  @override
  String get msgErrKadPublish => 'Échec de la publication Kad';

  @override
  String msgSearchGlobalSent(Object term, Object udp) {
    return 'Recherche globale \"$term\" envoyée$udp';
  }

  @override
  String msgSearchSent(Object term, Object udp) {
    return 'Recherche \"$term\" envoyée au serveur$udp';
  }

  @override
  String msgSearchNoResponse(Object term) {
    return 'Recherche \"$term\" terminée sans réponse du serveur';
  }

  @override
  String msgErrSearch(Object error) {
    return 'Échec de la recherche : $error';
  }

  @override
  String msgKadSearchDone(Object count, Object term) {
    return 'Recherche Kad \"$term\" terminée ($count résultat(s))';
  }

  @override
  String msgErrKadSearchRun(Object error) {
    return 'Échec de la recherche Kad : $error';
  }

  @override
  String msgSourceReqSent(Object hash) {
    return 'Demande de sources envoyée pour $hash';
  }

  @override
  String msgErrSourceReq(Object error) {
    return 'Échec de la demande de sources : $error';
  }

  @override
  String msgErrListServers(Object error) {
    return 'Échec de la liste des serveurs : $error';
  }

  @override
  String msgErrInspectServerMet(Object error) {
    return 'Échec de l\'inspection du server.met : $error';
  }

  @override
  String msgServersImported(Object allowed, Object blocked, Object imported) {
    return '$imported serveurs importés ($allowed autorisés, $blocked bloqués par le filtre IP)';
  }

  @override
  String msgErrImportServerMet(Object error) {
    return 'Échec de l\'import du server.met : $error';
  }

  @override
  String msgServersUpdated(Object count) {
    return 'Mise à jour réussie : $count serveurs traités depuis l\'URL';
  }

  @override
  String msgErrServersUpdate(Object error) {
    return 'Échec de la mise à jour des serveurs depuis l\'URL : $error';
  }

  @override
  String msgErrToggleServer(Object error) {
    return 'Échec du basculement du serveur : $error';
  }

  @override
  String msgErrDeleteServer(Object error) {
    return 'Échec de la suppression du serveur : $error';
  }

  @override
  String msgServersCleared(Object count) {
    return '$count serveurs effacés';
  }

  @override
  String msgErrClearServers(Object error) {
    return 'Échec de l\'effacement des serveurs : $error';
  }

  @override
  String msgConnectingToServer(Object name) {
    return 'Connexion au serveur $name...';
  }

  @override
  String msgErrConnectServer(Object error) {
    return 'Connexion impossible : $error';
  }

  @override
  String get msgDisconnectedFromServer => 'Déconnecté du serveur eD2K';

  @override
  String msgErrDisconnectServer(Object error) {
    return 'Erreur de déconnexion : $error';
  }

  @override
  String get msgSettingsUpdated => 'Paramètres mis à jour';

  @override
  String msgErrSettingsUpdate(Object error) {
    return 'Échec de la mise à jour des paramètres : $error';
  }

  @override
  String get msgDownloadDirUpdated =>
      'Dossier de téléchargement par défaut mis à jour';

  @override
  String msgErrDownloadDir(Object error) {
    return 'Échec de la définition du dossier de téléchargement : $error';
  }

  @override
  String get msgDownloadDirReset =>
      'Dossier de téléchargement réinitialisé (défaut portable)';

  @override
  String msgErrDownloadDirReset(Object error) {
    return 'Échec de la réinitialisation du dossier de téléchargement : $error';
  }

  @override
  String msgErrFetchHistory(Object error) {
    return 'Échec du chargement de l\'historique : $error';
  }

  @override
  String msgHistoryCleared(Object count) {
    return '$count enregistrement(s) d\'historique effacé(s)';
  }

  @override
  String msgErrClearHistory(Object error) {
    return 'Échec de l\'effacement de l\'historique : $error';
  }

  @override
  String msgErrExportHistory(Object error) {
    return 'Échec de l\'export de l\'historique : $error';
  }

  @override
  String get msgRetentionUnlimited => 'Rétention de l\'historique illimitée';

  @override
  String msgRetentionDays(Object days) {
    return 'Rétention de l\'historique : $days jours';
  }

  @override
  String msgErrRetention(Object error) {
    return 'Échec de la mise à jour de la rétention : $error';
  }

  @override
  String msgBytesInjected(Object count, Object offset) {
    return '$count octets injectés à l\'offset $offset';
  }

  @override
  String msgErrInject(Object error) {
    return 'Échec de l\'injection : $error';
  }

  @override
  String msgFixtureCreated(Object name) {
    return 'Fixture $name créée';
  }

  @override
  String msgPeerStarted(Object addr) {
    return 'Pair loopback démarré sur $addr';
  }

  @override
  String get msgPeerStopped => 'Pair de test loopback arrêté';

  @override
  String msgConnectedToPeer(Object addr) {
    return 'Téléchargement connecté à $addr';
  }

  @override
  String msgCatSetError(Object e) {
    return 'Échec de l\'assignation de la catégorie : $e';
  }

  @override
  String msgSourcesFound(Object hash, Object n) {
    return '$n source(s) trouvée(s) pour $hash';
  }

  @override
  String get kadTimeJustNow => 'À l\'instant';

  @override
  String labCreateFixtureError(Object e) {
    return 'Échec de la création de la fixture : $e';
  }

  @override
  String labAddDownloadError(Object e) {
    return 'Échec de l\'ajout du téléchargement : $e';
  }

  @override
  String labPeerError(Object e) {
    return 'Échec de l\'opération sur le pair : $e';
  }

  @override
  String labConnectError(Object e) {
    return 'Échec de la connexion du téléchargement : $e';
  }

  @override
  String labCleanupError(Object e) {
    return 'Échec du nettoyage : $e';
  }

  @override
  String labDownloadComplete(Object size) {
    return 'Téléchargement terminé et vérifié ($size)';
  }

  @override
  String labTransferring(Object rate, Object recv, Object total) {
    return 'Transfert : $recv / $total ($rate)';
  }

  @override
  String labReadyToConnect(Object state) {
    return 'État : $state — prêt à se connecter au pair loopback.';
  }

  @override
  String get labCleanupPortableOk => 'Sûr à exécuter en mode portable.';

  @override
  String get labCleanupPortableNo =>
      'Le nettoyage est restreint en mode installé pour protéger les fichiers utilisateur.';

  @override
  String histFilesCount(Object n) {
    return '$n fichiers';
  }

  @override
  String histSessionCount(Object n, Object size) {
    return '$n cette session ($size)';
  }

  @override
  String histResumesCount(Object n) {
    return '$n reprises';
  }

  @override
  String histHashMismatchCount(Object n) {
    return '$n erreur(s) de hash';
  }

  @override
  String get histHashVerified => '100 % hash vérifié';

  @override
  String get histIntegrityErrors => 'Erreurs d\'intégrité détectées';

  @override
  String peerReceivedBytes(Object bytes) {
    return 'Reçus : $bytes';
  }

  @override
  String searchUdpFailed(Object n) {
    return ', $n échec(s)';
  }

  @override
  String get kadIdGenerated => '(Généré à l\'initialisation)';

  @override
  String get ed2kErrPrefix => 'Le lien doit commencer par \"ed2k://|file|\"';

  @override
  String get ed2kErrIncomplete => 'Format de lien eD2K incomplet';

  @override
  String get ed2kErrInvalid =>
      'Nom de fichier, taille ou hash hexadécimal de 32 caractères invalide';

  @override
  String partsCount(Object n) {
    return '$n partie(s)';
  }

  @override
  String sourcesLoopback(Object n) {
    return '$n loopback';
  }

  @override
  String get diagLoopbackTitle => 'Profil réseau : LocalLab (loopback)';

  @override
  String get diagLoopbackSubtitle =>
      'Trafic confiné à 127.0.0.1 — aucune donnée ne quitte la machine.';

  @override
  String get diagPublicSubtitle =>
      'Connexions WAN autorisées — protection anti-SSRF active.';
}
