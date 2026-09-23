// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import 'search_target.dart';
import '../../../../l10n/l10n.dart';

/// Spinner shown while a search is in flight and no results have arrived.
class SearchSearchingState extends StatelessWidget {
  final SearchTarget target;

  const SearchSearchingState({super.key, required this.target});

  @override
  Widget build(BuildContext context) {
    final targetLabel = switch (target) {
      SearchTarget.kad => context.l10n.searchTargetLabelKad,
      SearchTarget.global => context.l10n.searchTargetLabelGlobal,
      SearchTarget.server => context.l10n.searchTargetLabelServer,
    };
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const CircularProgressIndicator(color: AppTheme.primary),
          const SizedBox(height: 16),
          Text(context.l10n.rechercheEnCoursSur(targetLabel),
            style: TextStyle(
              fontSize: 14,
              color: AppColors.textPrimary.withAlpha(180),
            ),
          ),
        ],
      ),
    );
  }
}

/// Empty results placeholder: invite to search, "no result" after a query,
/// or a hint to activate the required transport.
class SearchEmptyState extends StatelessWidget {
  final SearchTarget target;
  final String? lastTerm;
  final bool isSearchReady;

  const SearchEmptyState({
    super.key,
    required this.target,
    required this.lastTerm,
    required this.isSearchReady,
  });

  @override
  Widget build(BuildContext context) {
    final hasSearched = lastTerm != null;
    final targetName = switch (target) {
      SearchTarget.kad => context.l10n.searchTargetLabelKad,
      SearchTarget.global => context.l10n.searchTargetEnableGlobal,
      SearchTarget.server => context.l10n.searchTargetEnableServer,
    };
    final helpText = switch (target) {
      SearchTarget.kad => context.l10n.searchExplainKad,
      SearchTarget.global => context.l10n.searchExplainGlobal,
      SearchTarget.server => context.l10n.searchExplainServer,
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
                ? context.l10n.searchNoResults(lastTerm!)
                : isSearchReady
                ? context.l10n.searchPromptKeywords
                : context.l10n.searchEnableTarget(targetName),
            style: const TextStyle(
              fontSize: 15,
              fontWeight: FontWeight.w500,
              color: AppColors.textMuted,
            ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 6),
          Text(
            hasSearched ? context.l10n.searchTryOther : helpText,
            style: const TextStyle(fontSize: 12, color: AppColors.iconFaint),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }
}
