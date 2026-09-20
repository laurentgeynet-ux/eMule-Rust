import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import 'search_target.dart';

/// Spinner shown while a search is in flight and no results have arrived.
class SearchSearchingState extends StatelessWidget {
  final SearchTarget target;

  const SearchSearchingState({super.key, required this.target});

  @override
  Widget build(BuildContext context) {
    final targetLabel = switch (target) {
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
      SearchTarget.kad => 'le réseau Kad DHT',
      SearchTarget.global => 'la liste de serveurs eD2K',
      SearchTarget.server => 'un serveur eD2K',
    };
    final helpText = switch (target) {
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
                ? 'Aucun résultat trouvé pour « $lastTerm »'
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
            style: const TextStyle(fontSize: 12, color: AppColors.iconFaint),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }
}
