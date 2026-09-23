// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';

/// Small category tag shown next to the file name on download rows.
/// In compact mode the leading color dot is omitted.
class CategoryBadge extends StatelessWidget {
  final CategoryItem category;
  final bool compact;

  const CategoryBadge({
    super.key,
    required this.category,
    this.compact = false,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: EdgeInsets.symmetric(
        horizontal: compact ? 5 : 6,
        vertical: 1,
      ),
      decoration: BoxDecoration(
        color: category.parsedColor.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(4),
        border: Border.all(
          color: category.parsedColor.withValues(alpha: 0.4),
          width: 0.8,
        ),
      ),
      child: compact
          ? Text(
              category.name,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontSize: 9,
                fontWeight: FontWeight.w600,
                color: category.parsedColor,
              ),
            )
          : Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 6,
                  height: 6,
                  decoration: BoxDecoration(
                    color: category.parsedColor,
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: 4),
                Text(
                  category.name,
                  style: TextStyle(
                    fontSize: 10,
                    fontWeight: FontWeight.w600,
                    color: category.parsedColor,
                  ),
                ),
              ],
            ),
    );
  }
}
