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
    return const Ed2kLinkParseResult.error(
      'Must start with "ed2k://|file|"',
    );
  }

  final parts = text.split('|');
  if (parts.length < 5) {
    return const Ed2kLinkParseResult.error(
      'Incomplete eD2K link format',
    );
  }

  final name = Uri.decodeComponent(parts[2]);
  final size = int.tryParse(parts[3]);
  final hash = parts[4];

  if (name.isEmpty || size == null || size <= 0 || hash.length != 32) {
    return const Ed2kLinkParseResult.error(
      'Invalid filename, size, or 32-character hex hash',
    );
  }

  return Ed2kLinkParseResult.valid(
    fileName: name,
    size: size,
    hash: hash,
  );
}
