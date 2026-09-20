import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Disk allocation policy picker (sparse / preallocate / none).
class AddLinkAllocationDropdown extends StatelessWidget {
  final String value;
  final ValueChanged<String> onChanged;

  const AddLinkAllocationDropdown({
    super.key,
    required this.value,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12),
      decoration: BoxDecoration(
        color: AppTheme.darkSurface,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder),
      ),
      child: DropdownButtonHideUnderline(
        child: DropdownButton<String>(
          value: value,
          isExpanded: true,
          dropdownColor: AppTheme.darkCard,
          items: const [
            DropdownMenuItem(
              value: 'sparse',
              child: Text(
                'Sparse (Instant, grows dynamically)',
                style: TextStyle(fontSize: 12),
              ),
            ),
            DropdownMenuItem(
              value: 'preallocate',
              child: Text(
                'Preallocate (Full size, avoids fragmentation)',
                style: TextStyle(fontSize: 12),
              ),
            ),
            DropdownMenuItem(
              value: 'none',
              child: Text(
                'None (Minimal initial disk use)',
                style: TextStyle(fontSize: 12),
              ),
            ),
          ],
          onChanged: (val) {
            if (val != null) onChanged(val);
          },
        ),
      ),
    );
  }
}
