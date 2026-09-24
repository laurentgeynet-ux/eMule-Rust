// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';

/// Palette of [NetworkProfileBanner] — alpha-modulated tints for the strict
/// loopback confinement state (emerald) and the public network profile (blue).
abstract final class NetworkProfileColors {
  static const Color loopbackFill = Color(0x2210B981);
  static const Color loopbackBorder = Color(0x6610B981);
  static const Color loopbackAccent = Color(0xFF10B981);

  static const Color exposedFill = Color(0x223B82F6);
  static const Color exposedBorder = Color(0x663B82F6);
  static const Color exposedAccent = Color(0xFF60A5FA);
}
