// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

// Kad-specific display helpers (truncated Kad IDs, relative timestamps).

import '../../../../l10n/l10n_lookup.dart';

/// `abcd1234…wxyz9876` for long Kad IDs.
String formatTruncatedKadId(String id) {
  if (id.length <= 16) return id;
  return '${id.substring(0, 8)}...${id.substring(id.length - 8)}';
}

/// Human "last seen" for a unix-ms timestamp.
String formatKadLastSeen(int unixMs) {
  if (unixMs <= 0) return L10nLookup.current.kadTimeNever;
  final dt = DateTime.fromMillisecondsSinceEpoch(unixMs);
  final diff = DateTime.now().difference(dt);
  if (diff.inMinutes < 1) return L10nLookup.current.kadTimeJustNow;
  if (diff.inHours < 1) {
    return L10nLookup.current.kadTimeMinutesAgo(diff.inMinutes);
  }
  if (diff.inDays < 1) {
    return L10nLookup.current.kadTimeHoursAgo(diff.inHours);
  }
  final day = dt.day.toString().padLeft(2, '0');
  final month = dt.month.toString().padLeft(2, '0');
  final hour = dt.hour.toString().padLeft(2, '0');
  final minute = dt.minute.toString().padLeft(2, '0');
  return '$day/$month $hour:$minute';
}
