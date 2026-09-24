// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';
import 'sections/advanced_section.dart';
import 'sections/connection_section.dart';
import 'sections/general_section.dart';
import 'sections/security_section.dart';
import 'sections/transfers_section.dart';
import '../../downloads/widgets/settings/settings_friends_section.dart';

/// Full-page settings view (eMule-style "Préférences"): a left section
/// rail and a scrollable content pane. Replaces the old catch-all dialog.
class SettingsView extends StatefulWidget {
  /// Deep-linkable initial section (e.g. `/settings?section=security`).
  final String? initialSection;

  const SettingsView({super.key, this.initialSection});

  @override
  State<SettingsView> createState() => _SettingsViewState();
}

enum _SettingsSection { general, connection, transfers, security, friends, advanced }

class _SettingsViewState extends State<SettingsView> {
  late _SettingsSection _section = _SettingsSection.values.firstWhere(
    (s) => s.name == widget.initialSection,
    orElse: () => _SettingsSection.general,
  );
  final _scrollController = ScrollController();

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Container(
          width: 200,
          decoration: const BoxDecoration(
            color: AppTheme.darkSurface,
            border: Border(
              right: BorderSide(color: AppTheme.darkBorder, width: 0.5),
            ),
          ),
          child: ListView(
            padding: const EdgeInsets.symmetric(vertical: 12),
            children: [
              for (final s in _SettingsSection.values)
                _RailItem(
                  icon: _iconFor(s),
                  label: _labelFor(context, s),
                  selected: _section == s,
                  onTap: () => setState(() => _section = s),
                ),
            ],
          ),
        ),
        Expanded(
          child: Scrollbar(
            controller: _scrollController,
            thumbVisibility: true,
            child: SingleChildScrollView(
              controller: _scrollController,
              padding: const EdgeInsets.all(24),
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 760),
                child: switch (_section) {
                  _SettingsSection.general => const SettingsGeneralSection(),
                  _SettingsSection.connection =>
                    const SettingsConnectionSection(),
                  _SettingsSection.transfers => const SettingsTransfersSection(),
                  _SettingsSection.security => const SettingsSecuritySection(),
                  _SettingsSection.friends => const SettingsFriendsSection(),
                  _SettingsSection.advanced => const SettingsAdvancedSection(),
                },
              ),
            ),
          ),
        ),
      ],
    );
  }

  static IconData _iconFor(_SettingsSection s) => switch (s) {
        _SettingsSection.general => Icons.tune_rounded,
        _SettingsSection.connection => Icons.settings_ethernet_rounded,
        _SettingsSection.transfers => Icons.swap_vert_rounded,
        _SettingsSection.security => Icons.security_rounded,
        _SettingsSection.friends => Icons.people_alt_outlined,
        _SettingsSection.advanced => Icons.build_circle_outlined,
      };

  static String _labelFor(BuildContext context, _SettingsSection s) =>
      switch (s) {
        _SettingsSection.general => context.l10n.settingsSectionGeneral,
        _SettingsSection.connection => context.l10n.settingsSectionConnection,
        _SettingsSection.transfers => context.l10n.settingsSectionTransfers,
        _SettingsSection.security => context.l10n.settingsSectionSecurity,
        _SettingsSection.friends => context.l10n.settingsSectionFriends,
        _SettingsSection.advanced => context.l10n.settingsSectionAdvanced,
      };
}

class _RailItem extends StatelessWidget {
  final IconData icon;
  final String label;
  final bool selected;
  final VoidCallback onTap;

  const _RailItem({
    required this.icon,
    required this.label,
    required this.selected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final color = selected ? AppTheme.primary : AppColors.textSecondary;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      child: Material(
        color: selected
            ? AppTheme.primary.withValues(alpha: 0.12)
            : AppColors.transparent,
        borderRadius: BorderRadius.circular(6),
        child: InkWell(
          borderRadius: BorderRadius.circular(6),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            child: Row(
              children: [
                Icon(icon, size: 18, color: color),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    label,
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight:
                          selected ? FontWeight.w600 : FontWeight.w400,
                      color: color,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
