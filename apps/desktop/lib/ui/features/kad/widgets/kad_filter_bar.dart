import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Contacts filter field + destructive "clear routing table" action.
class KadFilterBar extends StatelessWidget {
  final ValueChanged<String> onQueryChanged;
  final VoidCallback onClearContacts;

  const KadFilterBar({
    super.key,
    required this.onQueryChanged,
    required this.onClearContacts,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
      child: Row(
        children: [
          Expanded(
            child: TextField(
              style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
              decoration: InputDecoration(
                hintText: 'Filtrer par Kad ID, IP ou Port...',
                hintStyle: const TextStyle(fontSize: 13, color: AppColors.textFaint),
                prefixIcon: const Icon(
                  Icons.search_rounded,
                  size: 18,
                  color: AppColors.textFaint,
                ),
                isDense: true,
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 8,
                ),
                filled: true,
                fillColor: AppTheme.darkSurface,
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(6),
                  borderSide: const BorderSide(
                    color: AppTheme.darkBorder,
                    width: 0.5,
                  ),
                ),
                enabledBorder: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(6),
                  borderSide: const BorderSide(
                    color: AppTheme.darkBorder,
                    width: 0.5,
                  ),
                ),
              ),
              onChanged: onQueryChanged,
            ),
          ),
          const SizedBox(width: 12),
          TextButton.icon(
            icon: const Icon(
              Icons.delete_sweep_outlined,
              size: 16,
              color: AppColors.danger,
            ),
            label: const Text(
              'Effacer la table',
              style: TextStyle(color: AppColors.danger, fontSize: 12),
            ),
            onPressed: onClearContacts,
          ),
        ],
      ),
    );
  }
}
