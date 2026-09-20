import 'dart:io';

/// Opens a native folder picker and returns the selected path, or null.
///
/// Desktop-only helper: on Windows it shells out to the WinForms
/// FolderBrowserDialog via PowerShell; returns null on other platforms
/// (mobile/web pickers are handled elsewhere).
Future<String?> pickFolder({required String description}) async {
  if (!Platform.isWindows) return null;
  try {
    final result = await Process.run('powershell', [
      '-NoProfile',
      '-Command',
      'Add-Type -AssemblyName System.Windows.Forms; '
          '\$f = New-Object System.Windows.Forms.FolderBrowserDialog; '
          '\$f.Description = \'$description\'; '
          'if (\$f.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) '
          '{ Write-Output \$f.SelectedPath }',
    ]);
    final chosen = result.stdout.toString().trim();
    if (chosen.isNotEmpty && Directory(chosen).existsSync()) {
      return chosen;
    }
  } catch (_) {}
  return null;
}
