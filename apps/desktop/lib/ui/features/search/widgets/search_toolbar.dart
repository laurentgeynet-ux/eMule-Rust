import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Search section toolbar: title, result count and "clear results" action.
/// Icon-only buttons below 650 px.
class SearchToolbar extends StatelessWidget {
  final int resultCount;
  final VoidCallback onClear;

  const SearchToolbar({
    super.key,
    required this.resultCount,
    required this.onClear,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
      decoration: const BoxDecoration(
        color: AppTheme.darkSurface,
        border: Border(
          bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isCompact = constraints.maxWidth < 650;

          return Row(
            children: [
              const Icon(
                Icons.search_rounded,
                color: AppTheme.primary,
                size: 22,
              ),
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
              if (resultCount > 0) ...[
                if (!isCompact) ...[
                  Text(
                    '$resultCount résultat(s)',
                    style: const TextStyle(
                      color: AppColors.textMuted,
                      fontSize: 13,
                    ),
                  ),
                  const SizedBox(width: 12),
                  OutlinedButton.icon(
                    icon: const Icon(Icons.clear_all_rounded, size: 16),
                    label: const Text('Effacer les résultats'),
                    onPressed: onClear,
                  ),
                ] else ...[
                  IconButton(
                    tooltip: 'Effacer les résultats ($resultCount)',
                    icon: const Icon(Icons.clear_all_rounded, size: 18),
                    onPressed: onClear,
                  ),
                ],
              ],
            ],
          );
        },
      ),
    );
  }
}
