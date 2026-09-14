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

    // 3. Portable mode: check if portable.flag exists
    final exe = executableFile ?? File(Platform.resolvedExecutable);
    final exeDir = exe.parent;

    // Check adjacent to exe
    final adjacentFlag = File('${exeDir.path}/$portableFlagFilename');
    if (adjacentFlag.existsSync()) {
      return ResolvedWorkDir(
        directory: Directory('${exeDir.path}/data'),
        mode: WorkDirMode.portable,
      );
    }

    // Check in parent directory (if binary is in bin/)
    final parentDir = exeDir.parent;
    final parentFlag = File('${parentDir.path}/$portableFlagFilename');
    if (parentFlag.existsSync()) {
      return ResolvedWorkDir(
        directory: Directory('${parentDir.path}/data'),
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
