import 'package:flutter/material.dart';

import 'theme.dart';

/// Design tokens — single source of truth for spacing, radii, text styles,
/// breakpoints and semantic colors. Views must use these tokens instead of
/// hardcoded values (no raw `Colors.*`, `EdgeInsets` literals for layout
/// rhythm, or ad-hoc `TextStyle`s).
abstract final class AppTokens {
  AppTokens._();
}

/// Layout breakpoints (logical pixels).
abstract final class AppBreakpoints {
  /// Below this width the navigation becomes a drawer / bottom nav.
  static const double compact = 750;

  /// Between [compact] and this width the sidebar is a compact rail.
  static const double medium = 1100;
}

/// Spacing scale (multiples of 4).
abstract final class AppSpacing {
  static const double xxs = 2;
  static const double xs = 4;
  static const double sm = 8;
  static const double md = 12;
  static const double lg = 16;
  static const double xl = 20;
  static const double xxl = 24;
  static const double xxxl = 32;

  static const EdgeInsets pagePadding = EdgeInsets.all(lg);
  static const EdgeInsets toolbarPadding =
      EdgeInsets.symmetric(horizontal: xl, vertical: md + 2);
  static const EdgeInsets sectionPadding =
      EdgeInsets.symmetric(horizontal: xl, vertical: md);
}

/// Border radius scale.
abstract final class AppRadius {
  static const double sm = 6;
  static const double md = 8;
  static const double lg = 10;

  static BorderRadius get smAll => BorderRadius.circular(sm);
  static BorderRadius get mdAll => BorderRadius.circular(md);
  static BorderRadius get lgAll => BorderRadius.circular(lg);
}

/// Semantic colors on top of the base palette in [AppTheme].
/// Prefer these over raw `Colors.*` in views.
abstract final class AppColors {
  // Surfaces & borders (aliases of the base palette)
  static const Color background = AppTheme.darkBackground;
  static const Color surface = AppTheme.darkSurface;
  static const Color card = AppTheme.darkCard;
  static const Color border = AppTheme.darkBorder;

  // Text hierarchy
  static const Color textPrimary = Colors.white;
  static const Color textSecondary = Colors.white70;
  static const Color textSoft = Colors.white60;
  static const Color textMuted = Colors.white54;
  static const Color textFaint = Colors.white38;

  // Large dimmed icons / decorative glyphs
  static const Color iconFaint = Colors.white30;

  // Subtle fill overlays (badges, hovered rows, chips)
  static const Color fillSubtle = Colors.white10;
  static const Color fillLight = Colors.white12;
  static const Color fillMedium = Colors.white24;

  // Brand & accents
  static const Color accent = AppTheme.primary;
  static const Color accentTeal = Colors.tealAccent;
  static const Color accentCyan = Colors.cyanAccent;
  static const Color accentAmber = Colors.amberAccent;

  // Semantic statuses
  static const Color success = Colors.tealAccent;
  static const Color warning = Colors.amber;
  static const Color danger = Colors.redAccent;
  static const Color info = AppTheme.primary;
  static const Color positive = Colors.greenAccent;

  // Secondary accent hues (category/status accents in views)
  static const Color accentOrange = Colors.orangeAccent;
  static const Color accentBlue = Colors.blueAccent;
  static const Color accentPurple = Colors.purpleAccent;
  static const Color accentLightBlue = Colors.lightBlueAccent;
  static const Color lab = Colors.teal;
  static const Color labShade700 = Color(0xFF00796B); // Colors.teal.shade700
  static const Color labDim = Colors.blueGrey;

  // Base hues for alpha-modulated tints (banners, borders, fills)
  static const Color tintRed = Colors.red;
  static const Color tintGreen = Colors.green;
  static const Color tintBlue = Colors.blue;
  static const Color tintCyan = Colors.cyan;
  static const Color tintPurple = Colors.purple;
  static const Color tintGrey = Colors.grey;
  static const Color warningShade800 = Color(0xFFFF8F00); // Colors.amber.shade800
  static const Color warningShade700 = Color(0xFFFFA000); // Colors.amber.shade700
  static const Color accentOrangeShade700 = Color(0xFFF57C00); // Colors.orange.shade700
  static const Color positiveShade700 = Color(0xFF388E3C); // Colors.green.shade700
  static const Color dangerShade700 = Color(0xFFD32F2F); // Colors.red.shade700
}

/// Text style tokens for the compact dense UI.
abstract final class AppTextStyles {
  static const TextStyle sectionTitle = TextStyle(
    fontSize: 16,
    fontWeight: FontWeight.bold,
    color: AppColors.textPrimary,
  );

  static const TextStyle toolbarLabel = TextStyle(
    fontSize: 11,
    fontWeight: FontWeight.w500,
    color: AppColors.textSecondary,
  );

  static const TextStyle badgeLabel = TextStyle(
    fontSize: 11,
    fontWeight: FontWeight.w600,
    color: AppColors.textSecondary,
  );

  static const TextStyle metricLabel = TextStyle(
    fontSize: 11,
    color: AppColors.textFaint,
  );

  static const TextStyle metricValue = TextStyle(
    fontSize: 12,
    fontWeight: FontWeight.w600,
    color: AppColors.textPrimary,
  );

  static const TextStyle bodySmall = TextStyle(
    fontSize: 13,
    color: AppColors.textSecondary,
  );
}

/// Convenience decorations for recurring UI patterns.
abstract final class AppDecorations {
  /// Thin bottom separator used under toolbars/filter bars.
  static const BoxDecoration headerBar = BoxDecoration(
    color: AppColors.surface,
    border: Border(
      bottom: BorderSide(color: AppColors.border, width: 0.5),
    ),
  );

  /// Thin top separator (details panel, footers).
  static const BoxDecoration footerBar = BoxDecoration(
    color: AppColors.surface,
    border: Border(
      top: BorderSide(color: AppColors.border, width: 0.5),
    ),
  );

  /// Neutral chip/badge container.
  static BoxDecoration chip({Color? color}) => BoxDecoration(
        color: color ?? AppColors.card,
        borderRadius: AppRadius.mdAll,
        border: Border.all(color: AppColors.border, width: 0.5),
      );
}
