// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// Garde-fou design tokens : interdit les littéraux de couleur dans le code
/// UI. Toute couleur doit provenir de `packages/ui_kit/lib/src/tokens.dart`
/// (`AppColors`, `AppTheme`) ou d'une palette de feature `*_colors.dart`
/// (ex. `network_profile_colors.dart`).
///
/// Si le token manque : l'ajouter dans `tokens.dart` (ou `theme.dart` pour la
/// palette de base) — ne pas désactiver ce test. Escape hatch ponctuel :
/// `// design_tokens_guard: ignore` en fin de ligne.
void main() {
  test('aucune couleur en dur dans app_shell / apps lib', () {
    // `flutter test` s'exécute depuis la racine de apps/desktop.
    const roots = <String>[
      '../../packages/app_shell/lib',
      'lib',
    ];

    final forbidden = RegExp(
      r'\bColors\.|\bColor\(0x|\bColor\.fromARGB\b|\bColor\.fromRGBO\b',
    );

    bool isExemptFile(String path) {
      final normalized = path.replaceAll('\\', '/');
      return normalized.endsWith('_colors.dart') ||
          normalized.endsWith('.g.dart') ||
          normalized.endsWith('.freezed.dart') ||
          normalized.contains('/generated/');
    }

    final violations = <String>[];

    for (final root in roots) {
      final dir = Directory(root);
      if (!dir.existsSync()) continue;
      for (final entity in dir.listSync(recursive: true)) {
        if (entity is! File || !entity.path.endsWith('.dart')) continue;
        if (isExemptFile(entity.path)) continue;
        final lines = entity.readAsLinesSync();
        for (var i = 0; i < lines.length; i++) {
          final line = lines[i];
          final trimmed = line.trimLeft();
          if (trimmed.startsWith('//') ||
              trimmed.startsWith('*') ||
              trimmed.startsWith('/*')) {
            continue;
          }
          if (line.contains('design_tokens_guard: ignore')) continue;
          if (forbidden.hasMatch(line)) {
            violations.add('${entity.path}:${i + 1}: ${line.trim()}');
          }
        }
      }
    }

    expect(
      violations,
      isEmpty,
      reason:
          'Littéraux de couleur interdits — utiliser AppColors/AppTheme '
          '(packages/ui_kit/lib/src/tokens.dart) ou une palette *_colors.dart :\n'
          '${violations.join('\n')}',
    );
  });
}
