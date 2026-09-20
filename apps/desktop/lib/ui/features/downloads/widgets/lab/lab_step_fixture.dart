import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'lab_step_card.dart';

/// Step 1: pick a fixture type and generate the deterministic test file.
class LabStepFixture extends StatelessWidget {
  final String selectedFixtureType;
  final ValueChanged<String> onFixtureTypeChanged;
  final LabFixtureInfo? lastCreatedFixture;
  final bool isCreating;
  final VoidCallback onCreate;

  const LabStepFixture({
    super.key,
    required this.selectedFixtureType,
    required this.onFixtureTypeChanged,
    required this.lastCreatedFixture,
    required this.isCreating,
    required this.onCreate,
  });

  static const _fixtureTypes = {
    'sample_1mb': '1 MB Sample (sample_1mb)',
    'part_9728000': '9.28 MB Full Part (part_9728000)',
    'pattern_9728001': '9.28 MB + 1 Byte (pattern_9728001)',
    'text': '4 KB Text (text)',
  };

  @override
  Widget build(BuildContext context) {
    final fixture = lastCreatedFixture;
    return LabStepCard(
      stepNumber: 1,
      title: 'Generate Local Test Fixture',
      description:
          'Creates a deterministic file in <workdir>/fixtures/ and computes its eD2K hash.',
      isCompleted: fixture != null,
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Wrap(
            spacing: 8,
            children: [
              for (final entry in _fixtureTypes.entries)
                ChoiceChip(
                  label: Text(entry.value),
                  selected: selectedFixtureType == entry.key,
                  onSelected: (val) {
                    if (val) onFixtureTypeChanged(entry.key);
                  },
                ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              FilledButton.icon(
                icon: isCreating
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(
                          strokeWidth: 2,
                          color: AppColors.textPrimary,
                        ),
                      )
                    : const Icon(Icons.create_new_folder_outlined, size: 16),
                label:
                    Text(isCreating ? 'Generating...' : 'Generate Fixture'),
                onPressed: isCreating ? null : onCreate,
              ),
              if (fixture != null) ...[
                const SizedBox(width: 12),
                const Icon(
                  Icons.check_circle,
                  color: AppTheme.statusCompleted,
                  size: 16,
                ),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(
                    '${fixture.fileName} (${Formatters.formatBytes(fixture.sizeBytes)})',
                    style: const TextStyle(
                      fontSize: 12,
                      fontWeight: FontWeight.bold,
                      color: AppColors.textPrimary,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
            ],
          ),
        ],
      ),
    );
  }
}
