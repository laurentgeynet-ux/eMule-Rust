// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'detail_copy_row.dart';
import 'detail_section_card.dart';
import '../../../../../l10n/l10n.dart';

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
      title: context.l10n.identityEd2kLinks,
      icon: Icons.fingerprint_rounded,
      child: Column(
        children: [
          DetailCopyRow(
            label: context.l10n.downloadUuid,
            value: item.id,
            monospace: true,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          DetailCopyRow(
            label: context.l10n.ed2kHashMd4,
            value: item.ed2kHash ?? context.l10n.pendingCalculation,
            monospace: true,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          DetailCopyRow(
            label: context.l10n.canonicalEd2kLink,
            value:
                item.ed2kLink ??
                'ed2k://|file|${item.fileName}|${item.totalBytes}|${item.ed2kHash ?? ""}|/',
            monospace: true,
            maxLines: 2,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          Row(
            children: [
              SizedBox(
                width: 140,
                child: Text(context.l10n.category,
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
                      DropdownMenuItem<String?>(
                        value: null,
                        child: Text(context.l10n.sansCategorieNone,
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
