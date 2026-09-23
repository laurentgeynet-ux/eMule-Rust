import '../models/category_item.dart';
import '../models/daemon_event.dart';
import '../models/daemon_info.dart';
import '../models/download_item.dart';
import '../models/filter_models.dart';
import '../models/history_entry.dart';
import '../models/history_stats.dart';
import '../models/lab_models.dart';
import '../models/network_policy_models.dart';
import '../models/part_met_models.dart';
import '../models/server_models.dart';

abstract class MuleDaemonClient {
  /// Connect to the daemon and perform authentication handshake.
  Future<DaemonInfo> connect({required Uri url, String? token});

  /// Disconnect gracefully.
  Future<void> disconnect();

  /// Stream of connection state changes.
  Stream<DaemonConnectionState> get connectionState;

  /// Current connection state snapshot.
  DaemonConnectionState get currentConnectionState;

  /// Cached daemon info snapshot from handshake, if connected.
  DaemonInfo? get daemonInfo;

  /// Stream of download updates emitted by the daemon.
  Stream<DownloadUpdatedEvent> get downloadEvents;

  /// Stream of download lifecycle events (`event.downloadAdded/Removed/
  /// Completed/Failed`). Complements [downloadEvents] which carries
  /// progression updates.
  Stream<DownloadLifecycleEvent> get downloadLifecycleEvents;

  /// Fetch full snapshot of all downloads and current sequence.
  Future<List<DownloadItem>> listDownloads();

  /// Fetch a single download by ID.
  Future<DownloadItem?> getDownload(String id);

  /// Add a new download via eD2K link.
  Future<String> addEd2kLink(
    String link, {
    String? targetDirectory,
    String? allocationPolicy,
    String? categoryId,
  });

  /// Pause an active download.
  Future<DownloadItem> pauseDownload(String id);

  /// Resume a paused download.
  Future<DownloadItem> resumeDownload(String id);

  /// Remove a download, optionally deleting files.
  Future<void> removeDownload(String id, {bool deleteFiles = false});

  /// Inject data range for simulation and integrity verification tests.
  Future<DownloadItem> injectRange(String id, int offset, String hexData);

  /// Request daemon graceful shutdown (system.stop).
  Future<void> stopDaemon();

  /// Fetch lab status (workdir mode, path, fixtures, active test peer).
  Future<LabStatus> getLabStatus();

  /// Create a deterministic test fixture in `<workdir>/fixtures`.
  Future<LabFixtureInfo> createLabFixture(String fixtureType);

  /// Start the loopback test peer (127.0.0.1) sharing all fixtures.
  Future<LabPeerInfo> startLabPeer();

  /// Stop the running loopback test peer.
  Future<void> stopLabPeer();

  /// Clean up test data and fixtures (allowed only in Portable / dev modes).
  Future<LabCleanupReport> cleanupLabTestData();

  /// Connect a download to an active loopback source.
  Future<void> connectDownloadToSource(String downloadId, String peerAddr);

  /// Change finalization destination directory of a download.
  Future<DownloadItem> setDownloadDestination(
    String id,
    String targetDirectory,
  );

  /// `downloads.rename` — rename the download's file name. For a
  /// completed download the file on disk and its share entry are renamed
  /// too; an in-progress download's `.part` keeps its internal name and
  /// the new name applies at finalization.
  Future<DownloadItem> renameDownload(String id, String newName);

  /// Change the eMule-style priority of a download ("low" | "normal" | "high").
  /// The daemon swarm allocator biases its peer slot quota accordingly.
  Future<DownloadItem> setDownloadPriority(String id, String priority);

  /// Per-file source cap (eMule FT_MAXSOURCES). `null` restores the
  /// global per-file limit.
  Future<DownloadItem> setDownloadMaxSources(String id, int? maxSources);

  /// Retry connecting loopback sources for a download.
  Future<DownloadItem> retryDownloadConnection(String id);

  /// Get persistent daemon settings (default download directory, etc.)
  Future<Map<String, dynamic>> getSettings();

  /// Applies a settings patch via `settings.set`. Supported keys:
  /// `defaultDownloadDir`, `historyRetentionDays`, `maxGlobalDownloadPeers`,
  /// `maxPeersPerDownload`, `minPeersPerDownload`, `shareDownloadDir`.
  Future<void> updateSettings(Map<String, dynamic> patch);

  /// Set persistent default download directory.
  Future<void> setDefaultDownloadDirectory(String path);

  /// Reset persistent default download directory to portable default.
  Future<void> resetDefaultDownloadDirectory();

  /// List historical download transitions with optional filters.
  Future<List<HistoryEntry>> listHistory({
    String? eventType,
    String? status,
    int? fromTimestampMs,
    int? toTimestampMs,
    int? limit,
    int? offset,
  });

  /// Get aggregated historical download statistics.
  Future<HistoryStats> getHistoryStats();

  /// Clear all history entries and return number of rows removed.
  Future<int> clearHistory();

  /// Export history in 'csv' or 'json' format.
  Future<String> exportHistory(
    String format, {
    String? eventType,
    String? status,
  });

  /// Update persistent history retention policy in days (0 = unlimited).
  Future<void> setHistoryRetentionDays(int days);

  /// List all categories ordered by sortOrder ascending.
  Future<List<CategoryItem>> listCategories();

  /// Create a new category.
  Future<CategoryItem> createCategory({
    required String name,
    required String color,
    String? defaultDestination,
    String? defaultPriority,
  });

  /// Update an existing category.
  Future<CategoryItem> updateCategory(
    String id, {
    String? name,
    String? color,
    String? defaultDestination,
    String? defaultPriority,
    int? sortOrder,
  });

  /// Delete a category and reassign all its downloads to null.
  Future<void> deleteCategory(String id);

  /// Reorder categories.
  Future<void> reorderCategories(List<String> categoryIds);

  /// Set or clear the category assigned to a download.
  Future<DownloadItem> setDownloadCategory(
    String downloadId,
    String? categoryId,
  );

  /// Inspect an eMule .part.met file before importing.
  Future<PartMetInspectionResult> inspectPartMet(
    String metPath, {
    String? partPath,
  });

  /// Import an eMule .part.met file into the local download manager (strictly paused).
  Future<ImportPartMetResult> importPartMet(
    String metPath, {
    String? partPath,
    String? targetDirectory,
    String? categoryId,
  });

  /// Get current network policy snapshot (profile, state, budgets, active conns).
  Future<NetworkPolicySnapshot> getNetworkPolicy();

  /// Set the active network profile.
  Future<NetworkPolicySnapshot> setNetworkProfile(NetworkProfile profile);

  /// Global Kill Switch: immediately disconnect all active sockets and set state to Disabled.
  Future<void> disconnectAll();

  /// Re-enable network policy from Disabled state.
  Future<void> enableNetwork();

  /// Fetch redacted network audit log entries.
  Future<List<NetworkAuditEntryItem>> getNetworkAuditLog({int limit = 50});

  /// Update resource budgets.
  Future<NetworkPolicySnapshot> setResourceBudgets(
    ResourceBudgetsModel budgets,
  );

  /// Fetch IP filter status and statistics.
  Future<FilterStatusModel> getFilterStatus();

  /// Import local ipfilter.dat file.
  Future<FilterStatusModel> importFilterFile(String filePath);

  /// Enable or disable IP filtering.
  Future<FilterStatusModel> setFilterEnabled(bool enabled);

  /// Clear in-memory IP filter.
  Future<FilterStatusModel> clearFilter();

  /// Fetch current rate limits.
  Future<RateLimitsModel> getRateLimits();

  /// Update rate limits.
  Future<RateLimitsModel> setRateLimits(RateLimitsModel limits);

  /// Inspect an eD2K server.met file without storing.
  Future<ServerInspectReport> inspectServerMet(String filePath);

  /// Import an eD2K server.met file into local SQLite storage.
  Future<ServerImportResult> importServerMet(String filePath);

  /// List all persisted eD2K servers.
  Future<List<Ed2kServerModel>> listServers();

  /// Enable or disable a server.
  Future<bool> setServerEnabled(String address, bool enabled);

  /// Delete a server from local storage.
  Future<bool> deleteServer(String address);

  /// Clear all servers from local storage.
  Future<int> clearServers();

  /// Connect to an eD2K server by address ("ip:port").
  Future<ServerSessionModel> connectServer(String address);

  /// Disconnect from the currently connected eD2K server.
  Future<ServerSessionModel> disconnectServer();

  /// Get current eD2K server session status.
  Future<ServerSessionModel> getServerStatus();

  /// Send a search query to eD2K servers. With [udpOnly] (eMule "Global"
  /// scope) only the UDP fan-out to the server list is performed, skipping
  /// the TCP search on the connected server.
  ///
  /// Returns the raw RPC result, including `udpTargets` — the per-server
  /// UDP wire form actually sent (`REQ3`/`REQ2`/`null`).
  Future<Map<String, dynamic>> searchServer({
    required String term,
    int? minSize,
    int? maxSize,
    String? extension,
    bool udpOnly = false,
  });

  /// Request sources from the connected eD2K server for a file hash.
  Future<void> queryServerSources({required String fileHash, int? fileSize});

  /// List registered peer sources for a file hash.
  Future<List<PeerSourceModel>> listSources(String fileHash);

  /// Add a manual peer source for a file hash.
  Future<PeerSourceModel> addManualSource(String fileHash, String address);

  /// Connect to a specific peer source.
  Future<PeerSessionModel> connectPeerSource(
    String fileHash,
    String address, {
    bool force = false,
  });

  /// Disconnect the active peer session, or a specific peer by address.
  Future<PeerSessionModel> disconnectPeerSource({String? address});

  /// Get the status of the active peer session.
  Future<PeerSessionModel> getPeerSessionStatus();

  /// Get the full swarm status snapshot.
  Future<SwarmStatusModel> getSwarmStatus({String? fileHash});

  /// Clear all registered sources for a file hash.
  Future<int> clearSources(String fileHash);

  /// List explicitly shared files.
  Future<List<SharedFileModel>> listSharedFiles();

  /// Register a file for sharing.
  Future<SharedFileModel> addSharedFile({
    required String fileHash,
    required String fileName,
    required int fileSize,
    required String filePath,
  });

  /// Remove a file from sharing. With [deleteFile] the file is also
  /// deleted from disk (and the download entry when it backs a completed
  /// download); rejected by the daemon for partial shares.
  Future<void> removeSharedFile(String fileHash, {bool deleteFile = false});

  /// `shares.rename` — rename a fully shared file: file on disk +
  /// `shared_files` entry (and the linked completed download when one
  /// exists).
  Future<void> renameSharedFile(String fileHash, String newName);

  /// Enable or disable sharing for a file.
  Future<SharedFileModel> setSharedFileEnabled(String fileHash, bool enabled);

  /// eMule upload priority for a shared file (powershare):
  /// "verylow" | "low" | "normal" | "high" | "veryhigh" | "powershare".
  Future<void> setSharedFileUploadPriority(String fileHash, String priority);

  /// `shares.setCategory` — assign (or clear with `null`) a category to a
  /// shared file; for shares synthesized from a download the download's
  /// category is updated.
  Future<void> setSharedFileCategory(String fileHash, String? categoryId);

  /// Re-scans the download (Incoming) directory and registers newly found
  /// files as shared. Returns the number of newly added files.
  Future<int> rescanSharedFiles();

  /// List configured friends (eMule `friends.met`) — userhashes granted a
  /// dedicated upload slot bypassing the queue.
  Future<List<FriendModel>> listFriends();

  /// Grant a dedicated upload slot to [userHash] (hex).
  Future<void> addFriend(String userHash, {String name = ''});

  /// Revoke a friend slot.
  Future<void> removeFriend(String userHash);

  /// Start inbound eD2K TCP listener.
  Future<InboundListenerStatusModel> startListener({int port = 0});

  /// Stop inbound eD2K TCP listener.
  Future<InboundListenerStatusModel> stopListener();

  /// Get status of inbound eD2K listener.
  Future<InboundListenerStatusModel> getListenerStatus();

  /// List active and queued uploads.
  Future<Map<String, dynamic>> listUploads();

  /// Get status of the active single-slot upload.
  Future<UploadSessionModel?> getUploadStatus();

  /// Stream of full uploads snapshots (`upload.uploadsUpdated` events):
  /// active slots, waiting queue and aggregate upload rate.
  Stream<UploadsSnapshotModel> get uploadsEvents;

  /// Stream of aggregate stats ticks (`event.statsTick`, ~2 s cadence):
  /// global rates, active downloads, connected peers, server/Kad state.
  Stream<DaemonStatsModel> get statsEvents;

  /// `stats.resetLifetimeTotals` — remet à zéro les compteurs cumulés
  /// « depuis l'installation » (download + upload) émis dans
  /// `statsTick`. Les totaux de session sont inchangés.
  Future<void> resetLifetimeTotals();

  /// Nudge stream for `kad.statusChanged` (payload = sequence; refetch
  /// `getKadStatus`/`listKadContacts` on receipt).
  Stream<int> get kadStatusEvents;

  /// Nudge stream for `history.entryAdded` (refetch `listHistory`).
  Stream<int> get historyEvents;

  /// Nudge stream for `shares.changed` (payload = sequence; refetch
  /// `listSharedFiles` on receipt). Emitted by share mutations
  /// (`shares.add`/`remove`/`setEnabled`/`setUploadPriority`/`rescan`),
  /// including ones initiated outside the UI (MCP, CLI).
  Stream<int> get sharesEvents;

  /// `settings.changed` notifications with the touched keys.
  Stream<SettingsChangedEvent> get settingsEvents;

  /// Nudge stream for `network.policyChanged` (refetch `getNetworkPolicy`).
  Stream<int> get networkPolicyEvents;

  /// `upnp.statusChanged` notifications carrying the full `UpnpStatus`
  /// snapshot — emitted on every UPnP enable/map/unmap action (including
  /// the daemon's background port-mapping retries).
  Stream<UpnpStatusModel> get upnpStatusEvents;

  /// Cancel an upload or dequeue a waiting peer.
  Future<void> cancelUpload(String clientAddress);

  /// Get secure user identification status.
  Future<SecureIdentityStatusModel> getIdentityStatus();

  /// Voluntarily rotate identity keypair.
  Future<SecureIdentityStatusModel> rotateIdentity();

  /// List persistent peer credits.
  Future<List<PeerCreditModel>> listPeerCredits({int limit = 50});

  /// Get aggregated peer credit statistics.
  Future<PeerCreditStatsModel> getPeerCreditStats();

  /// Get current Kad DHT status and routing table metrics.
  Future<KadStatusModel> getKadStatus();

  /// Start Kad UDP socket on loopback (optional bind address).
  Future<Map<String, dynamic>> startKadUdp({String? bindAddress});

  /// Stop Kad UDP socket and abort in-flight transactions.
  Future<Map<String, dynamic>> stopKadUdp();

  /// Ping a Kad contact by IP and port with optional Kad ID.
  Future<Map<String, dynamic>> pingKadContact({
    required String ip,
    required int port,
    String? kadId,
  });

  /// Bootstrap Kad DHT from a target node IP and port.
  Future<Map<String, dynamic>> bootstrapKad({
    required String ip,
    required int port,
  });

  /// Iteratively find nodes for a target Kad ID.
  Future<List<KadContactModel>> findKadNode(String targetId);

  /// Inspect a local nodes.dat file before importing.
  Future<NodesDatInspectReportModel> inspectNodesDat(String filePath);

  /// Import contacts from a nodes.dat file into SQLite and K-buckets.
  Future<Map<String, dynamic>> importNodesDat(String filePath);

  /// List known Kad contacts.
  Future<List<KadContactModel>> listKadContacts({int limit = 100});

  /// Clear all stored Kad contacts.
  Future<int> clearKadContacts();

  /// Search for sources of a target file hash on Kad DHT.
  Future<List<PeerSourceModel>> searchKadSources({
    required String fileHash,
    int? fileSize,
  });

  /// Search Kad DHT by keywords.
  Future<List<SearchResultFileModel>> searchKadKeywords({
    required String term,
    int? minSize,
    int? maxSize,
  });

  /// Publish a file to Kad DHT as a source.
  Future<int> publishKadSource({
    required String fileHash,
    required int fileSize,
    int tcpPort = 39735,
  });

  /// Publish file keywords to Kad DHT.
  Future<int> publishKadKeywords({
    required String fileHash,
    required String fileName,
    required int fileSize,
  });

  /// Stream of search result events.
  Stream<List<SearchResultFileModel>> get searchResultEvents;

  /// Stream of found sources events.
  Stream<Map<String, dynamic>> get foundSourcesEvents;

  /// Stream of peer session status updates.
  Stream<PeerSessionModel> get peerSessionEvents;

  /// Stream of server session status updates.
  Stream<ServerSessionModel> get serverSessionEvents;

  /// Get current UPnP status and port mappings.
  Future<UpnpStatusModel> getUpnpStatus();

  /// Enable or disable UPnP service.
  Future<UpnpStatusModel> setUpnpEnabled(bool enabled);

  /// Map ports via UPnP IGD. When [tcpPort]/[udpPort] are omitted the
  /// daemon maps its effective eD2K/Kad ports.
  Future<UpnpStatusModel> mapUpnpPorts({int? tcpPort, int? udpPort});

  /// Unmap ports via UPnP IGD.
  Future<UpnpStatusModel> unmapUpnpPorts();

  /// Update server list from remote URL.
  Future<Map<String, dynamic>> updateServersFromUrl(String url);

  /// Bootstrap Kad nodes from remote URL.
  Future<Map<String, dynamic>> bootstrapKadFromUrl(String url);

  /// Clean up any resources/sockets.
  void dispose();
}
