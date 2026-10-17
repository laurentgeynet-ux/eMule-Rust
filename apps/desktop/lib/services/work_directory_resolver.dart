// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:io';

import 'package:app_shell/core/work_dirs.dart';

export 'package:app_shell/core/work_dirs.dart' show WorkDirMode;


class ResolvedDirectories {
  final Directory? bundleRoot;
  final Directory workdir;
  final Directory defaultDownloadDirectory;
  final WorkDirMode mode;

  /// Compatibility getter returning [workdir].
  Directory get directory => workdir;

  const ResolvedDirectories({
    this.bundleRoot,
    required this.workdir,
    required this.defaultDownloadDirectory,
    required this.mode,
  });
}

typedef ResolvedWorkDir = ResolvedDirectories;

class WorkDirectoryResolver {
  static const String portableFlagFilename = 'portable.flag';
  static const String envWorkdir = 'EMULERUST_WORKDIR';
  /// Alias déprécié (avant le renommage eMuleRust), honoré avant [envLegacyWorkdir].
  static const String envSionWorkdir = 'EMULE_SION_WORKDIR';
  static const String envLegacyWorkdir = 'MULE_DATA_DIR';
  static const String envDownloadsDir = 'EMULERUST_DOWNLOADS_DIR';
  /// Alias déprécié (avant le renommage eMuleRust).
  static const String envSionDownloadsDir = 'EMULE_SION_DOWNLOADS_DIR';

  /// Resolves the runtime directories using the standard 4-tier cascade:
  /// 1. [customDir] (explicit CLI override), [customDownloadsDir]
  /// 2. [environment] EMULERUST_WORKDIR (or deprecated EMULE_SION_WORKDIR,
  ///    or legacy MULE_DATA_DIR)
  /// 3. portable.flag adjacent to executable or bundle root -> `<bundle_root>/data` & `<bundle_root>/downloads`
  /// 4. System default (%LOCALAPPDATA%\eMuleRust on Windows, ~/.emulerust on Unix; %USERPROFILE%\Downloads\eMuleRust)
  static ResolvedDirectories resolve({
    Directory? customDir,
    Directory? customDownloadsDir,
    File? executableFile,
    Map<String, String>? environment,
  }) {
    final env = environment ?? Platform.environment;

    Directory? explicitDownloads;
    if (customDownloadsDir != null) {
      explicitDownloads = customDownloadsDir;
    } else {
      final envDownloads =
          (env[envDownloadsDir] ?? env[envSionDownloadsDir])?.trim();
      if (envDownloads != null && envDownloads.isNotEmpty) {
        explicitDownloads = Directory(envDownloads);
      }
    }

    // 1. Explicit override
    if (customDir != null) {
      final parent = customDir.parent;
      final flagInParent = File('${parent.path}/$portableFlagFilename').existsSync();
      final bundleRoot = flagInParent ? parent : null;
      final defaultDownloads = explicitDownloads ??
          (bundleRoot != null
              ? Directory('${bundleRoot.path}/downloads')
              : Directory('${customDir.path}/downloads'));

      return ResolvedDirectories(
        bundleRoot: bundleRoot,
        workdir: customDir,
        defaultDownloadDirectory: defaultDownloads,
        mode: WorkDirMode.explicitCli,
      );
    }

    // 2. Environment variable: EMULERUST_WORKDIR (or deprecated
    //    EMULE_SION_WORKDIR, or legacy MULE_DATA_DIR)
    final envVal = (env[envWorkdir] ?? env[envSionWorkdir])?.trim();
    if (envVal != null && envVal.isNotEmpty) {
      final dir = Directory(envVal);
      final parent = dir.parent;
      final flagInParent = File('${parent.path}/$portableFlagFilename').existsSync();
      final bundleRoot = flagInParent ? parent : null;
      final defaultDownloads = explicitDownloads ??
          (bundleRoot != null
              ? Directory('${bundleRoot.path}/downloads')
              : Directory('${dir.path}/downloads'));

      return ResolvedDirectories(
        bundleRoot: bundleRoot,
        workdir: dir,
        defaultDownloadDirectory: defaultDownloads,
        mode: WorkDirMode.environment,
      );
    }

    final legacyVal = env[envLegacyWorkdir]?.trim();
    if (legacyVal != null && legacyVal.isNotEmpty) {
      final dir = Directory(legacyVal);
      final parent = dir.parent;
      final flagInParent = File('${parent.path}/$portableFlagFilename').existsSync();
      final bundleRoot = flagInParent ? parent : null;
      final defaultDownloads = explicitDownloads ??
          (bundleRoot != null
              ? Directory('${bundleRoot.path}/downloads')
              : Directory('${dir.path}/downloads'));

      return ResolvedDirectories(
        bundleRoot: bundleRoot,
        workdir: dir,
        defaultDownloadDirectory: defaultDownloads,
        mode: WorkDirMode.environment,
      );
    }

    // 3. Portable mode: strictly bounded bundle root detection
    // If executable directory is named "bin", bundleRoot = parent(executableDir)
    // Otherwise, bundleRoot = executableDir
    // portableFlag = bundleRoot / portable.flag
    final exe = executableFile ?? File(Platform.resolvedExecutable);
    final exeDir = exe.parent;
    final dirName = exeDir.uri.pathSegments.where((s) => s.isNotEmpty).lastOrNull?.toLowerCase();
    final isBin = dirName == 'bin';
    final bundleRoot = isBin ? exeDir.parent : exeDir;

    final portableFlag = File('${bundleRoot.path}/$portableFlagFilename');
    if (portableFlag.existsSync()) {
      return ResolvedDirectories(
        bundleRoot: bundleRoot,
        workdir: Directory('${bundleRoot.path}/data'),
        defaultDownloadDirectory: explicitDownloads ?? Directory('${bundleRoot.path}/downloads'),
        mode: WorkDirMode.portable,
      );
    }

    // 4. Installed default
    Directory defaultWorkdir;
    Directory defaultDownloads;

    if (Platform.isWindows) {
      final localAppData = env['LOCALAPPDATA']?.trim();
      final appData = env['APPDATA']?.trim();
      if (localAppData != null && localAppData.isNotEmpty) {
        defaultWorkdir = Directory('$localAppData\\eMuleRust');
      } else if (appData != null && appData.isNotEmpty) {
        defaultWorkdir = Directory('$appData\\eMuleRust');
      } else {
        defaultWorkdir = Directory('.mule_data');
      }

      final userProfile = env['USERPROFILE']?.trim();
      if (userProfile != null && userProfile.isNotEmpty) {
        defaultDownloads = explicitDownloads ?? Directory('$userProfile\\Downloads\\eMuleRust');
      } else {
        defaultDownloads = explicitDownloads ?? Directory('${defaultWorkdir.path}\\downloads');
      }
    } else {
      final home = env['HOME']?.trim();
      if (home != null && home.isNotEmpty) {
        defaultWorkdir = Directory('$home/.emulerust');
        defaultDownloads = explicitDownloads ?? Directory('$home/Downloads/eMuleRust');
      } else {
        defaultWorkdir = Directory('.mule_data');
        defaultDownloads = explicitDownloads ?? Directory('.mule_data/downloads');
      }
    }

    return ResolvedDirectories(
      bundleRoot: null,
      workdir: defaultWorkdir,
      defaultDownloadDirectory: defaultDownloads,
      mode: WorkDirMode.installedDefault,
    );
  }
}
