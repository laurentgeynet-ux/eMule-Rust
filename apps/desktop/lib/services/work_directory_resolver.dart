import 'dart:io';

enum WorkDirMode {
  explicitCli,
  environment,
  portable,
  installedDefault,
}

class ResolvedWorkDir {
  final Directory directory;
  final WorkDirMode mode;

  const ResolvedWorkDir({
    required this.directory,
    required this.mode,
  });
}

class WorkDirectoryResolver {
  static const String portableFlagFilename = 'portable.flag';
  static const String envWorkdir = 'EMULE_SION_WORKDIR';
  static const String envLegacyWorkdir = 'MULE_DATA_DIR';

  /// Resolves the work directory using the standard 4-tier cascade:
  /// 1. [customDir] (explicit CLI override)
  /// 2. [environment] EMULE_SION_WORKDIR (or legacy MULE_DATA_DIR)
  /// 3. portable.flag adjacent to executable or bundle root
  /// 4. System default (%LOCALAPPDATA%\EmuleSion on Windows, ~/.emule_sion on Unix)
  static ResolvedWorkDir resolve({
    Directory? customDir,
    File? executableFile,
    Map<String, String>? environment,
  }) {
    // 1. Explicit override
    if (customDir != null) {
      return ResolvedWorkDir(
        directory: customDir,
        mode: WorkDirMode.explicitCli,
      );
    }

    final env = environment ?? Platform.environment;

    // 2. Environment variable: EMULE_SION_WORKDIR (or MULE_DATA_DIR)
    final envVal = env[envWorkdir]?.trim();
    if (envVal != null && envVal.isNotEmpty) {
      return ResolvedWorkDir(
        directory: Directory(envVal),
        mode: WorkDirMode.environment,
      );
    }

    final legacyVal = env[envLegacyWorkdir]?.trim();
    if (legacyVal != null && legacyVal.isNotEmpty) {
      return ResolvedWorkDir(
        directory: Directory(legacyVal),
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
      return ResolvedWorkDir(
        directory: Directory('${bundleRoot.path}/data'),
        mode: WorkDirMode.portable,
      );
    }

    // 4. Installed default
    if (Platform.isWindows) {
      final localAppData = env['LOCALAPPDATA']?.trim();
      if (localAppData != null && localAppData.isNotEmpty) {
        return ResolvedWorkDir(
          directory: Directory('$localAppData\\EmuleSion'),
          mode: WorkDirMode.installedDefault,
        );
      }
      final appData = env['APPDATA']?.trim();
      if (appData != null && appData.isNotEmpty) {
        return ResolvedWorkDir(
          directory: Directory('$appData\\EmuleSion'),
          mode: WorkDirMode.installedDefault,
        );
      }
    } else {
      final home = env['HOME']?.trim();
      if (home != null && home.isNotEmpty) {
        return ResolvedWorkDir(
          directory: Directory('$home/.emule_sion'),
          mode: WorkDirMode.installedDefault,
        );
      }
    }

    return ResolvedWorkDir(
      directory: Directory('.mule_data'),
      mode: WorkDirMode.installedDefault,
    );
  }
}
