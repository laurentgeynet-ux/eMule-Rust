import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../view_models/downloads_view_model.dart';

class IpFilterDialog extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const IpFilterDialog({super.key, required this.viewModel});

  @override
  State<IpFilterDialog> createState() => _IpFilterDialogState();
}

class _IpFilterDialogState extends State<IpFilterDialog> with SingleTickerProviderStateMixin {
  late TabController _tabController;
  final TextEditingController _filePathController = TextEditingController();
  final TextEditingController _downloadLimitController = TextEditingController();
  final TextEditingController _uploadLimitController = TextEditingController();
  final TextEditingController _connLimitController = TextEditingController();

  bool _isLoading = false;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 2, vsync: this);
    _loadData();
  }

  @override
  void dispose() {
    _tabController.dispose();
    _filePathController.dispose();
    _downloadLimitController.dispose();
    _uploadLimitController.dispose();
    _connLimitController.dispose();
    super.dispose();
  }

  Future<void> _loadData() async {
    setState(() => _isLoading = true);
    await widget.viewModel.fetchFilterStatus();
    await widget.viewModel.fetchRateLimits();

    final limits = widget.viewModel.rateLimits;
    if (limits != null) {
      _downloadLimitController.text = (limits.downloadBytesPerSec ~/ 1024).toString();
      _uploadLimitController.text = (limits.uploadBytesPerSec ~/ 1024).toString();
      _connLimitController.text = limits.connectionsPerMinute.toString();
    }

    if (mounted) {
      setState(() => _isLoading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final filterStatus = widget.viewModel.filterStatus;

    return Dialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 720, maxHeight: 620),
        child: Padding(
          padding: const EdgeInsets.all(22),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Header
              Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: Colors.amber.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: const Icon(Icons.security, color: Colors.amber, size: 24),
                  ),
                  const SizedBox(width: 12),
                  const Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'IP Filter & Rate Limiters',
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                            color: Colors.white,
                          ),
                        ),
                        SizedBox(height: 2),
                        Text(
                          'Local defensive IP filter (ipfilter.dat) and Token Bucket bandwidth quotas',
                          style: TextStyle(
                            fontSize: 12,
                            color: Colors.white60,
                          ),
                        ),
                      ],
                    ),
                  ),
                  IconButton(
                    onPressed: () => Navigator.of(context).pop(),
                    icon: const Icon(Icons.close, color: Colors.white70),
                  ),
                ],
              ),
              const SizedBox(height: 16),

              // Tabs
              TabBar(
                controller: _tabController,
                indicatorColor: Colors.amber,
                labelColor: Colors.amber,
                unselectedLabelColor: Colors.white60,
                tabs: const [
                  Tab(icon: Icon(Icons.filter_alt, size: 18), text: 'IP Filter'),
                  Tab(icon: Icon(Icons.speed, size: 18), text: 'Rate Limits'),
                ],
              ),
              const SizedBox(height: 16),

              // Tab View
              Expanded(
                child: _isLoading
                    ? const Center(child: CircularProgressIndicator())
                    : TabBarView(
                        controller: _tabController,
                        children: [
                          _buildFilterTab(filterStatus),
                          _buildRateLimitsTab(),
                        ],
                      ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildFilterTab(FilterStatusModel? status) {
    final rangeCount = status?.rangeCount ?? 0;
    final hitsCount = status?.hitsCount ?? 0;
    final enabled = status?.enabled ?? true;
    final report = status?.lastImportReport;

    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Filter status card
          Container(
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
              color: AppTheme.darkCard,
              borderRadius: BorderRadius.circular(10),
              border: Border.all(color: AppTheme.darkBorder),
            ),
            child: Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Text(
                            enabled ? 'Filter Active' : 'Filter Disabled',
                            style: TextStyle(
                              fontWeight: FontWeight.bold,
                              color: enabled ? Colors.greenAccent : Colors.grey,
                            ),
                          ),
                          const SizedBox(width: 8),
                          Container(
                            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                            decoration: BoxDecoration(
                              color: Colors.white.withValues(alpha: 0.1),
                              borderRadius: BorderRadius.circular(4),
                            ),
                            child: Text(
                              '$rangeCount ranges loaded',
                              style: const TextStyle(fontSize: 11, color: Colors.white70),
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 4),
                      Text(
                        'Total blocked connection attempts: $hitsCount',
                        style: const TextStyle(fontSize: 12, color: Colors.white60),
                      ),
                    ],
                  ),
                ),
                Switch(
                  value: enabled,
                  activeThumbColor: Colors.amber,
                  onChanged: (val) async {
                    setState(() => _isLoading = true);
                    await widget.viewModel.setFilterEnabled(val);
                    if (mounted) setState(() => _isLoading = false);
                  },
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),

          // Import section
          const Text(
            'Import ipfilter.dat',
            style: TextStyle(fontWeight: FontWeight.w600, color: Colors.white, fontSize: 13),
          ),
          const SizedBox(height: 6),
          const Text(
            'Load a local PeerGuardian / eMule formatted ipfilter.dat file. Remote URL download is disallowed for safety.',
            style: TextStyle(fontSize: 11, color: Colors.white60),
          ),
          const SizedBox(height: 10),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _filePathController,
                  style: const TextStyle(color: Colors.white, fontSize: 12),
                  decoration: InputDecoration(
                    hintText: 'C:\\path\\to\\ipfilter.dat',
                    hintStyle: const TextStyle(color: Colors.white38, fontSize: 12),
                    filled: true,
                    fillColor: AppTheme.darkBackground,
                    contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(color: AppTheme.darkBorder),
                    ),
                  ),
                ),
              ),
              const SizedBox(width: 8),
              ElevatedButton.icon(
                onPressed: () async {
                  final path = _filePathController.text.trim();
                  if (path.isEmpty) return;
                  setState(() => _isLoading = true);
                  final success = await widget.viewModel.importFilterFile(path);
                  if (mounted) {
                    setState(() => _isLoading = false);
                    if (success) {
                      _filePathController.clear();
                    }
                  }
                },
                style: ElevatedButton.styleFrom(
                  backgroundColor: Colors.amber.shade700,
                  foregroundColor: Colors.black,
                  padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
                ),
                icon: const Icon(Icons.file_upload, size: 16),
                label: const Text('Import', style: TextStyle(fontWeight: FontWeight.bold)),
              ),
              if (rangeCount > 0) ...[
                const SizedBox(width: 8),
                OutlinedButton(
                  onPressed: () async {
                    setState(() => _isLoading = true);
                    await widget.viewModel.clearFilter();
                    if (mounted) setState(() => _isLoading = false);
                  },
                  style: OutlinedButton.styleFrom(
                    foregroundColor: Colors.redAccent,
                    side: const BorderSide(color: Colors.redAccent),
                  ),
                  child: const Text('Clear'),
                ),
              ],
            ],
          ),
          const SizedBox(height: 16),

          // Import Report (if available)
          if (report != null) ...[
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: AppTheme.darkBackground,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: AppTheme.darkBorder),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text(
                    'Last Import Report',
                    style: TextStyle(fontWeight: FontWeight.bold, fontSize: 12, color: Colors.amber),
                  ),
                  const SizedBox(height: 6),
                  Text('Total lines read: ${report.totalLines}', style: const TextStyle(fontSize: 11, color: Colors.white70)),
                  Text('Valid entries: ${report.validEntriesParsed}', style: const TextStyle(fontSize: 11, color: Colors.white70)),
                  Text('Coalesced (merged) ranges: ${report.coalescedCount}', style: const TextStyle(fontSize: 11, color: Colors.white70)),
                  Text('Final active intervals: ${report.finalRangeCount}', style: const TextStyle(fontSize: 11, color: Colors.white70)),
                  if (report.invalidLines > 0)
                    Text('Invalid lines skipped: ${report.invalidLines}', style: const TextStyle(fontSize: 11, color: Colors.orangeAccent)),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildRateLimitsTab() {
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Bandwidth & Token Bucket Quotas',
            style: TextStyle(fontWeight: FontWeight.bold, color: Colors.white, fontSize: 13),
          ),
          const SizedBox(height: 4),
          const Text(
            'Limits are enforced asynchronously using Token Buckets. 0 means unlimited.',
            style: TextStyle(fontSize: 11, color: Colors.white60),
          ),
          const SizedBox(height: 16),

          // Download limit
          TextField(
            controller: _downloadLimitController,
            keyboardType: TextInputType.number,
            style: const TextStyle(color: Colors.white, fontSize: 12),
            decoration: const InputDecoration(
              labelText: 'Download Limit (KB/s) [0 = unlimited]',
              labelStyle: TextStyle(color: Colors.white70, fontSize: 12),
              filled: true,
              fillColor: AppTheme.darkBackground,
            ),
          ),
          const SizedBox(height: 12),

          // Upload limit
          TextField(
            controller: _uploadLimitController,
            keyboardType: TextInputType.number,
            style: const TextStyle(color: Colors.white, fontSize: 12),
            decoration: const InputDecoration(
              labelText: 'Upload Limit (KB/s) [0 = unlimited]',
              labelStyle: TextStyle(color: Colors.white70, fontSize: 12),
              filled: true,
              fillColor: AppTheme.darkBackground,
            ),
          ),
          const SizedBox(height: 12),

          // Connection limit
          TextField(
            controller: _connLimitController,
            keyboardType: TextInputType.number,
            style: const TextStyle(color: Colors.white, fontSize: 12),
            decoration: const InputDecoration(
              labelText: 'Max New Connections / Minute',
              labelStyle: TextStyle(color: Colors.white70, fontSize: 12),
              filled: true,
              fillColor: AppTheme.darkBackground,
            ),
          ),
          const SizedBox(height: 20),

          // Save button
          Align(
            alignment: Alignment.centerRight,
            child: ElevatedButton.icon(
              onPressed: () async {
                final dlKb = int.tryParse(_downloadLimitController.text.trim()) ?? 0;
                final ulKb = int.tryParse(_uploadLimitController.text.trim()) ?? 0;
                final conn = int.tryParse(_connLimitController.text.trim()) ?? 120;

                final limits = RateLimitsModel(
                  downloadBytesPerSec: dlKb * 1024,
                  uploadBytesPerSec: ulKb * 1024,
                  connectionsPerMinute: conn,
                  serverRequestsPerSec: 10,
                  kadPacketsPerSec: 50,
                );

                setState(() => _isLoading = true);
                await widget.viewModel.setRateLimits(limits);
                if (mounted) setState(() => _isLoading = false);
              },
              style: ElevatedButton.styleFrom(
                backgroundColor: Colors.amber.shade700,
                foregroundColor: Colors.black,
              ),
              icon: const Icon(Icons.save, size: 16),
              label: const Text('Save Limits', style: TextStyle(fontWeight: FontWeight.bold)),
            ),
          ),
        ],
      ),
    );
  }
}
