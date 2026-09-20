import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:ui_kit/ui_kit.dart';
import 'package:mule_client/mule_client.dart';
import '../view_models/downloads_view_model.dart';

class LabGuideDialog extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const LabGuideDialog({super.key, required this.viewModel});

  @override
  State<LabGuideDialog> createState() => _LabGuideDialogState();
}

class _LabGuideDialogState extends State<LabGuideDialog> {
  bool _isLoading = true;
  String? _errorMessage;
  LabStatus? _labStatus;

  // Selected fixture type for generation
  String _selectedFixtureType = 'sample_1mb';
  LabFixtureInfo? _lastCreatedFixture;
  bool _isCreatingFixture = false;
  bool _isAddingToDownloads = false;
  bool _addedToDownloadsSuccess = false;

  // Peer management
  bool _isManagingPeer = false;

  // Cleanup
  bool _isCleaningUp = false;

  // Connect download
  String? _selectedDownloadIdForConnect;
  bool _isConnectingDownload = false;

  @override
  void initState() {
    super.initState();
    _loadStatus();
  }

  Future<void> _loadStatus() async {
    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    try {
      final status = await widget.viewModel.getLabStatus();
      if (mounted) {
        setState(() {
          _labStatus = status;
          if (status.fixtures.isNotEmpty && _lastCreatedFixture == null) {
            _lastCreatedFixture = status.fixtures.last;
          }
          _isLoading = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = e.toString();
          _isLoading = false;
        });
      }
    }
  }

  Future<void> _createFixture() async {
    setState(() {
      _isCreatingFixture = true;
      _errorMessage = null;
    });

    try {
      final fixture = await widget.viewModel.createLabFixture(_selectedFixtureType);
      final updatedStatus = await widget.viewModel.getLabStatus();
      if (mounted) {
        setState(() {
          _lastCreatedFixture = fixture;
          _labStatus = updatedStatus;
          _addedToDownloadsSuccess = false;
          _isCreatingFixture = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = 'Failed to create fixture: $e';
          _isCreatingFixture = false;
        });
      }
    }
  }

  Future<void> _addToDownloads() async {
    final fixture = _lastCreatedFixture;
    if (fixture == null) return;

    setState(() {
      _isAddingToDownloads = true;
      _errorMessage = null;
    });

    try {
      final success = await widget.viewModel.addEd2kLink(fixture.ed2kLink);
      if (mounted) {
        setState(() {
          _addedToDownloadsSuccess = success;
          _isAddingToDownloads = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = 'Failed to add download: $e';
          _isAddingToDownloads = false;
        });
      }
    }
  }

  Future<void> _togglePeer() async {
    final isRunning = _labStatus?.activeLoopbackPeer != null;
    setState(() {
      _isManagingPeer = true;
      _errorMessage = null;
    });

    try {
      if (isRunning) {
        await widget.viewModel.stopLabPeer();
      } else {
        await widget.viewModel.startLabPeer();
      }
      final updatedStatus = await widget.viewModel.getLabStatus();
      if (mounted) {
        setState(() {
          _labStatus = updatedStatus;
          _isManagingPeer = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = 'Peer operation failed: $e';
          _isManagingPeer = false;
        });
      }
    }
  }

  Future<void> _connectDownload() async {
    final peerAddr = _labStatus?.activeLoopbackPeer;
    final downloadId = _selectedDownloadIdForConnect ??
        (widget.viewModel.filteredDownloads.isNotEmpty
            ? widget.viewModel.filteredDownloads.first.id
            : null);

    if (peerAddr == null || downloadId == null) return;

    setState(() {
      _isConnectingDownload = true;
      _errorMessage = null;
    });

    try {
      await widget.viewModel.connectDownloadToSource(downloadId, peerAddr);
      if (mounted) {
        setState(() {
          _isConnectingDownload = false;
        });
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Connected download to loopback source ($peerAddr)'),
            duration: const Duration(seconds: 2),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = 'Failed to connect download: $e';
          _isConnectingDownload = false;
        });
      }
    }
  }

  Future<void> _cleanupTestData() async {
    final fixtureCount = _labStatus?.fixtures.length ?? 0;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.darkSurface,
        title: const Text('Confirm Cleanup'),
        content: Text(
          'This will remove $fixtureCount fixture file(s) in <workdir>/fixtures/ '
          'and clean temporary download part files. Incoming and finished downloads will never be touched.\n\n'
          'Are you sure you want to proceed?',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            style: FilledButton.styleFrom(backgroundColor: AppTheme.statusFailed),
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Delete Test Data'),
          ),
        ],
      ),
    );

    if (confirmed != true) return;

    setState(() {
      _isCleaningUp = true;
      _errorMessage = null;
    });

    try {
      final report = await widget.viewModel.cleanupLabTestData();
      final updatedStatus = await widget.viewModel.getLabStatus();
      if (mounted) {
        setState(() {
          _labStatus = updatedStatus;
          _lastCreatedFixture = null;
          _addedToDownloadsSuccess = false;
          _isCleaningUp = false;
        });
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              'Cleanup completed: ${report.deletedFilesCount} file(s) deleted across '
              '${report.cleanedFolders.join(', ')} (${Formatters.formatBytes(report.freedBytes)} freed).',
            ),
            duration: const Duration(seconds: 4),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = 'Cleanup failed: $e';
          _isCleaningUp = false;
        });
      }
    }
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
        constraints: const BoxConstraints(maxWidth: 720, maxHeight: 820),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
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
                    child: const Icon(Icons.science, color: AppTheme.primary, size: 24),
                  ),
                  const SizedBox(width: 12),
                  const Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Guided Local Loopback Test Flow',
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        SizedBox(height: 2),
                        Text(
                          'Full end-to-end eD2K testing on 127.0.0.1 without touching a terminal.',
                          style: TextStyle(fontSize: 12, color: AppColors.textSoft),
                        ),
                      ],
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.close, size: 20, color: AppColors.textMuted),
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                ],
              ),
              const SizedBox(height: 16),

              if (_errorMessage != null)
                Container(
                  margin: const EdgeInsets.only(bottom: 12),
                  padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
                  decoration: BoxDecoration(
                    color: AppTheme.statusFailed.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(color: AppTheme.statusFailed.withValues(alpha: 0.4)),
                  ),
                  child: Row(
                    children: [
                      const Icon(Icons.error_outline, color: AppTheme.statusFailed, size: 18),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          _errorMessage!,
                          style: const TextStyle(color: AppColors.textPrimary, fontSize: 12),
                        ),
                      ),
                    ],
                  ),
                ),

              Expanded(
                child: _isLoading
                    ? const Center(child: CircularProgressIndicator())
                    : SingleChildScrollView(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            _buildDirectoryOverview(),
                            _buildStep1FixtureCreation(),
                            const SizedBox(height: 16),
                            _buildStep2LinkAndDownload(),
                            const SizedBox(height: 16),
                            _buildStep3LoopbackPeer(),
                            const SizedBox(height: 16),
                            _buildStep4ConnectDownload(),
                            const SizedBox(height: 16),
                            _buildStep5Cleanup(),
                          ],
                        ),
                      ),
              ),

              const SizedBox(height: 16),

              // Footer Close
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: const Text('Close'),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildDirectoryOverview() {
    final workdir = widget.viewModel.daemonInfo?.workdirPath ??
        widget.viewModel.supervisor?.resolveWorkDir().directory.path ??
        'data';
    final downloadsDir = widget.viewModel.defaultDownloadDirectory.path;

    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.folder_outlined, color: AppTheme.primary, size: 16),
              const SizedBox(width: 8),
              const Expanded(
                child: Text(
                  'Storage: data/ (Internal State) vs downloads/ (Completed Files)',
                  style: TextStyle(fontSize: 11, fontWeight: FontWeight.bold, color: AppColors.textPrimary),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 8),
              OutlinedButton.icon(
                icon: const Icon(Icons.folder_open, size: 13),
                label: const Text('Open Folder', style: TextStyle(fontSize: 11)),
                style: OutlinedButton.styleFrom(
                  padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                  side: const BorderSide(color: AppTheme.darkBorder),
                  visualDensity: VisualDensity.compact,
                ),
                onPressed: () => widget.viewModel.openDownloadsFolder(),
              ),
            ],
          ),
          const SizedBox(height: 4),
          Text(
            '• Internal data (DB, temp .part, fixtures): $workdir',
            style: const TextStyle(fontSize: 10, fontFamily: 'monospace', color: AppColors.textSoft),
            overflow: TextOverflow.ellipsis,
          ),
          Text(
            '• Completed downloads: $downloadsDir',
            style: const TextStyle(fontSize: 10, fontFamily: 'monospace', color: AppTheme.primary),
            overflow: TextOverflow.ellipsis,
          ),
        ],
      ),
    );
  }

  Widget _buildStepCard({
    required int stepNumber,
    required String title,
    required String description,
    required Widget content,
    bool isCompleted = false,
  }) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(
          color: isCompleted ? AppTheme.statusCompleted.withValues(alpha: 0.5) : AppTheme.darkBorder,
          width: 0.75,
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                width: 24,
                height: 24,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: isCompleted ? AppTheme.statusCompleted : AppTheme.primary,
                ),
                alignment: Alignment.center,
                child: isCompleted
                    ? const Icon(Icons.check, size: 14, color: Colors.black)
                    : Text(
                        '$stepNumber',
                        style: const TextStyle(
                          fontSize: 12,
                          fontWeight: FontWeight.bold,
                          color: AppColors.textPrimary,
                        ),
                      ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      title,
                      style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                    Text(
                      description,
                      style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
                    ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          content,
        ],
      ),
    );
  }

  Widget _buildStep1FixtureCreation() {
    final fixture = _lastCreatedFixture;
    return _buildStepCard(
      stepNumber: 1,
      title: 'Generate Local Test Fixture',
      description: 'Creates a deterministic file in <workdir>/fixtures/ and computes its eD2K hash.',
      isCompleted: fixture != null,
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Wrap(
            spacing: 8,
            children: [
              ChoiceChip(
                label: const Text('1 MB Sample (sample_1mb)'),
                selected: _selectedFixtureType == 'sample_1mb',
                onSelected: (val) {
                  if (val) setState(() => _selectedFixtureType = 'sample_1mb');
                },
              ),
              ChoiceChip(
                label: const Text('9.28 MB Full Part (part_9728000)'),
                selected: _selectedFixtureType == 'part_9728000',
                onSelected: (val) {
                  if (val) setState(() => _selectedFixtureType = 'part_9728000');
                },
              ),
              ChoiceChip(
                label: const Text('9.28 MB + 1 Byte (pattern_9728001)'),
                selected: _selectedFixtureType == 'pattern_9728001',
                onSelected: (val) {
                  if (val) setState(() => _selectedFixtureType = 'pattern_9728001');
                },
              ),
              ChoiceChip(
                label: const Text('4 KB Text (text)'),
                selected: _selectedFixtureType == 'text',
                onSelected: (val) {
                  if (val) setState(() => _selectedFixtureType = 'text');
                },
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              FilledButton.icon(
                icon: _isCreatingFixture
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(strokeWidth: 2, color: AppColors.textPrimary),
                      )
                    : const Icon(Icons.create_new_folder_outlined, size: 16),
                label: Text(_isCreatingFixture ? 'Generating...' : 'Generate Fixture'),
                onPressed: _isCreatingFixture ? null : _createFixture,
              ),
              if (fixture != null) ...[
                const SizedBox(width: 12),
                const Icon(Icons.check_circle, color: AppTheme.statusCompleted, size: 16),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(
                    '${fixture.fileName} (${Formatters.formatBytes(fixture.sizeBytes)})',
                    style: const TextStyle(fontSize: 12, fontWeight: FontWeight.bold, color: AppColors.textPrimary),
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

  Widget _buildStep2LinkAndDownload() {
    final fixture = _lastCreatedFixture;
    final hasFixture = fixture != null;

    return _buildStepCard(
      stepNumber: 2,
      title: 'Obtain Canonical eD2K Link & Add to Downloads',
      description: 'Use the hash to create a tracked download task inside the client.',
      isCompleted: _addedToDownloadsSuccess,
      content: hasFixture
          ? Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Container(
                  padding: const EdgeInsets.all(10),
                  decoration: BoxDecoration(
                    color: AppTheme.darkSurface,
                    borderRadius: BorderRadius.circular(6),
                    border: Border.all(color: AppTheme.darkBorder, width: 0.5),
                  ),
                  child: Row(
                    children: [
                      Expanded(
                        child: SelectableText(
                          fixture.ed2kLink,
                          style: const TextStyle(fontSize: 11, fontFamily: 'monospace', color: AppColors.textSecondary),
                        ),
                      ),
                      IconButton(
                        icon: const Icon(Icons.copy, size: 16, color: AppColors.textSecondary),
                        tooltip: 'Copy Link',
                        onPressed: () {
                          Clipboard.setData(ClipboardData(text: fixture.ed2kLink));
                          ScaffoldMessenger.of(context).showSnackBar(
                            const SnackBar(
                              content: Text('eD2K Link copied to clipboard'),
                              duration: Duration(seconds: 2),
                            ),
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
                      icon: _isAddingToDownloads
                          ? const SizedBox(
                              width: 14,
                              height: 14,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Icon(Icons.download, size: 16),
                      label: Text(_addedToDownloadsSuccess ? 'Added to Downloads!' : 'Add to Downloads'),
                      onPressed: _isAddingToDownloads ? null : _addToDownloads,
                    ),
                    if (_addedToDownloadsSuccess)
                      const Text(
                        'Download task created in UI',
                        style: TextStyle(color: AppTheme.statusCompleted, fontSize: 12, fontWeight: FontWeight.bold),
                      ),
                  ],
                ),
              ],
            )
          : const Text(
              'Complete Step 1 to generate an eD2K fixture first.',
              style: TextStyle(fontSize: 12, color: AppColors.textFaint),
            ),
    );
  }

  Widget _buildStep3LoopbackPeer() {
    final activePeer = _labStatus?.activeLoopbackPeer;
    final isRunning = activePeer != null;

    return _buildStepCard(
      stepNumber: 3,
      title: 'Loopback Test Peer (127.0.0.1)',
      description: 'Spawns a local eD2K protocol peer hosting the test fixtures.',
      isCompleted: isRunning,
      content: Wrap(
        spacing: 16,
        runSpacing: 8,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          FilledButton.icon(
            style: FilledButton.styleFrom(
              backgroundColor: isRunning ? AppTheme.statusPaused : AppTheme.secondary,
            ),
            icon: _isManagingPeer
                ? const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2, color: AppColors.textPrimary),
                  )
                : Icon(isRunning ? Icons.stop : Icons.play_arrow, size: 16),
            label: Text(isRunning ? 'Stop Loopback Peer' : 'Start Loopback Peer'),
            onPressed: _isManagingPeer ? null : _togglePeer,
          ),
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 10,
                height: 10,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: isRunning ? AppTheme.statusCompleted : AppColors.fillMedium,
                ),
              ),
              const SizedBox(width: 8),
              Text(
                isRunning
                    ? 'Active on $activePeer (Serving ${_labStatus?.fixtures.length ?? 0} file(s))'
                    : 'Peer offline',
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: isRunning ? FontWeight.bold : FontWeight.normal,
                  color: isRunning ? AppColors.textPrimary : AppColors.textFaint,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildStep4ConnectDownload() {
    final activePeer = _labStatus?.activeLoopbackPeer;
    final downloads = widget.viewModel.filteredDownloads;

    if (downloads.isEmpty) {
      return _buildStepCard(
        stepNumber: 4,
        title: 'Connect Download to Loopback Source',
        description: 'Instructs the client to connect to 127.0.0.1 source to transfer chunks.',
        isCompleted: false,
        content: const Text(
          'No downloads in queue. Add an eD2K link in Step 2.',
          style: TextStyle(fontSize: 12, color: AppColors.textFaint),
        ),
      );
    }

    final selectedDownload = downloads.firstWhere(
      (d) => d.id == (_selectedDownloadIdForConnect ?? downloads.first.id),
      orElse: () => downloads.first,
    );

    final isPeerRunning = activePeer != null;
    final isCompleted = selectedDownload.status == DownloadStatus.completed;
    final isDownloading = selectedDownload.status == DownloadStatus.downloading;

    // Check size match with created fixture if available
    final fixture = _lastCreatedFixture;
    final hasSizeMismatch = fixture != null && selectedDownload.totalBytes != fixture.sizeBytes;
    final canConnect = isPeerRunning && !isCompleted && !_isConnectingDownload;

    return _buildStepCard(
      stepNumber: 4,
      title: 'Connect Download to Loopback Source',
      description: 'Instructs the client to connect to 127.0.0.1 source to transfer chunks.',
      isCompleted: isCompleted,
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          DropdownButtonFormField<String>(
            initialValue: selectedDownload.id,
            decoration: const InputDecoration(
              labelText: 'Target Download Task',
              isDense: true,
              contentPadding: EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            ),
            items: downloads.map((d) {
              return DropdownMenuItem<String>(
                value: d.id,
                child: Text(
                  '${d.fileName} (${Formatters.formatPercent(d.progress)})',
                  style: const TextStyle(fontSize: 12),
                  overflow: TextOverflow.ellipsis,
                ),
              );
            }).toList(),
            onChanged: (val) {
              setState(() => _selectedDownloadIdForConnect = val);
            },
          ),
          if (hasSizeMismatch) ...[
            const SizedBox(height: 8),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
              decoration: BoxDecoration(
                color: AppColors.warning.withValues(alpha: 0.15),
                borderRadius: BorderRadius.circular(6),
                border: Border.all(color: AppColors.warning.withValues(alpha: 0.4)),
              ),
              child: Row(
                children: [
                  const Icon(Icons.warning_amber_rounded, color: AppColors.accentAmber, size: 16),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      'Warning: Download size (${Formatters.formatBytes(selectedDownload.totalBytes)}) does not match fixture (${Formatters.formatBytes(fixture.sizeBytes)}).',
                      style: const TextStyle(fontSize: 11, color: AppColors.accentAmber),
                    ),
                  ),
                ],
              ),
            ),
          ],
          const SizedBox(height: 12),
          // Status progression banner
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            decoration: BoxDecoration(
              color: AppTheme.darkSurface,
              borderRadius: BorderRadius.circular(6),
              border: Border.all(color: AppTheme.darkBorder, width: 0.5),
            ),
            child: Row(
              children: [
                Icon(
                  isCompleted
                      ? Icons.check_circle
                      : (isDownloading ? Icons.sync : Icons.info_outline),
                  size: 16,
                  color: isCompleted
                      ? AppTheme.statusCompleted
                      : (isDownloading ? AppTheme.primary : AppColors.textMuted),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    isCompleted
                        ? 'Download completed and verified (${Formatters.formatBytes(selectedDownload.totalBytes)})'
                        : (isDownloading
                            ? 'Transferring: ${Formatters.formatBytes(selectedDownload.receivedBytes)} / ${Formatters.formatBytes(selectedDownload.totalBytes)} (${Formatters.formatSpeed(selectedDownload.downloadRateBps)})'
                            : 'State: ${selectedDownload.status.name.toUpperCase()} — Ready to connect to loopback peer.'),
                    style: TextStyle(
                      fontSize: 11,
                      color: isCompleted ? AppTheme.statusCompleted : AppColors.textSecondary,
                      fontWeight: isCompleted || isDownloading ? FontWeight.bold : FontWeight.normal,
                    ),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              FilledButton.icon(
                icon: _isConnectingDownload
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(strokeWidth: 2, color: AppColors.textPrimary),
                      )
                    : const Icon(Icons.cable, size: 16),
                label: Text(isCompleted ? 'Already Completed' : 'Connect to Source'),
                onPressed: canConnect ? _connectDownload : null,
              ),
              if (!isPeerRunning) ...[
                const SizedBox(width: 12),
                const Expanded(
                  child: Text(
                    'Start loopback peer in Step 3 before connecting.',
                    style: TextStyle(fontSize: 11, color: AppColors.accentAmber),
                  ),
                ),
              ],
            ],
          ),
          if (isCompleted) ...[
            const SizedBox(height: 12),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              decoration: BoxDecoration(
                color: AppTheme.darkCard,
                borderRadius: BorderRadius.circular(6),
                border: Border.all(color: AppTheme.statusCompleted.withValues(alpha: 0.4)),
              ),
              child: Row(
                children: [
                  const Icon(Icons.verified, color: AppTheme.statusCompleted, size: 16),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      'Final file: ${widget.viewModel.defaultDownloadDirectory.path}\\${selectedDownload.fileName}',
                      style: const TextStyle(fontSize: 11, fontFamily: 'monospace', color: AppColors.textPrimary),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  const SizedBox(width: 8),
                  OutlinedButton.icon(
                    icon: const Icon(Icons.folder_open, size: 14),
                    label: const Text('Open Folder', style: TextStyle(fontSize: 11)),
                    style: OutlinedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                      side: const BorderSide(color: AppTheme.darkBorder),
                      visualDensity: VisualDensity.compact,
                    ),
                    onPressed: () => widget.viewModel.openDownloadsFolder(),
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildStep5Cleanup() {
    final isPortable = _labStatus?.isPortable ?? false;

    return _buildStepCard(
      stepNumber: 5,
      title: 'Clean Test Data (Fixtures & Incomplete Parts)',
      description: 'Purges generated files in <workdir>/fixtures and partial files in <workdir>/temp. Completed downloads in downloads/ are preserved and never touched.',
      content: Row(
        children: [
          OutlinedButton.icon(
            style: OutlinedButton.styleFrom(
              foregroundColor: AppTheme.statusFailed,
              side: const BorderSide(color: AppTheme.statusFailed, width: 0.75),
            ),
            icon: _isCleaningUp
                ? const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2, color: AppTheme.statusFailed),
                  )
                : const Icon(Icons.delete_sweep_outlined, size: 16),
            label: const Text('Cleanup Test Data'),
            onPressed: isPortable && !_isCleaningUp ? _cleanupTestData : null,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              isPortable
                  ? 'Safe to run in Portable mode.'
                  : 'Cleanup is restricted in installed mode to protect user files.',
              style: TextStyle(
                fontSize: 11,
                color: isPortable ? AppColors.textMuted : AppColors.accentAmber,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
