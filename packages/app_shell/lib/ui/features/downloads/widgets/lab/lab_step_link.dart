// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'lab_step_card.dart';
import '../../../../../l10n/l10n.dart';

/// Step 2: display/copy the fixture's canonical ed2k link and add it as a
/// tracked download task.
class LabStepLink extends StatelessWidget {
  final LabFixtureInfo? fixture;
  final bool isAdding;
  final bool addedSuccess;
  final VoidCallback onAdd;

  const LabStepLink({
    super.key,
    required this.fixture,
    required this.isAdding,
    required this.addedSuccess,
    required this.onAdd,
  });

  @override
  Widget build(BuildContext context) {
    final fixture = this.fixture;
    return LabStepCard(
      stepNumber: 2,
      title: context.l10n.obtainCanonicalEd2kLinkAdd,
      description: context.l10n.labStep2Desc,
      isCompleted: addedSuccess,
      content: fixture != null
          ? Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Container(
                  padding: const EdgeInsets.all(10),
                  decoration: BoxDecoration(
                    color: AppTheme.darkSurface,
                    borderRadius: BorderRadius.circular(6),
                    border: Border.all(
                      color: AppTheme.darkBorder,
                      width: 0.5,
                    ),
                  ),
                  child: Row(
                    children: [
                      Expanded(
                        child: SelectableText(
                          fixture.ed2kLink,
                          style: const TextStyle(
                            fontSize: 11,
                            fontFamily: 'monospace',
                            color: AppColors.textSecondary,
                          ),
                        ),
                      ),
                      IconButton(
                        icon: const Icon(
                          Icons.copy,
                          size: 16,
                          color: AppColors.textSecondary,
                        ),
                        tooltip: context.l10n.copyLink,
                        onPressed: () {
                          Clipboard.setData(
                            ClipboardData(text: fixture.ed2kLink),
                          );
                        },
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 10),
                Wrap(
                  spacing: 10,
                  runSpacing: 8,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    FilledButton.tonalIcon(
                      icon: isAdding
                          ? const SizedBox(
                              width: 14,
                              height: 14,
                              child: CircularProgressIndicator(
                                strokeWidth: 2,
                              ),
                            )
                          : const Icon(Icons.download, size: 16),
                      label: Text(
                        addedSuccess
                            ? context.l10n.labAddedToDownloads
                            : context.l10n.labAddToDownloads,
                      ),
                      onPressed: isAdding ? null : onAdd,
                    ),
                    if (addedSuccess)
                      Text(context.l10n.downloadTaskCreatedInUi,
                        style: TextStyle(
                          color: AppTheme.statusCompleted,
                          fontSize: 12,
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                  ],
                ),
              ],
            )
          : Text(context.l10n.completeStep1ToGenerate,
              style: TextStyle(fontSize: 12, color: AppColors.textFaint),
            ),
    );
  }
}
