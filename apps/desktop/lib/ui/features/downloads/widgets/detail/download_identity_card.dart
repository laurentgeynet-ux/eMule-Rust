import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'detail_copy_row.dart';
import 'detail_section_card.dart';

/// Identity card: download UUID, eD2K hash, canonical ed2k link and the
/// category dropdown.
class DownloadIdentityCard extends StatelessWidget {
  final DownloadItem item;
  final List<CategoryItem> categories;
  final ValueChanged<String?> onCategoryChanged;

  const DownloadIdentityCard({
    super.key,
    required this.item,
    required this.categories,
    required this.onCategoryChanged,
  });

  @override
  Widget build(BuildContext context) {
    return DetailSectionCard(
      title: 'Identity & eD2K Links',
      icon: Icons.fingerprint_rounded,
      child: Column(
        children: [
          DetailCopyRow(
            label: 'Download UUID',
            value: item.id,
            monospace: true,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          DetailCopyRow(
            label: 'eD2K Hash (MD4)',
            value: item.ed2kHash ?? 'Pending calculation...',
            monospace: true,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          DetailCopyRow(
            label: 'Canonical eD2K Link',
            value:
                item.ed2kLink ??
                'ed2k://|file|${item.fileName}|${item.totalBytes}|${item.ed2kHash ?? ""}|/',
            monospace: true,
            maxLines: 2,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          Row(
            children: [
              const SizedBox(
                width: 140,
                child: Text(
                  'Category',
                  style: TextStyle(fontSize: 12, color: AppColors.textMuted),
                ),
              ),
              Expanded(
                child: DropdownButtonHideUnderline(
                  child: DropdownButton<String?>(
                    value: item.categoryId,
                    isDense: true,
                    dropdownColor: AppTheme.darkCard,
                    items: [
                      const DropdownMenuItem<String?>(
                        value: null,
                        child: Text(
                          'Sans catégorie (None)',
                          style: TextStyle(
                            color: AppColors.textSecondary,
                            fontSize: 13,
                          ),
                        ),
                      ),
                      ...categories.map(
                        (cat) => DropdownMenuItem<String?>(
                          value: cat.id,
                          child: Row(
                            children: [
                              Container(
                                width: 10,
                                height: 10,
                                decoration: BoxDecoration(
                                  color: cat.parsedColor,
                                  shape: BoxShape.circle,
                                ),
                              ),
                              const SizedBox(width: 8),
                              Text(
                                cat.name,
                                style: const TextStyle(
                                  color: AppColors.textPrimary,
                                  fontSize: 13,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ],
                    onChanged: onCategoryChanged,
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
