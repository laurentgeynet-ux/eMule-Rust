// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../app/providers.dart';
import '../../../../../l10n/l10n.dart';

/// Interface language picker (FR/EN), persisted via [LocaleController].
class SettingsLanguageSection extends ConsumerWidget {
  const SettingsLanguageSection({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final localeCtl = ref.watch(localeControllerProvider);

    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                context.l10n.language,
                style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.bold,
                  color: AppColors.textPrimary,
                ),
              ),
              const SizedBox(height: 4),
              Text(
                context.l10n.languageDescription,
                style: const TextStyle(
                  fontSize: 11,
                  color: AppColors.textMuted,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(width: 10),
        SegmentedButton<Locale>(
          segments: [
            ButtonSegment(
              value: const Locale('fr'),
              label: Text(context.l10n.french),
            ),
            ButtonSegment(
              value: const Locale('en'),
              label: Text(context.l10n.english),
            ),
          ],
          selected: {localeCtl.locale},
          onSelectionChanged: (set) => localeCtl.setLocale(set.first),
          style: const ButtonStyle(
            visualDensity: VisualDensity.compact,
            tapTargetSize: MaterialTapTargetSize.shrinkWrap,
          ),
        ),
      ],
    );
  }
}
