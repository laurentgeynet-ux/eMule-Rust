// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/foundation.dart';

@immutable
class LabFixtureInfo {
  final String fileName;
  final String filePath;
  final int sizeBytes;
  final String ed2kHash;
  final String ed2kLink;
  final String status;

  const LabFixtureInfo({
    required this.fileName,
    required this.filePath,
    required this.sizeBytes,
    required this.ed2kHash,
    required this.ed2kLink,
    this.status = 'ready',
  });

  factory LabFixtureInfo.fromJson(Map<String, dynamic> json) {
    return LabFixtureInfo(
      fileName: json['fileName'] as String? ?? '',
      filePath: json['filePath'] as String? ?? '',
      sizeBytes: (json['sizeBytes'] as num?)?.toInt() ?? 0,
      ed2kHash: json['ed2kHash'] as String? ?? '',
      ed2kLink: json['ed2kLink'] as String? ?? '',
      status: json['status'] as String? ?? 'ready',
    );
  }
}

@immutable
class LabPeerInfo {
  final String peerAddr;
  final int sharedCount;
  final String status;

  const LabPeerInfo({
    required this.peerAddr,
    required this.sharedCount,
    required this.status,
  });

  factory LabPeerInfo.fromJson(Map<String, dynamic> json) {
    return LabPeerInfo(
      peerAddr: json['peerAddr'] as String? ?? '',
      sharedCount: (json['sharedCount'] as num?)?.toInt() ?? 0,
      status: json['status'] as String? ?? 'running',
    );
  }
}

@immutable
class LabStatus {
  final String workdirMode;
  final String workdirPath;
  final bool isPortable;
  final String? activeLoopbackPeer;
  final List<LabFixtureInfo> fixtures;

  const LabStatus({
    required this.workdirMode,
    required this.workdirPath,
    required this.isPortable,
    this.activeLoopbackPeer,
    required this.fixtures,
  });

  factory LabStatus.fromJson(Map<String, dynamic> json) {
    final rawFixtures = json['fixtures'] as List<dynamic>? ?? [];
    final fixtures = rawFixtures
        .whereType<Map<String, dynamic>>()
        .map((m) => LabFixtureInfo.fromJson(m))
        .toList();

    return LabStatus(
      workdirMode: json['workdirMode'] as String? ?? 'InstalledDefault',
      workdirPath: json['workdirPath'] as String? ?? '',
      isPortable: json['isPortable'] as bool? ?? false,
      activeLoopbackPeer: json['activeLoopbackPeer'] as String?,
      fixtures: fixtures,
    );
  }
}

@immutable
class LabCleanupReport {
  final bool success;
  final List<String> cleanedFolders;
  final int deletedFilesCount;
  final int freedBytes;
  final String message;

  const LabCleanupReport({
    required this.success,
    required this.cleanedFolders,
    required this.deletedFilesCount,
    required this.freedBytes,
    required this.message,
  });

  factory LabCleanupReport.fromJson(Map<String, dynamic> json) {
    final rawFolders = json['cleanedFolders'] as List<dynamic>? ?? [];
    return LabCleanupReport(
      success: json['success'] as bool? ?? false,
      cleanedFolders: rawFolders.map((e) => e.toString()).toList(),
      deletedFilesCount: (json['deletedFilesCount'] as num?)?.toInt() ?? 0,
      freedBytes: (json['freedBytes'] as num?)?.toInt() ?? 0,
      message: json['message'] as String? ?? '',
    );
  }
}

