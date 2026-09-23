// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:app_shell/app/providers.dart';
import 'package:app_shell/ui/features/downloads/widgets/download_detail_dialog.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';

class PeerSourcesMockClient implements MuleDaemonClient {
  final List<DownloadItem> _items;
  final List<PeerSourceModel> _sources = [
    PeerSourceModel(
      fileHash: '0123456789abcdef0123456789abcdef',
      address: '127.0.0.1:39735',
      ip: 2130706433,
      port: 39735,
      status: PeerSourceStatus.allowed,
      origin: PeerSourceOrigin.server,
      failCount: 0,
      discoveredAtUnixMs: 1700000000000,
      lastSeenUnixMs: 1700000000000,
    ),
    PeerSourceModel(
      fileHash: '0123456789abcdef0123456789abcdef',
      address: '198.51.100.22:39735',
      ip: 3325256726,
      port: 39735,
      status: PeerSourceStatus.filtered,
      origin: PeerSourceOrigin.search,
      failCount: 1,
      lastError: 'IP banned by filter',
      discoveredAtUnixMs: 1700000000000,
      lastSeenUnixMs: 1700000000000,
    ),
    // Deadlist eMule : échec de connexion — persisté pour le backoff,
    // masqué dans la liste des sources.
    PeerSourceModel(
      fileHash: '0123456789abcdef0123456789abcdef',
      address: '203.0.113.9:4662',
      ip: 3395909897,
      port: 4662,
      status: PeerSourceStatus.failed,
      origin: PeerSourceOrigin.server,
      failCount: 2,
      lastError: 'Connection timed out',
      cooldownUntilUnixMs: DateTime.now()
          .add(const Duration(minutes: 15))
          .millisecondsSinceEpoch,
      discoveredAtUnixMs: 1700000000000,
      lastSeenUnixMs: 1700000000000,
    ),
  ];

  final StreamController<PeerSessionModel> _sessionEventsController =
      StreamController<PeerSessionModel>.broadcast();
  PeerSessionModel _currentSession = PeerSessionModel.disconnected;

  PeerSourcesMockClient(this._items);

  @override
  DaemonInfo? get daemonInfo => null;

  @override
  DaemonConnectionState get currentConnectionState =>
      DaemonConnectionState.connected;

  @override
  Stream<DaemonConnectionState> get connectionState => const Stream.empty();

  @override
  Stream<ServerSessionModel> get serverSessionEvents => const Stream.empty();

  @override
  Stream<DownloadUpdatedEvent> get downloadEvents => const Stream.empty();

  @override
  Future<DaemonInfo> connect({required Uri url, String? token}) async =>
      const DaemonInfo(
        version: '1.0.0',
        protocolVersion: 1,
        instanceId: 'test-inst',
        sessionId: 'sess',
        capabilities: ['downloads', 'peer-sources', 'peer-connection'],
      );

  @override
  Future<void> stopDaemon() async {}

  @override
  Future<void> disconnect() async {}

  @override
  Future<List<DownloadItem>> listDownloads() async => _items;

  @override
  Future<DownloadItem?> getDownload(String id) async =>
      _items.firstWhere((d) => d.id == id);

  @override
  Future<String> addEd2kLink(
    String link, {
    String? targetDirectory,
    String? allocationPolicy,
    String? categoryId,
  }) async => 'new-id';

  @override
  Future<DownloadItem> pauseDownload(String id) async =>
      _items.firstWhere((d) => d.id == id);

  @override
  Future<DownloadItem> resumeDownload(String id) async =>
      _items.firstWhere((d) => d.id == id);

  @override
  Future<void> removeDownload(String id, {bool deleteFiles = false}) async {}

  @override
  Future<DownloadItem> injectRange(
    String id,
    int offset,
    String hexData,
  ) async => _items.firstWhere((d) => d.id == id);

  @override
  Future<LabStatus> getLabStatus() async => const LabStatus(
    workdirMode: 'Portable',
    workdirPath: 'test_path',
    isPortable: true,
    fixtures: [],
  );

  @override
  Future<LabFixtureInfo> createLabFixture(String fixtureType) async =>
      const LabFixtureInfo(
        fileName: 'test.bin',
        filePath: 'test.bin',
        sizeBytes: 1024,
        ed2kHash: '0123456789abcdef0123456789abcdef',
        ed2kLink:
            'ed2k://|file|test.bin|1024|0123456789abcdef0123456789abcdef|/',
      );

  @override
  Future<LabPeerInfo> startLabPeer() async => const LabPeerInfo(
    peerAddr: '127.0.0.1:39735',
    sharedCount: 1,
    status: 'running',
  );

  @override
  Future<void> stopLabPeer() async {}

  @override
  Future<LabCleanupReport> cleanupLabTestData() async => const LabCleanupReport(
    success: true,
    cleanedFolders: ['fixtures'],
    deletedFilesCount: 0,
    freedBytes: 0,
    message: 'Mock cleanup',
  );

  @override
  Future<void> connectDownloadToSource(
    String downloadId,
    String peerAddr,
  ) async {}

  @override
  Future<DownloadItem> setDownloadDestination(
    String id,
    String targetDirectory,
  ) async => _items.firstWhere((d) => d.id == id);

  @override
  Future<DownloadItem> retryDownloadConnection(String id) async =>
      _items.firstWhere((d) => d.id == id);

  @override
  Future<Map<String, dynamic>> getSettings() async => {
    'defaultDownloadDir': 'downloads',
    'portableDownloadDir': 'downloads',
    'isCustom': false,
  };

  @override
  Future<void> setDefaultDownloadDirectory(String path) async {}

  @override
  Future<void> updateSettings(Map<String, dynamic> patch) async {}

  @override
  Future<void> resetDefaultDownloadDirectory() async {}

  @override
  Future<List<HistoryEntry>> listHistory({
    String? eventType,
    String? status,
    int? fromTimestampMs,
    int? toTimestampMs,
    int? limit,
    int? offset,
  }) async => [];

  @override
  Future<HistoryStats> getHistoryStats() async => const HistoryStats(
    sessionCompletedCount: 0,
    sessionCompletedBytes: 0,
    last24hCompletedCount: 0,
    last24hCompletedBytes: 0,
    last7dCompletedCount: 0,
    last7dCompletedBytes: 0,
    totalCompletedCount: 0,
    totalCompletedBytes: 0,
    totalFailedCount: 0,
    totalResumesCount: 0,
    totalIntegrityFailures: 0,
  );

  @override
  Future<int> clearHistory() async => 0;

  @override
  Future<String> exportHistory(
    String format, {
    String? eventType,
    String? status,
  }) async => '';

  @override
  Future<void> setHistoryRetentionDays(int days) async {}

  @override
  Future<List<CategoryItem>> listCategories() async => [];

  @override
  Future<CategoryItem> createCategory({
    required String name,
    required String color,
    String? defaultDestination,
    String? defaultPriority,
  }) async => CategoryItem(
    id: 'mock-cat',
    name: name,
    color: color,
    createdAt: DateTime.now(),
    updatedAt: DateTime.now(),
  );

  @override
  Future<CategoryItem> updateCategory(
    String id, {
    String? name,
    String? color,
    String? defaultDestination,
    String? defaultPriority,
    int? sortOrder,
  }) async => CategoryItem(
    id: id,
    name: name ?? 'Updated',
    color: color ?? '#2196F3',
    createdAt: DateTime.now(),
    updatedAt: DateTime.now(),
  );

  @override
  Future<void> deleteCategory(String id) async {}

  @override
  Future<void> reorderCategories(List<String> categoryIds) async {}

  @override
  Future<DownloadItem> setDownloadCategory(
    String downloadId,
    String? categoryId,
  ) async => _items.firstWhere((d) => d.id == downloadId);

  @override
  Future<PartMetInspectionResult> inspectPartMet(
    String metPath, {
    String? partPath,
  }) async => PartMetInspectionResult(
    fileName: 'mock.bin',
    totalSize: 1000,
    ed2kHash: '00000000000000000000000000000000',
    partCount: 1,
    partMetPath: metPath,
    partFileExists: true,
    estimatedTransferredBytes: 1000,
  );

  @override
  Future<ImportPartMetResult> importPartMet(
    String metPath, {
    String? partPath,
    String? targetDirectory,
    String? categoryId,
  }) async => ImportPartMetResult(
    download: _items.first,
    totalParts: 1,
    verifiedParts: 1,
    corruptedParts: 0,
    transferredBytes: 1000,
    totalSize: 1000,
  );

  @override
  Future<NetworkPolicySnapshot> getNetworkPolicy() async =>
      const NetworkPolicySnapshot(
        profile: NetworkProfile.localLab,
        state: NetworkSubsystemState.previewActive,
        budgets: ResourceBudgetsModel(
          maxActivePeerConnections: 20,
          maxActiveServerConnections: 2,
          maxActiveListeners: 2,
          connectTimeoutMs: 10000,
          handshakeTimeoutMs: 15000,
          maxFrameSizeBytes: 10485760,
          maxTotalBufferBytes: 33554432,
          rateLimitPerMin: 60,
        ),
        activeConnections: NetworkActiveConnections(),
      );

  @override
  Future<NetworkPolicySnapshot> setNetworkProfile(
    NetworkProfile profile,
  ) async => getNetworkPolicy();

  @override
  Future<void> disconnectAll() async {}

  @override
  Future<void> enableNetwork() async {}

  @override
  Future<List<NetworkAuditEntryItem>> getNetworkAuditLog({
    int limit = 50,
  }) async => [];

  @override
  Future<NetworkPolicySnapshot> setResourceBudgets(
    ResourceBudgetsModel budgets,
  ) async => getNetworkPolicy();

  @override
  Future<FilterStatusModel> getFilterStatus() async =>
      const FilterStatusModel(enabled: true, rangeCount: 0, hitsCount: 0);

  @override
  Future<FilterStatusModel> importFilterFile(String filePath) async =>
      const FilterStatusModel(enabled: true, rangeCount: 100, hitsCount: 0);

  @override
  Future<FilterStatusModel> setFilterEnabled(bool enabled) async =>
      const FilterStatusModel(enabled: true, rangeCount: 0, hitsCount: 0);

  @override
  Future<FilterStatusModel> clearFilter() async =>
      const FilterStatusModel(enabled: true, rangeCount: 0, hitsCount: 0);

  @override
  Future<RateLimitsModel> getRateLimits() async => const RateLimitsModel(
    downloadBytesPerSec: 0,
    uploadBytesPerSec: 0,
    connectionsPerMinute: 120,
    serverRequestsPerSec: 10,
    kadPacketsPerSec: 50,
  );

  @override
  Future<RateLimitsModel> setRateLimits(RateLimitsModel limits) async => limits;

  @override
  Future<ServerInspectReport> inspectServerMet(String filePath) async =>
      ServerInspectReport(
        filePath: filePath,
        totalCount: 0,
        allowedCount: 0,
        blockedCount: 0,
        servers: const [],
      );

  @override
  Future<ServerImportResult> importServerMet(String filePath) async =>
      const ServerImportResult(
        success: true,
        imported: 0,
        totalCount: 0,
        allowedCount: 0,
        blockedCount: 0,
      );

  @override
  Future<List<Ed2kServerModel>> listServers() async => const [];

  @override
  Future<bool> setServerEnabled(String address, bool enabled) async => true;

  @override
  Future<bool> deleteServer(String address) async => true;

  @override
  Future<int> clearServers() async => 0;

  @override
  Future<ServerSessionModel> connectServer(String address) async =>
      const ServerSessionModel(status: ServerConnectionStatus.connected);

  @override
  Future<ServerSessionModel> disconnectServer() async =>
      ServerSessionModel.disconnected;

  @override
  Future<ServerSessionModel> getServerStatus() async =>
      ServerSessionModel.disconnected;

  @override
  Future<Map<String, dynamic>> searchServer({
    required String term,
    int? minSize,
    int? maxSize,
    String? extension,
    bool udpOnly = false,
  }) async =>
      const {'sent': true};

  @override
  Future<void> queryServerSources({
    required String fileHash,
    int? fileSize,
  }) async {}

  @override
  Stream<List<SearchResultFileModel>> get searchResultEvents =>
      const Stream.empty();

  @override
  Stream<Map<String, dynamic>> get foundSourcesEvents => const Stream.empty();

  @override
  Stream<UploadsSnapshotModel> get uploadsEvents => const Stream.empty();

  @override
  Stream<DaemonStatsModel> get statsEvents => const Stream.empty();

  @override
  Stream<DownloadLifecycleEvent> get downloadLifecycleEvents =>
      const Stream.empty();

  @override
  Stream<int> get kadStatusEvents => const Stream.empty();

  @override
  Stream<int> get historyEvents => const Stream.empty();

  @override
  Stream<int> get sharesEvents => const Stream.empty();

  @override
  Stream<SettingsChangedEvent> get settingsEvents => const Stream.empty();

  @override
  Stream<int> get networkPolicyEvents => const Stream.empty();

  @override
  Stream<UpnpStatusModel> get upnpStatusEvents => const Stream.empty();

  @override
  Stream<PeerSessionModel> get peerSessionEvents =>
      _sessionEventsController.stream;

  @override
  Future<List<PeerSourceModel>> listSources(String fileHash) async {
    return _sources.where((s) => s.fileHash == fileHash).toList();
  }

  @override
  Future<PeerSourceModel> addManualSource(
    String fileHash,
    String address,
  ) async {
    final s = PeerSourceModel(
      fileHash: fileHash,
      address: address,
      ip: 167772165,
      port: 39735,
      status: PeerSourceStatus.allowed,
      origin: PeerSourceOrigin.manual,
      failCount: 0,
      discoveredAtUnixMs: DateTime.now().millisecondsSinceEpoch,
      lastSeenUnixMs: DateTime.now().millisecondsSinceEpoch,
    );
    _sources.add(s);
    return s;
  }

  @override
  Future<PeerSessionModel> connectPeerSource(
    String fileHash,
    String address, {
    bool force = false,
  }) async {
    _currentSession = PeerSessionModel(
      status: PeerConnectionStatus.transferring,
      fileHash: fileHash,
      peerAddress: address,
      transferredBytes: 184320,
      transferRateBps: 64000,
    );
    _sessionEventsController.add(_currentSession);
    return _currentSession;
  }

  @override
  Future<PeerSessionModel> disconnectPeerSource({String? address}) async {
    _currentSession = PeerSessionModel.disconnected;
    _sessionEventsController.add(_currentSession);
    return _currentSession;
  }

  @override
  Future<PeerSessionModel> getPeerSessionStatus() async => _currentSession;

  @override
  Future<SwarmStatusModel> getSwarmStatus({String? fileHash}) async {
    final active = _currentSession.status != PeerConnectionStatus.disconnected
        ? [_currentSession]
        : <PeerSessionModel>[];
    return SwarmStatusModel(
      fileHash: fileHash ?? _currentSession.fileHash,
      activePeerCount: active.length,
      peers: active,
      inFlightBlocks: const [],
    );
  }

  @override
  Future<int> clearSources(String fileHash) async {
    final count = _sources.where((s) => s.fileHash == fileHash).length;
    _sources.removeWhere((s) => s.fileHash == fileHash);
    return count;
  }

  @override
  Future<List<SharedFileModel>> listSharedFiles() async => [];

  @override
  Future<SharedFileModel> addSharedFile({
    required String fileHash,
    required String fileName,
    required int fileSize,
    required String filePath,
  }) async => SharedFileModel(
    fileHash: fileHash,
    fileName: fileName,
    fileSize: fileSize,
    filePath: filePath,
    sharedAtUnixMs: DateTime.now().millisecondsSinceEpoch,
    enabled: true,
  );

  @override
  Future<void> removeSharedFile(String fileHash, {bool deleteFile = false}) async {}

  @override
  Future<DownloadItem> renameDownload(String id, String newName) async =>
      throw UnimplementedError();

  @override
  Future<void> renameSharedFile(String fileHash, String newName) async {}

  @override
  Future<void> resetLifetimeTotals() async {}

  @override
  Future<SharedFileModel> setSharedFileEnabled(
    String fileHash,
    bool enabled,
  ) async => SharedFileModel(
    fileHash: fileHash,
    fileName: 'file.bin',
    fileSize: 1000,
    filePath: 'C:\\file.bin',
    sharedAtUnixMs: 0,
    enabled: enabled,
  );

  @override
  Future<void> setSharedFileUploadPriority(
    String fileHash,
    String priority,
  ) async {}

  @override
  Future<void> setSharedFileCategory(
    String fileHash,
    String? categoryId,
  ) async {}

  @override
  Future<int> rescanSharedFiles() async => 0;

  @override
  Future<DownloadItem> setDownloadPriority(String id, String priority) =>
      throw UnimplementedError();

  @override
  Future<DownloadItem> setDownloadMaxSources(String id, int? maxSources) =>
      throw UnimplementedError();

  @override
  Future<List<FriendModel>> listFriends() async => [];

  @override
  Future<void> addFriend(String userHash, {String name = ''}) async {}

  @override
  Future<void> removeFriend(String userHash) async {}

  @override
  Future<InboundListenerStatusModel> startListener({int port = 0}) async =>
      InboundListenerStatusModel.idle;

  @override
  Future<InboundListenerStatusModel> stopListener() async =>
      InboundListenerStatusModel.idle;

  @override
  Future<InboundListenerStatusModel> getListenerStatus() async =>
      InboundListenerStatusModel.idle;

  @override
  Future<Map<String, dynamic>> listUploads() async => {
    'active': [],
    'queued': [],
  };

  @override
  Future<UploadSessionModel?> getUploadStatus() async => null;

  @override
  Future<void> cancelUpload(String clientAddress) async {}

  @override
  Future<SecureIdentityStatusModel> getIdentityStatus() async =>
      const SecureIdentityStatusModel(
        enabled: true,
        hasKey: true,
        pubkeyHash: 'mock_hash',
        keyBits: 384,
      );

  @override
  Future<SecureIdentityStatusModel> rotateIdentity() async =>
      const SecureIdentityStatusModel(
        enabled: true,
        hasKey: true,
        pubkeyHash: 'rotated_mock_hash',
        keyBits: 384,
      );

  @override
  Future<List<PeerCreditModel>> listPeerCredits({int limit = 50}) async =>
      const [];

  @override
  Future<PeerCreditStatsModel> getPeerCreditStats() async =>
      PeerCreditStatsModel.zero;

  @override
  Future<KadStatusModel> getKadStatus() async => KadStatusModel.initial;

  @override
  Future<Map<String, dynamic>> startKadUdp({String? bindAddress}) async => {
    'boundAddress': '127.0.0.1:39737',
  };

  @override
  Future<Map<String, dynamic>> stopKadUdp() async => {'stopped': true};

  @override
  Future<Map<String, dynamic>> pingKadContact({
    required String ip,
    required int port,
    String? kadId,
  }) async => {'success': true, 'latencyMs': 5};

  @override
  Future<NodesDatInspectReportModel> inspectNodesDat(String filePath) async =>
      const NodesDatInspectReportModel(
        version: 2,
        declaredCount: 0,
        validContactsCount: 0,
        rejectedByPolicyCount: 0,
        rejectedByFilterCount: 0,
        sampleContacts: [],
      );

  @override
  Future<Map<String, dynamic>> importNodesDat(String filePath) async => {
    'importedCount': 0,
  };

  @override
  Future<List<KadContactModel>> listKadContacts({int limit = 100}) async =>
      const [];

  @override
  Future<Map<String, dynamic>> bootstrapKad({
    required String ip,
    required int port,
  }) async => {
    'status': 'ok',
    'contactsDiscovered': 0,
    'bootstrapNode': '$ip:$port',
  };

  @override
  Future<List<KadContactModel>> findKadNode(String targetId) async => const [];

  @override
  Future<int> clearKadContacts() async => 0;

  @override
  Future<List<PeerSourceModel>> searchKadSources({
    required String fileHash,
    int? fileSize,
  }) async => const [];

  @override
  Future<List<SearchResultFileModel>> searchKadKeywords({
    required String term,
    int? minSize,
    int? maxSize,
  }) async => const [];

  @override
  Future<int> publishKadSource({
    required String fileHash,
    required int fileSize,
    int tcpPort = 39735,
  }) async => 0;

  @override
  Future<int> publishKadKeywords({
    required String fileHash,
    required String fileName,
    required int fileSize,
  }) async => 0;

  @override
  Future<UpnpStatusModel> getUpnpStatus() async => const UpnpStatusModel(
    enabled: true,
    available: false,
    gatewayAddr: null,
    externalIp: null,
    tcpPort: 39735,
    udpPort: 39737,
    tcpMapped: false,
    udpMapped: false,
  );

  @override
  Future<UpnpStatusModel> setUpnpEnabled(bool enabled) async => getUpnpStatus();

  @override
  Future<UpnpStatusModel> mapUpnpPorts({int? tcpPort, int? udpPort}) async =>
      getUpnpStatus();

  @override
  Future<UpnpStatusModel> unmapUpnpPorts() async => getUpnpStatus();

  @override
  Future<Map<String, dynamic>> updateServersFromUrl(String url) async => {
    'imported': 0,
    'serversCount': 0,
  };

  @override
  Future<Map<String, dynamic>> bootstrapKadFromUrl(String url) async => {
    'importedNodes': 0,
  };

  @override
  void dispose() {
    _sessionEventsController.close();
  }
}

void main() {
  testWidgets(
    'DownloadDetailDialog displays peer sources and allows connecting/disconnecting',
    (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 900);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      final download = DownloadItem(
        id: 'dl-1',
        fileName: 'ubuntu-22.04-live-server.iso',
        totalBytes: 1500000000,
        receivedBytes: 500000000,
        verifiedBytes: 500000000,
        progress: 0.33,
        status: DownloadStatus.downloading,
        priority: DownloadPriority.normal,
        sourceCount: 2,
        downloadRateBps: 250000,
        uploadedBytes: 0,
        ed2kHash: '0123456789abcdef0123456789abcdef',
        createdAt: DateTime.now(),
        updatedAt: DateTime.now(),
      );

      final mockClient = PeerSourcesMockClient([download]);
      final viewModel = AppController(client: mockClient);
      await viewModel.initialize(url: Uri.parse('ws://mock:4660/rpc'));

      await tester.pumpWidget(
        ProviderScope(
        overrides: [
          appControllerProvider.overrideWith((ref) => viewModel),
          devToolsEnabledProvider
              .overrideWithValue(viewModel.isDevToolsAvailable),
          daemonSupervisorProvider
              .overrideWithValue(viewModel.supervisor),
          muleDaemonClientProvider.overrideWithValue(viewModel.client),
        ],
        child: MaterialApp(
        locale: const Locale('fr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: DownloadDetailDialog(
              downloadId: 'dl-1',
              ),
          ),
        )),
      );
      await tester.pumpAndSettle();

      // Verify section title is visible
      expect(find.text('SOURCES ED2K & PAIRS DE SESSION'), findsOneWidget);

      // Source vivante (allowed) affichée — sémantique eMule srclist.
      expect(find.text('127.0.0.1:39735'), findsOneWidget);
      expect(find.text('Autorisé'), findsOneWidget);
      // Entrées deadlist masquées : filtrée IP et échec en cooldown ne
      // doivent pas apparaître (eMule les retire de la liste).
      expect(find.text('198.51.100.22:39735'), findsNothing);
      expect(find.text('203.0.113.9:4662'), findsNothing);
      expect(find.text('Filtré IP'), findsNothing);
      expect(find.text('Échec'), findsNothing);

      // Test adding a manual source
      final manualInput = find.byType(TextField);
      expect(manualInput, findsWidgets);
      await tester.ensureVisible(manualInput.first);
      await tester.pumpAndSettle();
      await tester.enterText(manualInput.first, '10.0.0.5:39735');
      await tester.tap(find.text('Ajouter'));
      await tester.pumpAndSettle();

      expect(find.text('10.0.0.5:39735'), findsOneWidget);

      // Test connecting to an allowed source
      final connectButtons = find.widgetWithText(ElevatedButton, 'Connecter');
      expect(connectButtons, findsWidgets);

      await tester.ensureVisible(connectButtons.first);
      await tester.pumpAndSettle();
      await tester.tap(connectButtons.first);
      await tester.pumpAndSettle();

      // Verify session banner is displayed
      expect(find.text('Téléchargement'), findsWidgets);
      expect(find.textContaining('Session active :'), findsOneWidget);

      // Test disconnecting
      final disconnectBtn = find
          .widgetWithText(OutlinedButton, 'Déconnecter')
          .first;
      await tester.ensureVisible(disconnectBtn);
      await tester.pumpAndSettle();
      await tester.tap(disconnectBtn);
      await tester.pumpAndSettle();

      // Session banner is gone
      expect(find.textContaining('Session active :'), findsNothing);
    },
  );
}
