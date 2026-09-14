import 'dart:io';
import 'package:flutter/material.dart';
import '../../../core/formatters.dart';
import '../../../core/theme.dart';
import '../view_models/downloads_view_model.dart';

class AddLinkDialog extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const AddLinkDialog({super.key, required this.viewModel});

  @override
  State<AddLinkDialog> createState() => _AddLinkDialogState();
}

class _AddLinkDialogState extends State<AddLinkDialog> {
  final _linkController = TextEditingController();
  late final TextEditingController _targetDirController;
  String _allocationPolicy = 'sparse';

  String? _parsedFileName;
  int? _parsedSize;
  String? _parsedHash;
  String? _parseError;

  @override
  void initState() {
    super.initState();
    _targetDirController = TextEditingController(
      text: widget.viewModel.defaultDownloadDirectory.path,
    );
    _linkController.addListener(_validateLink);
  }

  void _validateLink() {
    final text = _linkController.text.trim();
    if (text.isEmpty) {
      setState(() {
        _parsedFileName = null;
        _parsedSize = null;
        _parsedHash = null;
        _parseError = null;
      });
      return;
    }

    if (!text.startsWith('ed2k://|file|')) {
      setState(() {
        _parsedFileName = null;
        _parsedSize = null;
        _parsedHash = null;
        _parseError = 'Must start with "ed2k://|file|"';
      });
      return;
    }

    final parts = text.split('|');
    if (parts.length < 5) {
      setState(() {
        _parsedFileName = null;
        _parsedSize = null;
        _parsedHash = null;
        _parseError = 'Incomplete eD2K link format';
      });
      return;
    }

    final name = Uri.decodeComponent(parts[2]);
    final size = int.tryParse(parts[3]);
    final hash = parts[4];

    if (name.isEmpty || size == null || size <= 0 || hash.length != 32) {
      setState(() {
        _parsedFileName = null;
        _parsedSize = null;
        _parsedHash = null;
        _parseError = 'Invalid filename, size, or 32-character hex hash';
      });
      return;
    }

    setState(() {
      _parsedFileName = name;
      _parsedSize = size;
      _parsedHash = hash;
      _parseError = null;
    });
  }

  bool get _isValid => _parsedFileName != null && _parsedSize != null && _parseError == null;

  @override
  void dispose() {
    _linkController.dispose();
    _targetDirController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 580),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              // Header
              Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: AppTheme.primary.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: const Icon(
                      Icons.add_link_rounded,
                      color: AppTheme.primary,
                      size: 22,
                    ),
                  ),
                  const SizedBox(width: 12),
                  const Text(
                    'Add eD2K Download',
                    style: TextStyle(
                      fontSize: 18,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  const Spacer(),
                  IconButton(
                    icon: const Icon(Icons.close, size: 20, color: Colors.white54),
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                ],
              ),

              const SizedBox(height: 20),

              // Link input
              const Text(
                'eD2K Link',
                style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: Colors.white70),
              ),
              const SizedBox(height: 6),
              TextField(
                controller: _linkController,
                autofocus: true,
                style: const TextStyle(fontSize: 13, fontFamily: 'monospace'),
                decoration: InputDecoration(
                  hintText: 'ed2k://|file|example.zip|104857600|0123456789abcdef0123456789abcdef|/',
                  hintStyle: const TextStyle(fontSize: 12, color: Colors.white30),
                  errorText: _parseError,
                ),
              ),

              // Parsed Link Preview Card
              if (_isValid) ...[
                const SizedBox(height: 12),
                Container(
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: AppTheme.darkCard,
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(color: AppTheme.primary.withValues(alpha: 0.5)),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          const Icon(Icons.check_circle_outline, color: AppTheme.statusCompleted, size: 16),
                          const SizedBox(width: 6),
                          Expanded(
                            child: Text(
                              _parsedFileName!,
                              style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 13),
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                          Text(
                            Formatters.formatBytes(_parsedSize!),
                            style: const TextStyle(color: AppTheme.statusDownloading, fontWeight: FontWeight.w600, fontSize: 12),
                          ),
                        ],
                      ),
                      const SizedBox(height: 4),
                      Text(
                        'Hash: $_parsedHash',
                        style: const TextStyle(fontSize: 11, color: Colors.white54, fontFamily: 'monospace'),
                      ),
                    ],
                  ),
                ),
              ],

              const SizedBox(height: 16),

              // Target directory & Allocation policy
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(
                    flex: 2,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text(
                          'Target Directory',
                          style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: Colors.white70),
                        ),
                        const SizedBox(height: 6),
                        Row(
                          children: [
                            Expanded(
                              child: TextField(
                                controller: _targetDirController,
                                style: const TextStyle(fontSize: 13),
                                decoration: const InputDecoration(
                                  isDense: true,
                                  hintText: 'Default downloads folder',
                                ),
                              ),
                            ),
                            const SizedBox(width: 8),
                            OutlinedButton.icon(
                              icon: const Icon(Icons.folder_open, size: 16),
                              label: const Text('Browse...', style: TextStyle(fontSize: 12)),
                              style: OutlinedButton.styleFrom(
                                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 12),
                                side: const BorderSide(color: AppTheme.darkBorder),
                              ),
                              onPressed: _browseFolder,
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 16),
                  Expanded(
                    flex: 2,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text(
                          'Disk Allocation',
                          style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: Colors.white70),
                        ),
                        const SizedBox(height: 6),
                        Container(
                          padding: const EdgeInsets.symmetric(horizontal: 12),
                          decoration: BoxDecoration(
                            color: AppTheme.darkSurface,
                            borderRadius: BorderRadius.circular(8),
                            border: Border.all(color: AppTheme.darkBorder),
                          ),
                          child: DropdownButtonHideUnderline(
                            child: DropdownButton<String>(
                              value: _allocationPolicy,
                              isExpanded: true,
                              dropdownColor: AppTheme.darkCard,
                              items: const [
                                DropdownMenuItem(
                                  value: 'sparse',
                                  child: Text('Sparse (Instant, grows dynamically)', style: TextStyle(fontSize: 12)),
                                ),
                                DropdownMenuItem(
                                  value: 'preallocate',
                                  child: Text('Preallocate (Full size, avoids fragmentation)', style: TextStyle(fontSize: 12)),
                                ),
                                DropdownMenuItem(
                                  value: 'none',
                                  child: Text('None (Minimal initial disk use)', style: TextStyle(fontSize: 12)),
                                ),
                              ],
                              onChanged: (val) {
                                if (val != null) {
                                  setState(() => _allocationPolicy = val);
                                }
                              },
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),

              const SizedBox(height: 24),

              // Action buttons
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: const Text('Cancel', style: TextStyle(color: Colors.white60)),
                  ),
                  const SizedBox(width: 12),
                  FilledButton.icon(
                    icon: const Icon(Icons.add_rounded, size: 18),
                    label: const Text('Add Download'),
                    style: FilledButton.styleFrom(backgroundColor: AppTheme.primary),
                    onPressed: _isValid ? _submit : null,
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _browseFolder() async {
    if (Platform.isWindows) {
      try {
        final result = await Process.run('powershell', [
          '-NoProfile',
          '-Command',
          r'''Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.FolderBrowserDialog; $f.Description = 'Select Download Directory'; if ($f.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $f.SelectedPath }'''
        ]);
        final chosen = result.stdout.toString().trim();
        if (chosen.isNotEmpty && Directory(chosen).existsSync()) {
          setState(() {
            _targetDirController.text = chosen;
          });
        }
      } catch (_) {}
    }
  }

  void _submit() async {
    final link = _linkController.text.trim();
    final targetDir = _targetDirController.text.trim();

    final destination = targetDir.isNotEmpty
        ? targetDir
        : widget.viewModel.defaultDownloadDirectory.path;

    final success = await widget.viewModel.addEd2kLink(
      link,
      targetDirectory: destination,
      allocationPolicy: _allocationPolicy,
    );

    if (success && mounted) {
      Navigator.of(context).pop();
    }
  }
}
