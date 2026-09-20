import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../app/providers.dart';
import '../../downloads/view_models/downloads_view_model.dart';

enum SearchTarget {
  server,
  global,
  kad;

  String get label {
    switch (this) {
      case SearchTarget.server:
        return 'Serveur eD2K';
      case SearchTarget.global:
        return 'Global (UDP)';
      case SearchTarget.kad:
        return 'Réseau Kad DHT';
    }
  }
}

/// Search section (eD2K server / global UDP / Kad DHT).
///
/// Rebuilds are driven by the domain controllers (search results, kad
/// status, server session); the root VM facade is used for cross-domain
/// actions and reads that have not migrated yet.
class SearchView extends ConsumerStatefulWidget {
  const SearchView({super.key});

  @override
  ConsumerState<SearchView> createState() => _SearchViewState();
}

class _SearchViewState extends ConsumerState<SearchView> {
  final TextEditingController _queryController = TextEditingController();
  final TextEditingController _minSizeController = TextEditingController();
  final TextEditingController _maxSizeController = TextEditingController();
  final TextEditingController _extController = TextEditingController();
  final ScrollController _scrollController = ScrollController();
  SearchTarget _searchTarget = SearchTarget.server;
  bool _showFilters = false;
  int _sortColumnIndex = 2;
  bool _sortAscending = false;

  List<SearchResultFileModel> _cachedResults = const [];
  int _lastResultsLength = -1;
  int _lastSortCol = -1;
  bool _lastSortAsc = false;
  int _lastSearchEpoch = -1;
  List<SearchResultFileModel>? _lastRawListRef;

  @override
  void dispose() {
    _queryController.dispose();
    _minSizeController.dispose();
    _maxSizeController.dispose();
    _extController.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  List<SearchResultFileModel> _getSortedResults(
    List<SearchResultFileModel> rawResults,
    int epoch,
  ) {
    if (rawResults.isEmpty) {
      _cachedResults = const [];
      _lastResultsLength = 0;
      _lastSearchEpoch = epoch;
      _lastRawListRef = null;
      return const [];
    }

    if (identical(rawResults, _lastRawListRef) &&
        epoch == _lastSearchEpoch &&
        rawResults.length == _lastResultsLength &&
        _sortColumnIndex == _lastSortCol &&
        _sortAscending == _lastSortAsc) {
      return _cachedResults;
    }

    final sorted = List<SearchResultFileModel>.from(rawResults);
    sorted.sort((a, b) {
      int cmp = 0;
      switch (_sortColumnIndex) {
        case 0:
          cmp = a.fileName.toLowerCase().compareTo(b.fileName.toLowerCase());
          break;
        case 1:
          cmp = a.fileSize.compareTo(b.fileSize);
          break;
        case 2:
          cmp = a.sourceCount.compareTo(b.sourceCount);
          break;
        default:
          cmp = a.sourceCount.compareTo(b.sourceCount);
      }
      return _sortAscending ? cmp : -cmp;
    });

    _lastRawListRef = rawResults;
    _lastResultsLength = rawResults.length;
    _lastSearchEpoch = epoch;
    _lastSortCol = _sortColumnIndex;
    _lastSortAsc = _sortAscending;
    _cachedResults = sorted;
    return sorted;
  }

  void _handleSearch() {
    _cachedResults = const [];
    _lastResultsLength = -1;
    _lastSearchEpoch = -1;
    _lastRawListRef = null;

    final term = _queryController.text.trim();
    if (term.isEmpty) return;

    final minMb = int.tryParse(_minSizeController.text.trim());
    final maxMb = int.tryParse(_maxSizeController.text.trim());
    final ext = _extController.text.trim();

    final minBytes = minMb != null ? minMb * 1024 * 1024 : null;
    final maxBytes = maxMb != null ? maxMb * 1024 * 1024 : null;

    final vm = ref.read(downloadsViewModelProvider);
    if (_searchTarget == SearchTarget.kad) {
      vm.searchKadKeywords(
        term: term,
        minSize: minBytes,
        maxSize: maxBytes,
      );
    } else {
      vm.searchServer(
        term: term,
        minSize: minBytes,
        maxSize: maxBytes,
        extension: ext.isNotEmpty ? ext : null,
        udpOnly: _searchTarget == SearchTarget.global,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final vm = ref.watch(downloadsViewModelProvider);
    // Rebuild triggers — domain controllers actually consumed by this view.
    ref.watch(searchControllerProvider);
    ref.watch(kadControllerProvider);
    ref.watch(serversControllerProvider);
    final isConnected =
        vm.serverSession.status == ServerConnectionStatus.connected;
    final isKadActive = vm.kadStatus.isUdpActive;
    final isSearchReady = switch (_searchTarget) {
      SearchTarget.kad => isKadActive,
      // Global UDP search only needs a populated server list.
      SearchTarget.global => true,
      SearchTarget.server => isConnected,
    };

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _buildToolbar(context, vm),
        _buildConnectionStatusBanner(context, vm, isConnected, isKadActive),
        _buildSearchForm(context, vm, isSearchReady),
        const Divider(color: AppTheme.darkBorder, height: 1),
        Expanded(
          child: vm.isSearching && vm.searchResults.isEmpty
              ? _buildSearchingState(context)
              : vm.searchResults.isEmpty
                  ? _buildEmptyState(context, vm, isSearchReady)
                  : _buildResultsTable(context, vm),
        ),
      ],
    );
  }

  // ── Toolbar ────────────────────────────────────────────────────────────────

  Widget _buildToolbar(BuildContext context, DownloadsViewModel vm) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
      decoration: const BoxDecoration(
        color: AppTheme.darkSurface,
        border: Border(bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5)),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isCompact = constraints.maxWidth < 650;

          return Row(
            children: [
              const Icon(Icons.search_rounded, color: AppTheme.primary, size: 22),
              const SizedBox(width: 10),
              const Expanded(
                child: Text(
                  'Recherche sur le Réseau eD2K',
                  style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.bold,
                    color: AppColors.textPrimary,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              if (vm.searchResults.isNotEmpty) ...[
                if (!isCompact) ...[
                  Text(
                    '${vm.searchResults.length} résultat(s)',
                    style: const TextStyle(color: AppColors.textMuted, fontSize: 13),
                  ),
                  const SizedBox(width: 12),
                  OutlinedButton.icon(
                    icon: const Icon(Icons.clear_all_rounded, size: 16),
                    label: const Text('Effacer les résultats'),
                    onPressed: () => vm.clearSearchResults(),
                  ),
                ] else ...[
                  IconButton(
                    tooltip: 'Effacer les résultats (${vm.searchResults.length})',
                    icon: const Icon(Icons.clear_all_rounded, size: 18),
                    onPressed: () => vm.clearSearchResults(),
                  ),
                ],
              ],
            ],
          );
        },
      ),
    );
  }

  // ── Connection Status Banner ───────────────────────────────────────────────

  Widget _buildConnectionStatusBanner(
    BuildContext context,
    DownloadsViewModel vm,
    bool isConnected,
    bool isKadActive,
  ) {
    if (_searchTarget == SearchTarget.kad) {
      if (isKadActive) {
        final kad = vm.kadStatus;
        return Container(
          margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
          decoration: BoxDecoration(
            color: AppColors.tintBlue.withAlpha(20),
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: AppColors.accentLightBlue.withAlpha(80)),
          ),
          child: Row(
            children: [
              const Icon(Icons.hub_rounded, color: AppColors.accentLightBlue, size: 18),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  'Kad DHT Actif • Contacts: ${kad.totalContacts} (${kad.activeBuckets} k-buckets) • Bind: ${kad.udpBindAddress ?? "Inconnu"} • Profil: ${kad.networkProfile}',
                  style: const TextStyle(fontSize: 13, color: AppColors.textSecondary),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
        );
      } else {
        return Container(
          margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
          decoration: BoxDecoration(
            color: AppColors.warning.withAlpha(20),
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: AppColors.accentAmber.withAlpha(80)),
          ),
          child: Row(
            children: [
              const Icon(Icons.warning_amber_rounded, color: AppColors.accentAmber, size: 20),
              const SizedBox(width: 10),
              const Expanded(
                child: Text(
                  'Le réseau Kad DHT n\'est pas encore actif. Démarrez le socket Kad ou effectuez un bootstrap.',
                  style: TextStyle(fontSize: 13, color: AppColors.accentAmber),
                ),
              ),
              const SizedBox(width: 10),
              ElevatedButton.icon(
                icon: const Icon(Icons.hub_rounded, size: 14),
                label: const Text('Aller à Kad'),
                style: ElevatedButton.styleFrom(
                  backgroundColor: AppColors.warningShade700,
                  foregroundColor: AppColors.textPrimary,
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                  textStyle: const TextStyle(fontSize: 12),
                ),
                onPressed: () => vm.setSection(AppNavSection.kad),
              ),
            ],
          ),
        );
      }
    }

    // Global UDP search targets the known-server list; a TCP connection is
    // helpful (the connected server is queried too) but not required.
    if (_searchTarget == SearchTarget.global && !isConnected) {
      return Container(
        margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
        decoration: BoxDecoration(
          color: AppColors.tintBlue.withAlpha(20),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: AppColors.accentLightBlue.withAlpha(80)),
        ),
        child: const Row(
          children: [
            Icon(Icons.public_rounded, color: AppColors.accentLightBlue, size: 18),
            SizedBox(width: 10),
            Expanded(
              child: Text(
                'Recherche globale UDP — interroge tous les serveurs connus '
                '(REQ3 si le serveur supporte EXT_GETFILES + LARGEFILES).',
                style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
              ),
            ),
          ],
        ),
      );
    }

    if (isConnected) {
      final session = vm.serverSession;
      return Container(
        margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
        decoration: BoxDecoration(
          color: AppColors.tintGreen.withAlpha(20),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: AppColors.positive.withAlpha(80)),
        ),
        child: Row(
          children: [
            const Icon(Icons.cloud_done_rounded, color: AppColors.positive, size: 18),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                'Connecté à ${session.serverName ?? session.serverAddress} (${session.serverAddress})'
                ' • Client ID: ${session.clientId != null ? "0x${session.clientId!.toRadixString(16).padLeft(8, '0')}" : "Inconnu"} (${session.isHighId ? "HighID" : "LowID"})',
                style: const TextStyle(fontSize: 13, color: AppColors.textSecondary),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
      );
    } else {
      return Container(
        margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
        decoration: BoxDecoration(
          color: AppColors.warning.withAlpha(20),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: AppColors.accentAmber.withAlpha(80)),
        ),
        child: LayoutBuilder(
          builder: (context, constraints) {
            final isNarrow = constraints.maxWidth < 650;

            if (isNarrow) {
              return Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: const [
                      Icon(Icons.warning_amber_rounded, color: AppColors.accentAmber, size: 20),
                      SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          'Non connecté à un serveur eD2K.',
                          style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold, color: AppColors.accentAmber),
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 6),
                  const Text(
                    'Connectez-vous d\'abord pour effectuer une recherche.',
                    style: TextStyle(fontSize: 12, color: AppColors.accentAmber),
                  ),
                  const SizedBox(height: 8),
                  ElevatedButton.icon(
                    icon: const Icon(Icons.dns_rounded, size: 14),
                    label: const Text('Aller aux Serveurs'),
                    style: ElevatedButton.styleFrom(
                      backgroundColor: AppColors.warningShade700,
                      foregroundColor: AppColors.textPrimary,
                      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                      textStyle: const TextStyle(fontSize: 12),
                    ),
                    onPressed: () => vm.setSection(AppNavSection.servers),
                  ),
                ],
              );
            }

            return Row(
              children: [
                const Icon(Icons.warning_amber_rounded, color: AppColors.accentAmber, size: 20),
                const SizedBox(width: 10),
                const Expanded(
                  child: Text(
                    'Vous n\'êtes actuellement pas connecté à un serveur eD2K. Connectez-vous d\'abord pour effectuer une recherche.',
                    style: TextStyle(fontSize: 13, color: AppColors.accentAmber),
                  ),
                ),
                const SizedBox(width: 10),
                ElevatedButton.icon(
                  icon: const Icon(Icons.dns_rounded, size: 14),
                  label: const Text('Aller aux Serveurs'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: AppColors.warningShade700,
                    foregroundColor: AppColors.textPrimary,
                    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                    textStyle: const TextStyle(fontSize: 12),
                  ),
                  onPressed: () => vm.setSection(AppNavSection.servers),
                ),
              ],
            );
          },
        ),
      );
    }
  }

  // ── Search Form ────────────────────────────────────────────────────────────

  Widget _buildSearchForm(
    BuildContext context,
    DownloadsViewModel vm,
    bool isSearchReady,
  ) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isStacked = constraints.maxWidth < 750;

          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (isStacked) ...[
                // Target selector on first row
                Row(
                  children: [
                    Flexible(
                      child: SingleChildScrollView(
                        scrollDirection: Axis.horizontal,
                        child: SegmentedButton<SearchTarget>(
                          segments: const [
                            ButtonSegment(
                              value: SearchTarget.server,
                              icon: Icon(Icons.dns_rounded, size: 16),
                              label: Text('Serveur eD2K', style: TextStyle(fontSize: 12)),
                            ),
                            ButtonSegment(
                              value: SearchTarget.global,
                              icon: Icon(Icons.public_rounded, size: 16),
                              label: Text('Global UDP', style: TextStyle(fontSize: 12)),
                            ),
                            ButtonSegment(
                              value: SearchTarget.kad,
                              icon: Icon(Icons.hub_rounded, size: 16),
                              label: Text('Kad DHT', style: TextStyle(fontSize: 12)),
                            ),
                          ],
                          selected: {_searchTarget},
                          onSelectionChanged: (set) {
                            setState(() {
                              _searchTarget = set.first;
                            });
                          },
                          style: ButtonStyle(
                            visualDensity: VisualDensity.compact,
                            tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                          ),
                        ),
                      ),
                    ),
                    IconButton(
                      icon: Icon(
                        _showFilters ? Icons.filter_alt_off_rounded : Icons.filter_alt_rounded,
                        color: _showFilters ? AppTheme.primary : AppColors.textMuted,
                      ),
                      tooltip: _showFilters ? 'Masquer les filtres' : 'Afficher les filtres avancés',
                      onPressed: () {
                        setState(() {
                          _showFilters = !_showFilters;
                        });
                      },
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                // Query field and search button
                Row(
                  children: [
                    Expanded(
                      child: _buildQueryTextField(isSearchReady, vm),
                    ),
                    const SizedBox(width: 8),
                    _buildSearchButton(isSearchReady, vm),
                  ],
                ),
              ] else ...[
                Row(
                  children: [
                    SegmentedButton<SearchTarget>(
                      segments: const [
                        ButtonSegment(
                          value: SearchTarget.server,
                          icon: Icon(Icons.dns_rounded, size: 16),
                          label: Text('Serveur eD2K', style: TextStyle(fontSize: 13)),
                        ),
                        ButtonSegment(
                          value: SearchTarget.global,
                          icon: Icon(Icons.public_rounded, size: 16),
                          label: Text('Global UDP', style: TextStyle(fontSize: 13)),
                        ),
                        ButtonSegment(
                          value: SearchTarget.kad,
                          icon: Icon(Icons.hub_rounded, size: 16),
                          label: Text('Réseau Kad DHT', style: TextStyle(fontSize: 13)),
                        ),
                      ],
                      selected: {_searchTarget},
                      onSelectionChanged: (set) {
                        setState(() {
                          _searchTarget = set.first;
                        });
                      },
                      style: ButtonStyle(
                        visualDensity: VisualDensity.compact,
                        tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: _buildQueryTextField(isSearchReady, vm),
                    ),
                    const SizedBox(width: 8),
                    _buildSearchButton(isSearchReady, vm),
                    IconButton(
                      visualDensity: VisualDensity.compact,
                      icon: Icon(
                        _showFilters ? Icons.filter_alt_off_rounded : Icons.filter_alt_rounded,
                        color: _showFilters ? AppTheme.primary : AppColors.textMuted,
                      ),
                      tooltip: _showFilters ? 'Masquer les filtres' : 'Afficher les filtres avancés',
                      onPressed: () {
                        setState(() {
                          _showFilters = !_showFilters;
                        });
                      },
                    ),
                  ],
                ),
              ],
          if (_showFilters) ...[
            const SizedBox(height: 10),
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: AppTheme.darkSurface.withAlpha(150),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: AppTheme.darkBorder),
              ),
              child: Row(
                children: [
                  if (_searchTarget != SearchTarget.kad) ...[
                    Expanded(
                      flex: 2,
                      child: TextField(
                        controller: _extController,
                        style: const TextStyle(color: AppColors.textPrimary, fontSize: 13),
                        decoration: const InputDecoration(
                          labelText: 'Extension (ex: iso, zip, avi)',
                          labelStyle: TextStyle(color: AppColors.textMuted, fontSize: 12),
                          isDense: true,
                          contentPadding: EdgeInsets.symmetric(horizontal: 10, vertical: 8),
                          border: OutlineInputBorder(),
                        ),
                      ),
                    ),
                    const SizedBox(width: 12),
                  ],
                  Expanded(
                    child: TextField(
                      controller: _minSizeController,
                      keyboardType: TextInputType.number,
                      inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                      style: const TextStyle(color: AppColors.textPrimary, fontSize: 13),
                      decoration: const InputDecoration(
                        labelText: 'Taille Min (Mo)',
                        labelStyle: TextStyle(color: AppColors.textMuted, fontSize: 12),
                        isDense: true,
                        contentPadding: EdgeInsets.symmetric(horizontal: 10, vertical: 8),
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: TextField(
                      controller: _maxSizeController,
                      keyboardType: TextInputType.number,
                      inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                      style: const TextStyle(color: AppColors.textPrimary, fontSize: 13),
                      decoration: const InputDecoration(
                        labelText: 'Taille Max (Mo)',
                        labelStyle: TextStyle(color: AppColors.textMuted, fontSize: 12),
                        isDense: true,
                        contentPadding: EdgeInsets.symmetric(horizontal: 10, vertical: 8),
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                  const SizedBox(width: 12),
                  TextButton.icon(
                    icon: const Icon(Icons.clear_rounded, size: 14),
                    label: const Text('Effacer filtres', style: TextStyle(fontSize: 12)),
                    onPressed: () {
                      setState(() {
                        _extController.clear();
                        _minSizeController.clear();
                        _maxSizeController.clear();
                      });
                    },
                  ),
                ],
              ),
            ),
          ],
        ],
      );
    },
  ),
);
}

  Widget _buildQueryTextField(bool isSearchReady, DownloadsViewModel vm) {
    return TextField(
      controller: _queryController,
      enabled: isSearchReady && !vm.isSearching,
      style: const TextStyle(color: AppColors.textPrimary, fontSize: 14),
      decoration: InputDecoration(
        hintText: _searchTarget == SearchTarget.kad
            ? 'Mots-clés Kad (min. 3 lettres)...'
            : 'Nom de fichier ou mots-clés...',
        hintStyle: const TextStyle(color: AppColors.textFaint, fontSize: 14),
        prefixIcon: const Icon(Icons.search, color: AppColors.textFaint, size: 20),
        suffixIcon: _queryController.text.isNotEmpty
            ? IconButton(
                icon: const Icon(Icons.clear, size: 16, color: AppColors.textFaint),
                onPressed: () {
                  setState(() {
                    _queryController.clear();
                  });
                },
              )
            : null,
        filled: true,
        fillColor: AppTheme.darkSurface,
        contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: const BorderSide(color: AppTheme.darkBorder),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: const BorderSide(color: AppTheme.darkBorder),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: const BorderSide(color: AppTheme.primary, width: 1.5),
        ),
      ),
      onSubmitted: (_) => _handleSearch(),
      onChanged: (_) => setState(() {}),
    );
  }

  Widget _buildSearchButton(bool isSearchReady, DownloadsViewModel vm) {
    return ElevatedButton.icon(
      icon: vm.isSearching
          ? const SizedBox(
              width: 16,
              height: 16,
              child: CircularProgressIndicator(strokeWidth: 2, color: AppColors.textPrimary),
            )
          : const Icon(Icons.search_rounded, size: 18),
      label: Text(vm.isSearching ? 'Recherche...' : 'Rechercher'),
      style: ElevatedButton.styleFrom(
        backgroundColor: AppTheme.primary,
        foregroundColor: AppColors.textPrimary,
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
      ),
      onPressed: (isSearchReady && !vm.isSearching && _queryController.text.trim().isNotEmpty)
          ? _handleSearch
          : null,
    );
  }

  // ── Searching State ────────────────────────────────────────────────────────

  Widget _buildSearchingState(BuildContext context) {
    final targetLabel = switch (_searchTarget) {
      SearchTarget.kad => 'le réseau Kad DHT',
      SearchTarget.global => 'les serveurs eD2K (UDP)',
      SearchTarget.server => 'le serveur eD2K',
    };
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const CircularProgressIndicator(color: AppTheme.primary),
          const SizedBox(height: 16),
          Text(
            'Recherche en cours sur $targetLabel...',
            style: TextStyle(fontSize: 14, color: AppColors.textPrimary.withAlpha(180)),
          ),
        ],
      ),
    );
  }

  // ── Empty State ────────────────────────────────────────────────────────────

  Widget _buildEmptyState(
    BuildContext context,
    DownloadsViewModel vm,
    bool isSearchReady,
  ) {
    final hasSearched = vm.lastSearchTerm != null;
    final targetName = switch (_searchTarget) {
      SearchTarget.kad => 'le réseau Kad DHT',
      SearchTarget.global => 'la liste de serveurs eD2K',
      SearchTarget.server => 'un serveur eD2K',
    };
    final helpText = switch (_searchTarget) {
      SearchTarget.kad =>
        'Les requêtes KADEMLIA2_SEARCH_KEYWORD interrogent les nœuds Kad stockant l\'index du mot-clé MD4.',
      SearchTarget.global =>
        'Les requêtes OP_GLOBSEARCHREQ3/REQ2 sont envoyées en UDP à tous les serveurs connus (port TCP+4). '
            'REQ3 est utilisé quand le serveur annonce EXT_GETFILES + LARGEFILES.',
      SearchTarget.server =>
        'Les requêtes OP_SEARCHREQUEST interrogent directement l\'index du serveur eD2K connecté.',
    };

    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            hasSearched ? Icons.search_off_rounded : Icons.search_rounded,
            size: 56,
            color: AppColors.fillMedium,
          ),
          const SizedBox(height: 16),
          Text(
            hasSearched
                ? 'Aucun résultat trouvé pour « ${vm.lastSearchTerm} »'
                : isSearchReady
                    ? 'Entrez des mots-clés ci-dessus pour lancer une recherche'
                    : 'Activez $targetName pour commencer à chercher des fichiers',
            style: const TextStyle(
              fontSize: 15,
              fontWeight: FontWeight.w500,
              color: AppColors.textMuted,
            ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 6),
          Text(
            hasSearched
                ? 'Essayez avec d\'autres termes ou élargissez vos filtres de taille'
                : helpText,
            style: const TextStyle(
              fontSize: 12,
              color: AppColors.iconFaint,
            ),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }

  // ── Results Table ──────────────────────────────────────────────────────────

  // ── Results Table (Virtualized) ───────────────────────────────────────────

  Widget _buildResultsTable(BuildContext context, DownloadsViewModel vm) {
    final results = _getSortedResults(vm.searchResults, vm.searchEpoch);
    final downloadsByHash = <String, DownloadItem>{};
    for (final d in vm.allDownloads) {
      if (d.ed2kHash != null) {
        downloadsByHash[d.ed2kHash!.toLowerCase()] = d;
      }
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        const minTableWidth = 950.0;
        final tableWidth = constraints.maxWidth < minTableWidth ? minTableWidth : constraints.maxWidth;

        return SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: SizedBox(
            width: tableWidth,
            child: Column(
              children: [
                _buildTableHeader(context),
                const Divider(color: AppTheme.darkBorder, height: 1, thickness: 0.5),
                Expanded(
                  child: Scrollbar(
                    controller: _scrollController,
                    thumbVisibility: true,
                    child: ListView.builder(
                      controller: _scrollController,
                      itemCount: results.length,
                      itemExtent: 44.0,
                      itemBuilder: (context, index) {
                        final file = results[index];
                        final existingDl = downloadsByHash[file.fileHash.toLowerCase()];
                        return _buildResultRow(context, file, index, vm, existingDl);
                      },
                    ),
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }

  Widget _buildTableHeader(BuildContext context) {
    return Container(
      height: 40,
      color: AppTheme.darkSurface,
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: Row(
        children: [
          _buildHeaderCell(label: 'Nom du fichier', columnIndex: 0, flex: 4),
          const SizedBox(width: 12),
          _buildHeaderCell(label: 'Taille', columnIndex: 1, width: 95, numeric: true),
          const SizedBox(width: 12),
          _buildHeaderCell(label: 'Sources', columnIndex: 2, width: 100, numeric: true),
          const SizedBox(width: 12),
          _buildHeaderCell(label: 'Hachage eD2K', columnIndex: 3, width: 220, sortable: false),
          const SizedBox(width: 12),
          _buildHeaderCell(label: 'Actions', columnIndex: 4, width: 280, sortable: false),
        ],
      ),
    );
  }

  Widget _buildHeaderCell({
    required String label,
    required int columnIndex,
    int flex = 0,
    double? width,
    bool numeric = false,
    bool sortable = true,
  }) {
    final isSorted = _sortColumnIndex == columnIndex;
    final content = Row(
      mainAxisSize: MainAxisSize.min,
      mainAxisAlignment: numeric ? MainAxisAlignment.end : MainAxisAlignment.start,
      children: [
        Flexible(
          child: Text(
            label,
            style: TextStyle(
              fontWeight: FontWeight.bold,
              color: isSorted ? AppColors.textPrimary : AppColors.textSecondary,
              fontSize: 12.5,
            ),
            overflow: TextOverflow.ellipsis,
          ),
        ),
        if (sortable) ...[
          const SizedBox(width: 4),
          Icon(
            isSorted
                ? (_sortAscending ? Icons.arrow_upward_rounded : Icons.arrow_downward_rounded)
                : Icons.unfold_more_rounded,
            size: 14,
            color: isSorted ? AppTheme.primary : AppColors.fillMedium,
          ),
        ],
      ],
    );

    final Widget cellWidget;
    if (sortable) {
      cellWidget = Align(
        alignment: numeric ? Alignment.centerRight : Alignment.centerLeft,
        child: InkWell(
          onTap: () {
            setState(() {
              if (_sortColumnIndex == columnIndex) {
                _sortAscending = !_sortAscending;
              } else {
                _sortColumnIndex = columnIndex;
                _sortAscending = (columnIndex == 0);
              }
            });
          },
          borderRadius: BorderRadius.circular(4),
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 4),
            child: content,
          ),
        ),
      );
    } else {
      cellWidget = Align(
        alignment: numeric ? Alignment.centerRight : Alignment.centerLeft,
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 4),
          child: content,
        ),
      );
    }

    if (flex > 0) {
      return Expanded(flex: flex, child: cellWidget);
    }
    return SizedBox(width: width, child: cellWidget);
  }

  Widget _buildResultRow(
    BuildContext context,
    SearchResultFileModel file,
    int index,
    DownloadsViewModel vm,
    DownloadItem? existingDl,
  ) {
    final isCompleted = existingDl != null && existingDl.status.isCompleted;

    return Container(
      height: 44.0,
      decoration: BoxDecoration(
        border: const Border(
          bottom: BorderSide(color: AppTheme.darkBorder, width: 0.3),
        ),
        color: index.isEven ? Colors.transparent : AppColors.textPrimary.withAlpha(2),
      ),
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          hoverColor: AppTheme.primary.withAlpha(15),
          onTap: () {},
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(
              children: [
                // Nom du fichier
                Expanded(
                  flex: 4,
                  child: Row(
                    children: [
                      const Icon(Icons.insert_drive_file_outlined,
                          size: 16, color: AppColors.textMuted),
                      const SizedBox(width: 8),
                      Flexible(
                        child: Tooltip(
                          message: file.fileName,
                          waitDuration: const Duration(milliseconds: 500),
                          child: Text(
                            file.fileName,
                            style: const TextStyle(
                              color: AppColors.textPrimary,
                              fontWeight: FontWeight.w500,
                              fontSize: 13,
                            ),
                            overflow: TextOverflow.ellipsis,
                            maxLines: 1,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 12),

                // Taille
                SizedBox(
                  width: 95,
                  child: Align(
                    alignment: Alignment.centerRight,
                    child: Text(
                      _formatBytes(file.fileSize),
                      style: const TextStyle(color: AppColors.textSecondary, fontSize: 12.5),
                    ),
                  ),
                ),
                const SizedBox(width: 12),

                // Sources
                SizedBox(
                  width: 100,
                  child: Align(
                    alignment: Alignment.centerRight,
                    child: Container(
                      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                      decoration: BoxDecoration(
                        color: file.sourceCount > 0
                            ? AppColors.tintGreen.withAlpha(30)
                            : AppColors.fillSubtle,
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(
                        '${file.sourceCount}',
                        style: TextStyle(
                          color: file.sourceCount > 0
                              ? AppColors.positive
                              : AppColors.textFaint,
                          fontWeight: FontWeight.bold,
                          fontSize: 12,
                        ),
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 12),

                // Hachage eD2K
                SizedBox(
                  width: 220,
                  child: Tooltip(
                    message: 'Cliquer pour copier le hash : ${file.fileHash}',
                    waitDuration: const Duration(milliseconds: 400),
                    child: InkWell(
                      borderRadius: BorderRadius.circular(4),
                      onTap: () {
                        Clipboard.setData(ClipboardData(text: file.fileHash));
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(
                            content: Text('Hachage copié : ${file.fileHash}'),
                            duration: const Duration(seconds: 2),
                          ),
                        );
                      },
                      child: Padding(
                        padding: const EdgeInsets.symmetric(vertical: 4, horizontal: 2),
                        child: Text(
                          file.fileHash,
                          style: const TextStyle(
                            fontFamily: 'monospace',
                            fontSize: 11,
                            color: AppColors.textMuted,
                          ),
                          overflow: TextOverflow.ellipsis,
                          maxLines: 1,
                        ),
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 12),

                // Actions
                SizedBox(
                  width: 280,
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (isCompleted)
                        OutlinedButton.icon(
                          icon: const Icon(Icons.check_circle_rounded,
                              size: 14, color: AppTheme.statusCompleted),
                          label: const Text('Terminé',
                              style: TextStyle(fontSize: 12, color: AppTheme.statusCompleted)),
                          style: OutlinedButton.styleFrom(
                            side: const BorderSide(color: AppTheme.statusCompleted),
                            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                            visualDensity: VisualDensity.compact,
                          ),
                          onPressed: () {
                            vm.setSection(AppNavSection.downloads);
                          },
                        )
                      else if (existingDl != null)
                        OutlinedButton.icon(
                          icon: const Icon(Icons.sync_rounded,
                              size: 14, color: AppTheme.primary),
                          label: Text(existingDl.status.label,
                              style: const TextStyle(fontSize: 12, color: AppTheme.primary)),
                          style: OutlinedButton.styleFrom(
                            side: const BorderSide(color: AppTheme.primary),
                            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                            visualDensity: VisualDensity.compact,
                          ),
                          onPressed: () {
                            vm.setSection(AppNavSection.downloads);
                          },
                        )
                      else
                        ElevatedButton.icon(
                          icon: const Icon(Icons.download_rounded, size: 14),
                          label: const Text('Télécharger', style: TextStyle(fontSize: 12)),
                          style: ElevatedButton.styleFrom(
                            backgroundColor: AppTheme.primary,
                            foregroundColor: AppColors.textPrimary,
                            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                            visualDensity: VisualDensity.compact,
                          ),
                          onPressed: () async {
                            final success = await vm.downloadSearchResult(file);
                            if (context.mounted) {
                              ScaffoldMessenger.of(context).showSnackBar(
                                SnackBar(
                                  content: Text(
                                    success
                                        ? 'Téléchargement lancé : ${file.fileName}'
                                        : (vm.errorMessage ?? 'Échec de l\'ajout au téléchargement'),
                                  ),
                                  duration: const Duration(seconds: 4),
                                  backgroundColor: success ? null : AppColors.accentOrangeShade700,
                                  action: success
                                      ? SnackBarAction(
                                          label: 'Afficher',
                                          onPressed: () {
                                            vm.setSection(AppNavSection.downloads);
                                          },
                                        )
                                      : null,
                                ),
                              );
                            }
                          },
                        ),
                      const SizedBox(width: 4),
                      IconButton(
                        icon: const Icon(Icons.hub_outlined, size: 16),
                        tooltip: 'Demander sources au serveur',
                        padding: EdgeInsets.zero,
                        constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
                        onPressed: () {
                          vm.queryServerSources(file.fileHash, fileSize: file.fileSize);
                        },
                      ),
                      IconButton(
                        icon: const Icon(Icons.copy_rounded, size: 16),
                        tooltip: 'Copier lien eD2K',
                        padding: EdgeInsets.zero,
                        constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
                        onPressed: () {
                          Clipboard.setData(ClipboardData(text: file.ed2kLink));
                          ScaffoldMessenger.of(context).showSnackBar(
                            const SnackBar(
                              content: Text('Lien eD2K copié dans le presse-papier'),
                              duration: Duration(seconds: 2),
                            ),
                          );
                        },
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }


  String _formatBytes(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
    if (bytes < 1024 * 1024 * 1024) {
      return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(2)} GB';
  }
}
