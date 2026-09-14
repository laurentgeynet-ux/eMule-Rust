import 'dart:io';

enum WorkDirMode {
  explicitCli,
  environment,
  portable,
  installedDefault,
}

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
  static const String envWorkdir = 'EMULE_SION_WORKDIR';
  static const String envLegacyWorkdir = 'MULE_DATA_DIR';
  static const String envDownloadsDir = 'EMULE_SION_DOWNLOADS_DIR';

  /// Resolves the runtime directories using the standard 4-tier cascade:
  /// 1. [customDir] (explicit CLI override), [customDownloadsDir]
  /// 2. [environment] EMULE_SION_WORKDIR (or legacy MULE_DATA_DIR)
  /// 3. portable.flag adjacent to executable or bundle root -> `<bundle_root>/data` & `<bundle_root>/downloads`
  /// 4. System default (%LOCALAPPDATA%\EmuleSion on Windows, ~/.emule_sion on Unix; %USERPROFILE%\Downloads\EmuleSion)
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
      final envDownloads = env[envDownloadsDir]?.trim();
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

    // 2. Environment variable: EMULE_SION_WORKDIR (or MULE_DATA_DIR)
    final envVal = env[envWorkdir]?.trim();
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
        defaultWorkdir = Directory('$localAppData\\EmuleSion');
      } else if (appData != null && appData.isNotEmpty) {
        defaultWorkdir = Directory('$appData\\EmuleSion');
      } else {
        defaultWorkdir = Directory('.mule_data');
      }

      final userProfile = env['USERPROFILE']?.trim();
      if (userProfile != null && userProfile.isNotEmpty) {
        defaultDownloads = explicitDownloads ?? Directory('$userProfile\\Downloads\\EmuleSion');
      } else {
        defaultDownloads = explicitDownloads ?? Directory('${defaultWorkdir.path}\\downloads');
      }
    } else {
      final home = env['HOME']?.trim();
      if (home != null && home.isNotEmpty) {
        defaultWorkdir = Directory('$home/.emule_sion');
        defaultDownloads = explicitDownloads ?? Directory('$home/Downloads/EmuleSion');
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
