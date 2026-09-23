// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

/// How the daemon working directory was resolved.
enum WorkDirMode {
  explicitCli,
  environment,
  portable,
  installedDefault,
}

/// Pure-Dart view of resolved daemon directories (paths only — no `dart:io`).
class WorkDirInfo {
  final String? bundleRootPath;
  final String workDirPath;
  final String defaultDownloadDirPath;
  final WorkDirMode mode;

  const WorkDirInfo({
    this.bundleRootPath,
    required this.workDirPath,
    required this.defaultDownloadDirPath,
    required this.mode,
  });
}
