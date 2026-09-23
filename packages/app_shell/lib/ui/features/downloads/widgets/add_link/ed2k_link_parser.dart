// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import '../../../../../l10n/l10n_lookup.dart';

/// Result of parsing a candidate `ed2k://|file|…` link.
class Ed2kLinkParseResult {
  final String? fileName;
  final int? size;
  final String? hash;
  final String? error;

  const Ed2kLinkParseResult._({
    this.fileName,
    this.size,
    this.hash,
    this.error,
  });

  const Ed2kLinkParseResult.empty() : this._();

  const Ed2kLinkParseResult.error(String message)
      : this._(error: message);

  const Ed2kLinkParseResult.valid({
    required String fileName,
    required int size,
    required String hash,
  }) : this._(fileName: fileName, size: size, hash: hash);

  bool get isValid => fileName != null && size != null && error == null;
}

/// Parses an `ed2k://|file|name|size|hash|/` link; returns a result with a
/// human-readable error when invalid.
Ed2kLinkParseResult parseEd2kLink(String raw) {
  final text = raw.trim();
  if (text.isEmpty) return const Ed2kLinkParseResult.empty();

  if (!text.startsWith('ed2k://|file|')) {
    return Ed2kLinkParseResult.error(
      L10nLookup.current.ed2kErrPrefix,
    );
  }

  final parts = text.split('|');
  if (parts.length < 5) {
    return Ed2kLinkParseResult.error(
      L10nLookup.current.ed2kErrIncomplete,
    );
  }

  final name = Uri.decodeComponent(parts[2]);
  final size = int.tryParse(parts[3]);
  final hash = parts[4];

  if (name.isEmpty || size == null || size <= 0 || hash.length != 32) {
    return Ed2kLinkParseResult.error(
      L10nLookup.current.ed2kErrInvalid,
    );
  }

  return Ed2kLinkParseResult.valid(
    fileName: name,
    size: size,
    hash: hash,
  );
}
