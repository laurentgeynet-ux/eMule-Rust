// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:ui_kit/ui_kit.dart';

import 'search_target.dart';
import '../../../../l10n/l10n.dart';
import '../../../../l10n/l10n_lookup.dart';

/// Search form: target selector (server / global UDP / Kad), keyword field,
/// search button and the collapsible advanced filters row (extension,
/// min/max size in MB).
///
/// Text controllers are owned by the parent view; this widget is purely
/// presentational.
class SearchForm extends StatelessWidget {
  final TextEditingController queryController;
  final TextEditingController minSizeController;
  final TextEditingController maxSizeController;
  final TextEditingController extController;
  final SearchTarget target;
  final bool showFilters;
  final bool isSearchReady;
  final bool isSearching;
  final ValueChanged<SearchTarget> onTargetChanged;
  final VoidCallback onToggleFilters;
  final VoidCallback onSubmit;
  final VoidCallback onClearFilters;
  final VoidCallback onQueryChanged;

  const SearchForm({
    super.key,
    required this.queryController,
    required this.minSizeController,
    required this.maxSizeController,
    required this.extController,
    required this.target,
    required this.showFilters,
    required this.isSearchReady,
    required this.isSearching,
    required this.onTargetChanged,
    required this.onToggleFilters,
    required this.onSubmit,
    required this.onClearFilters,
    required this.onQueryChanged,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isStacked = constraints.maxWidth < 750;

          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (isStacked) ...[
                Row(
                  children: [
                    Flexible(
                      child: SingleChildScrollView(
                        scrollDirection: Axis.horizontal,
                        child: _TargetSelector(
                          target: target,
                          fontSize: 12,
                          onChanged: onTargetChanged,
                        ),
                      ),
                    ),
                    _filterToggle(),
                  ],
                ),
                const SizedBox(height: 10),
                Row(
                  children: [
                    Expanded(child: _queryField()),
                    const SizedBox(width: 8),
                    _searchButton(),
                  ],
                ),
              ] else ...[
                Row(
                  children: [
                    _TargetSelector(
                      target: target,
                      fontSize: 13,
                      onChanged: onTargetChanged,
                    ),
                    const SizedBox(width: 8),
                    Expanded(child: _queryField()),
                    const SizedBox(width: 8),
                    _searchButton(),
                    _filterToggle(),
                  ],
                ),
              ],
              if (showFilters) ...[
                const SizedBox(height: 10),
                _filtersPanel(context),
              ],
            ],
          );
        },
      ),
    );
  }

  Widget _filterToggle() {
    return IconButton(
      visualDensity: VisualDensity.compact,
      icon: Icon(
        showFilters ? Icons.filter_alt_off_rounded : Icons.filter_alt_rounded,
        color: showFilters ? AppTheme.primary : AppColors.textMuted,
      ),
      tooltip: showFilters
          ? L10nLookup.current.searchHideFilters
          : L10nLookup.current.searchShowFilters,
      onPressed: onToggleFilters,
    );
  }

  Widget _queryField() {
    return TextField(
      controller: queryController,
      enabled: isSearchReady && !isSearching,
      style: const TextStyle(color: AppColors.textPrimary, fontSize: 14),
      decoration: InputDecoration(
        hintText: target == SearchTarget.kad
            ? L10nLookup.current.searchHintKad
            : L10nLookup.current.searchHintServer,
        hintStyle: const TextStyle(color: AppColors.textFaint, fontSize: 14),
        prefixIcon: const Icon(
          Icons.search,
          color: AppColors.textFaint,
          size: 20,
        ),
        suffixIcon: queryController.text.isNotEmpty
            ? IconButton(
                icon: const Icon(
                  Icons.clear,
                  size: 16,
                  color: AppColors.textFaint,
                ),
                onPressed: () {
                  queryController.clear();
                  onQueryChanged();
                },
              )
            : null,
        filled: true,
        fillColor: AppTheme.darkSurface,
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 14,
          vertical: 10,
        ),
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
      onSubmitted: (_) => onSubmit(),
      onChanged: (_) => onQueryChanged(),
    );
  }

  Widget _searchButton() {
    return ElevatedButton.icon(
      icon: isSearching
          ? const SizedBox(
              width: 16,
              height: 16,
              child: CircularProgressIndicator(
                strokeWidth: 2,
                color: AppColors.textPrimary,
              ),
            )
          : const Icon(Icons.search_rounded, size: 18),
      label: Text(
        isSearching
            ? L10nLookup.current.searchButtonSearching
            : L10nLookup.current.searchButton,
      ),
      style: ElevatedButton.styleFrom(
        backgroundColor: AppTheme.primary,
        foregroundColor: AppColors.textPrimary,
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
      ),
      onPressed:
          (isSearchReady &&
              !isSearching &&
              queryController.text.trim().isNotEmpty)
          ? onSubmit
          : null,
    );
  }

  Widget _filtersPanel(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppTheme.darkSurface.withAlpha(150),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder),
      ),
      child: Row(
        children: [
          if (target != SearchTarget.kad) ...[
            Expanded(
              flex: 2,
              child: _filterField(
                controller: extController,
                label: context.l10n.extensionExIsoZipAvi,
              ),
            ),
            const SizedBox(width: 12),
          ],
          Expanded(
            child: _filterField(
              controller: minSizeController,
              label: context.l10n.tailleMinMo,
              numeric: true,
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: _filterField(
              controller: maxSizeController,
              label: context.l10n.tailleMaxMo,
              numeric: true,
            ),
          ),
          const SizedBox(width: 12),
          TextButton.icon(
            icon: const Icon(Icons.clear_rounded, size: 14),
            label: Text(context.l10n.effacerFiltres, style: TextStyle(fontSize: 12)),
            onPressed: onClearFilters,
          ),
        ],
      ),
    );
  }

  Widget _filterField({
    required TextEditingController controller,
    required String label,
    bool numeric = false,
  }) {
    return TextField(
      controller: controller,
      keyboardType: numeric ? TextInputType.number : null,
      inputFormatters: numeric
          ? [FilteringTextInputFormatter.digitsOnly]
          : null,
      style: const TextStyle(color: AppColors.textPrimary, fontSize: 13),
      decoration: InputDecoration(
        labelText: label,
        labelStyle: const TextStyle(
          color: AppColors.textMuted,
          fontSize: 12,
        ),
        isDense: true,
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 10,
          vertical: 8,
        ),
        border: const OutlineInputBorder(),
      ),
    );
  }
}

class _TargetSelector extends StatelessWidget {
  final SearchTarget target;
  final double fontSize;
  final ValueChanged<SearchTarget> onChanged;

  const _TargetSelector({
    required this.target,
    required this.fontSize,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return SegmentedButton<SearchTarget>(
      segments: [
        ButtonSegment(
          value: SearchTarget.server,
          icon: const Icon(Icons.dns_rounded, size: 16),
          label: Text(context.l10n.serveurEd2k, style: TextStyle(fontSize: fontSize)),
        ),
        ButtonSegment(
          value: SearchTarget.global,
          icon: const Icon(Icons.public_rounded, size: 16),
          label: Text(context.l10n.globalUdp, style: TextStyle(fontSize: fontSize)),
        ),
        ButtonSegment(
          value: SearchTarget.kad,
          icon: const Icon(Icons.hub_rounded, size: 16),
          label: Text(
            context.l10n.searchTargetKad,
            style: TextStyle(fontSize: fontSize),
          ),
        ),
      ],
      selected: {target},
      onSelectionChanged: (set) => onChanged(set.first),
      style: const ButtonStyle(
        visualDensity: VisualDensity.compact,
        tapTargetSize: MaterialTapTargetSize.shrinkWrap,
      ),
    );
  }
}
