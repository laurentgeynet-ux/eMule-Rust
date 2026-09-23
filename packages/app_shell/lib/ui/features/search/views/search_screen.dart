// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/providers.dart';
import '../../../../app/shell_controller.dart';
import '../../downloads/view_models/downloads_store_controller.dart';
import '../../../../app/app_controller.dart';
import '../view_models/search_controller.dart' as search_ctl;
import '../widgets/search_connection_banner.dart';
import '../widgets/search_form.dart';
import '../widgets/search_results_table.dart';
import '../widgets/search_states.dart';
import '../widgets/search_target.dart';
import '../widgets/search_toolbar.dart';

/// Search section (eD2K server / global UDP / Kad DHT) — composition only.
///
/// Domain state comes from [searchControllerProvider],
/// [kadControllerProvider], [serversControllerProvider] and
/// [downloadsStoreControllerProvider]; navigation goes through
/// [shellControllerProvider].
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

    final search = ref.read(searchControllerProvider);
    if (_searchTarget == SearchTarget.kad) {
      search.searchKadKeywords(
        term: term,
        minSize: minBytes,
        maxSize: maxBytes,
      );
    } else {
      search.searchServer(
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
    final search = ref.watch(searchControllerProvider);
    final kad = ref.watch(kadControllerProvider);
    final serversCtl = ref.watch(serversControllerProvider);
    final downloadsCtl = ref.watch(downloadsStoreControllerProvider);
    final shell = ref.watch(shellControllerProvider);

    final session = serversCtl.session;
    final isConnected =
        session.status == ServerConnectionStatus.connected;
    final isKadActive = kad.status.isUdpActive;
    final isSearchReady = switch (_searchTarget) {
      SearchTarget.kad => isKadActive,
      // Global UDP search only needs a populated server list.
      SearchTarget.global => true,
      SearchTarget.server => isConnected,
    };

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SearchToolbar(
          resultCount: search.results.length,
          onClear: search.clearResults,
        ),
        SearchConnectionBanner(
          target: _searchTarget,
          session: session,
          kadStatus: kad.status,
          isConnected: isConnected,
          onGoToServers: () =>
              ref.read(shellControllerProvider).setSection(AppNavSection.servers),
          onGoToKad: () =>
              ref.read(shellControllerProvider).setSection(AppNavSection.kad),
        ),
        SearchForm(
          queryController: _queryController,
          minSizeController: _minSizeController,
          maxSizeController: _maxSizeController,
          extController: _extController,
          target: _searchTarget,
          showFilters: _showFilters,
          isSearchReady: isSearchReady,
          isSearching: search.isSearching,
          onTargetChanged: (t) => setState(() => _searchTarget = t),
          onToggleFilters: () =>
              setState(() => _showFilters = !_showFilters),
          onSubmit: _handleSearch,
          onClearFilters: () => setState(() {
            _extController.clear();
            _minSizeController.clear();
            _maxSizeController.clear();
          }),
          onQueryChanged: () => setState(() {}),
        ),
        const Divider(color: AppTheme.darkBorder, height: 1),
        Expanded(
          child: search.isSearching && search.results.isEmpty
              ? SearchSearchingState(target: _searchTarget)
              : search.results.isEmpty
              ? SearchEmptyState(
                  target: _searchTarget,
                  lastTerm: search.lastTerm,
                  isSearchReady: isSearchReady,
                )
              : _buildResults(downloadsCtl, search, shell),
        ),
      ],
    );
  }

  Widget _buildResults(
    DownloadsStoreController downloadsCtl,
    search_ctl.SearchController search,
    ShellController shell,
  ) {
    final results = _getSortedResults(search.results, search.epoch);
    final downloadsByHash = <String, DownloadItem>{};
    for (final d in downloadsCtl.allDownloads) {
      if (d.ed2kHash != null) {
        downloadsByHash[d.ed2kHash!.toLowerCase()] = d;
      }
    }

    return SearchResultsTable(
      scrollController: _scrollController,
      results: results,
      downloadsByHash: downloadsByHash,
      sortColumnIndex: _sortColumnIndex,
      sortAscending: _sortAscending,
      onSort: (columnIndex, ascending) => setState(() {
        _sortColumnIndex = columnIndex;
        _sortAscending = ascending;
      }),
      onDownload: (file) => ref
          .read(downloadsStoreControllerProvider)
          .addEd2kLink(file.ed2kLink),
      onGoToDownloads: () =>
          shell.setSection(AppNavSection.downloads),
      onQuerySources: (file) => ref
          .read(searchControllerProvider)
          .querySources(file.fileHash, fileSize: file.fileSize),
      lastError: () => ref.read(shellControllerProvider).errorMessage,
    );
  }
}
