// Servers feature display helpers.

/// Compact large counts: 1234567 → `1.2M`, 1500 → `1.5k`.
String formatCompactCount(int v) {
  if (v >= 1000000) return '${(v / 1000000).toStringAsFixed(1)}M';
  if (v >= 1000) return '${(v / 1000).toStringAsFixed(1)}k';
  return '$v';
}
