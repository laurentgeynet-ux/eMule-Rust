import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../../domain/models/server_models.dart';
import '../../../core/theme.dart';
import '../../downloads/view_models/downloads_view_model.dart';

enum SearchTarget {
  server,
  kad;

  String get label {
    switch (this) {
      case SearchTarget.server:
        return 'Serveur eD2K';
      case SearchTarget.kad:
        return 'Réseau Kad DHT';
    }
  }
}

class SearchView extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const SearchView({super.key, required this.viewModel});

  @override
  State<SearchView> createState() => _SearchViewState();
}

class _SearchViewState extends State<SearchView> {
  final TextEditingController _queryController = TextEditingController();
  final TextEditingController _minSizeController = TextEditingController();
  final TextEditingController _maxSizeController = TextEditingController();
  final TextEditingController _extController = TextEditingController();
  SearchTarget _searchTarget = SearchTarget.server;
  bool _showFilters = false;

  @override
  void dispose() {
    _queryController.dispose();
    _minSizeController.dispose();
    _maxSizeController.dispose();
    _extController.dispose();
    super.dispose();
  }

  void _handleSearch() {
    final term = _queryController.text.trim();
    if (term.isEmpty) return;

    final minMb = int.tryParse(_minSizeController.text.trim());
    final maxMb = int.tryParse(_maxSizeController.text.trim());
    final ext = _extController.text.trim();

    final minBytes = minMb != null ? minMb * 1024 * 1024 : null;
    final maxBytes = maxMb != null ? maxMb * 1024 * 1024 : null;

    if (_searchTarget == SearchTarget.kad) {
      widget.viewModel.searchKadKeywords(
        term: term,
        minSize: minBytes,
        maxSize: maxBytes,
      );
    } else {
      widget.viewModel.searchServer(
        term: term,
        minSize: minBytes,
        maxSize: maxBytes,
        extension: ext.isNotEmpty ? ext : null,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final vm = widget.viewModel;
    final isConnected = vm.serverSession.status == ServerConnectionStatus.connected;
    final isKadActive = vm.kadStatus.isUdpActive;
    final isSearchReady = _searchTarget == SearchTarget.kad ? isKadActive : isConnected;

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
                    color: Colors.white,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              if (vm.searchResults.isNotEmpty) ...[
                if (!isCompact) ...[
                  Text(
                    '${vm.searchResults.length} résultat(s)',
                    style: const TextStyle(color: Colors.white54, fontSize: 13),
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
            color: Colors.blue.withAlpha(20),
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: Colors.lightBlueAccent.withAlpha(80)),
          ),
          child: Row(
            children: [
              const Icon(Icons.hub_rounded, color: Colors.lightBlueAccent, size: 18),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  'Kad DHT Actif • Contacts: ${kad.totalContacts} (${kad.activeBuckets} k-buckets) • Bind: ${kad.udpBindAddress ?? "Inconnu"} • Profil: ${kad.networkProfile}',
                  style: const TextStyle(fontSize: 13, color: Colors.white70),
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
            color: Colors.amber.withAlpha(20),
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: Colors.amberAccent.withAlpha(80)),
          ),
          child: Row(
            children: [
              const Icon(Icons.warning_amber_rounded, color: Colors.amberAccent, size: 20),
              const SizedBox(width: 10),
              const Expanded(
                child: Text(
                  'Le réseau Kad DHT n\'est pas encore actif. Démarrez le socket Kad ou effectuez un bootstrap.',
                  style: TextStyle(fontSize: 13, color: Colors.amberAccent),
                ),
              ),
              const SizedBox(width: 10),
              ElevatedButton.icon(
                icon: const Icon(Icons.hub_rounded, size: 14),
                label: const Text('Aller à Kad'),
                style: ElevatedButton.styleFrom(
                  backgroundColor: Colors.amber.shade700,
                  foregroundColor: Colors.white,
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

    if (isConnected) {
      final session = vm.serverSession;
      return Container(
        margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
        decoration: BoxDecoration(
          color: Colors.green.withAlpha(20),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: Colors.greenAccent.withAlpha(80)),
        ),
        child: Row(
          children: [
            const Icon(Icons.cloud_done_rounded, color: Colors.greenAccent, size: 18),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                'Connecté à ${session.serverName ?? session.serverAddress} (${session.serverAddress})'
                ' • Client ID: ${session.clientId != null ? "0x${session.clientId!.toRadixString(16).padLeft(8, '0')}" : "Inconnu"} (${session.isHighId ? "HighID" : "LowID"})',
                style: const TextStyle(fontSize: 13, color: Colors.white70),
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
          color: Colors.amber.withAlpha(20),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: Colors.amberAccent.withAlpha(80)),
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
                      Icon(Icons.warning_amber_rounded, color: Colors.amberAccent, size: 20),
                      SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          'Non connecté à un serveur eD2K.',
                          style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold, color: Colors.amberAccent),
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 6),
                  const Text(
                    'Connectez-vous d\'abord pour effectuer une recherche.',
                    style: TextStyle(fontSize: 12, color: Colors.amberAccent),
                  ),
                  const SizedBox(height: 8),
                  ElevatedButton.icon(
                    icon: const Icon(Icons.dns_rounded, size: 14),
                    label: const Text('Aller aux Serveurs'),
                    style: ElevatedButton.styleFrom(
                      backgroundColor: Colors.amber.shade700,
                      foregroundColor: Colors.white,
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
                const Icon(Icons.warning_amber_rounded, color: Colors.amberAccent, size: 20),
                const SizedBox(width: 10),
                const Expanded(
                  child: Text(
                    'Vous n\'êtes actuellement pas connecté à un serveur eD2K. Connectez-vous d\'abord pour effectuer une recherche.',
                    style: TextStyle(fontSize: 13, color: Colors.amberAccent),
                  ),
                ),
                const SizedBox(width: 10),
                ElevatedButton.icon(
                  icon: const Icon(Icons.dns_rounded, size: 14),
                  label: const Text('Aller aux Serveurs'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Colors.amber.shade700,
                    foregroundColor: Colors.white,
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
                    SegmentedButton<SearchTarget>(
                      segments: const [
                        ButtonSegment(
                          value: SearchTarget.server,
                          icon: Icon(Icons.dns_rounded, size: 16),
                          label: Text('Serveur eD2K', style: TextStyle(fontSize: 12)),
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
                    const Spacer(),
                    IconButton(
                      icon: Icon(
                        _showFilters ? Icons.filter_alt_off_rounded : Icons.filter_alt_rounded,
                        color: _showFilters ? AppTheme.primary : Colors.white54,
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
                    const SizedBox(width: 16),
                    Expanded(
                      child: _buildQueryTextField(isSearchReady, vm),
                    ),
                    const SizedBox(width: 10),
                    _buildSearchButton(isSearchReady, vm),
                    const SizedBox(width: 8),
                    IconButton(
                      icon: Icon(
                        _showFilters ? Icons.filter_alt_off_rounded : Icons.filter_alt_rounded,
                        color: _showFilters ? AppTheme.primary : Colors.white54,
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
                  if (_searchTarget == SearchTarget.server) ...[
                    Expanded(
                      flex: 2,
                      child: TextField(
                        controller: _extController,
                        style: const TextStyle(color: Colors.white, fontSize: 13),
                        decoration: const InputDecoration(
                          labelText: 'Extension (ex: iso, zip, avi)',
                          labelStyle: TextStyle(color: Colors.white54, fontSize: 12),
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
                      style: const TextStyle(color: Colors.white, fontSize: 13),
                      decoration: const InputDecoration(
                        labelText: 'Taille Min (Mo)',
                        labelStyle: TextStyle(color: Colors.white54, fontSize: 12),
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
                      style: const TextStyle(color: Colors.white, fontSize: 13),
                      decoration: const InputDecoration(
                        labelText: 'Taille Max (Mo)',
                        labelStyle: TextStyle(color: Colors.white54, fontSize: 12),
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
      style: const TextStyle(color: Colors.white, fontSize: 14),
      decoration: InputDecoration(
        hintText: _searchTarget == SearchTarget.kad
            ? 'Mots-clés Kad (min. 3 lettres)...'
            : 'Nom de fichier ou mots-clés...',
        hintStyle: const TextStyle(color: Colors.white38, fontSize: 14),
        prefixIcon: const Icon(Icons.search, color: Colors.white38, size: 20),
        suffixIcon: _queryController.text.isNotEmpty
            ? IconButton(
                icon: const Icon(Icons.clear, size: 16, color: Colors.white38),
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
              child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white),
            )
          : const Icon(Icons.search_rounded, size: 18),
      label: Text(vm.isSearching ? 'Recherche...' : 'Rechercher'),
      style: ElevatedButton.styleFrom(
        backgroundColor: AppTheme.primary,
        foregroundColor: Colors.white,
        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
      ),
      onPressed: (isSearchReady && !vm.isSearching && _queryController.text.trim().isNotEmpty)
          ? _handleSearch
          : null,
    );
  }

  // ── Searching State ────────────────────────────────────────────────────────

  Widget _buildSearchingState(BuildContext context) {
    final targetLabel = _searchTarget == SearchTarget.kad ? 'le réseau Kad DHT' : 'le serveur eD2K';
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const CircularProgressIndicator(color: AppTheme.primary),
          const SizedBox(height: 16),
          Text(
            'Recherche en cours sur $targetLabel...',
            style: TextStyle(fontSize: 14, color: Colors.white.withAlpha(180)),
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
    final targetName = _searchTarget == SearchTarget.kad ? 'le réseau Kad DHT' : 'un serveur eD2K';
    final helpText = _searchTarget == SearchTarget.kad
        ? 'Les requêtes KADEMLIA2_SEARCH_KEYWORD interrogent les nœuds Kad stockant l\'index du mot-clé MD4.'
        : 'Les requêtes OP_SEARCHREQUEST interrogent directement l\'index du serveur eD2K connecté.';

    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            hasSearched ? Icons.search_off_rounded : Icons.search_rounded,
            size: 56,
            color: Colors.white24,
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
              color: Colors.white54,
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
              color: Colors.white30,
            ),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }

  // ── Results Table ──────────────────────────────────────────────────────────

  Widget _buildResultsTable(BuildContext context, DownloadsViewModel vm) {
    final results = vm.searchResults;

    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: SingleChildScrollView(
        scrollDirection: Axis.vertical,
        child: DataTable(
          headingRowColor: WidgetStateProperty.all(AppTheme.darkSurface),
          dataRowColor: WidgetStateProperty.resolveWith((states) {
            if (states.contains(WidgetState.hovered)) {
              return AppTheme.primary.withAlpha(15);
            }
            return Colors.transparent;
          }),
          columnSpacing: 14,
          horizontalMargin: 16,
          columns: const [
            DataColumn(
              label: Text(
                'Nom du fichier',
                style: TextStyle(fontWeight: FontWeight.bold, color: Colors.white70),
              ),
            ),
            DataColumn(
              label: Text(
                'Taille',
                style: TextStyle(fontWeight: FontWeight.bold, color: Colors.white70),
              ),
              numeric: true,
            ),
            DataColumn(
              label: Text(
                'Sources',
                style: TextStyle(fontWeight: FontWeight.bold, color: Colors.white70),
              ),
              numeric: true,
            ),
            DataColumn(
              label: Text(
                'Hachage eD2K',
                style: TextStyle(fontWeight: FontWeight.bold, color: Colors.white70),
              ),
            ),
            DataColumn(
              label: Text(
                'Actions',
                style: TextStyle(fontWeight: FontWeight.bold, color: Colors.white70),
              ),
            ),
          ],
          rows: results.map((file) {
            return DataRow(
              cells: [
                DataCell(
                  ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 350),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        const Icon(Icons.insert_drive_file_outlined,
                            size: 16, color: Colors.white54),
                        const SizedBox(width: 8),
                        Flexible(
                          child: Text(
                            file.fileName,
                            style: const TextStyle(
                              color: Colors.white,
                              fontWeight: FontWeight.w500,
                            ),
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
                DataCell(
                  Text(
                    _formatBytes(file.fileSize),
                    style: const TextStyle(color: Colors.white70),
                  ),
                ),
                DataCell(
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                    decoration: BoxDecoration(
                      color: file.sourceCount > 0
                          ? Colors.green.withAlpha(30)
                          : Colors.white10,
                      borderRadius: BorderRadius.circular(4),
                    ),
                    child: Text(
                      '${file.sourceCount}',
                      style: TextStyle(
                        color: file.sourceCount > 0
                            ? Colors.greenAccent
                            : Colors.white38,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                  ),
                ),
                DataCell(
                  SelectableText(
                    file.fileHash,
                    style: const TextStyle(
                      fontFamily: 'monospace',
                      fontSize: 11,
                      color: Colors.white54,
                    ),
                  ),
                ),
                DataCell(
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      ElevatedButton.icon(
                        icon: const Icon(Icons.download_rounded, size: 14),
                        label: const Text('Télécharger', style: TextStyle(fontSize: 12)),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: AppTheme.primary,
                          foregroundColor: Colors.white,
                          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                          visualDensity: VisualDensity.compact,
                        ),
                        onPressed: () {
                          vm.downloadSearchResult(file);
                        },
                      ),
                      const SizedBox(width: 6),
                      IconButton(
                        icon: const Icon(Icons.hub_outlined, size: 16),
                        tooltip: 'Demander sources au serveur',
                        onPressed: () {
                          vm.queryServerSources(file.fileHash, fileSize: file.fileSize);
                        },
                      ),
                      IconButton(
                        icon: const Icon(Icons.copy_rounded, size: 16),
                        tooltip: 'Copier lien eD2K',
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
            );
          }).toList(),
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
