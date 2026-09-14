import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:desktop/services/work_directory_resolver.dart';

void main() {
  group('WorkDirectoryResolver Tests', () {
    late Directory tempDir;

    setUp(() async {
      tempDir = await Directory.systemTemp.createTemp('workdir_test_');
    });

    tearDown(() async {
      try {
        await tempDir.delete(recursive: true);
      } catch (_) {}
    });

    test('Tier 1: customDir explicit argument overrides all others', () {
      final bundleDir = Directory('${tempDir.path}/bundle')..createSync();
      File('${bundleDir.path}/portable.flag').createSync();

      final fakeExe = File('${bundleDir.path}/EmuleSion.exe');
      final explicitDir = Directory('${tempDir.path}/custom_dir');

      final env = {
        WorkDirectoryResolver.envWorkdir: 'D:\\Env\\WorkDir',
        'LOCALAPPDATA': 'C:\\Users\\QA\\AppData\\Local',
      };

      final resolved = WorkDirectoryResolver.resolve(
        customDir: explicitDir,
        executableFile: fakeExe,
        environment: env,
      );

      expect(resolved.mode, WorkDirMode.explicitCli);
      expect(resolved.directory.path, explicitDir.path);
    });

    test('Tier 2: EMULE_SION_WORKDIR environment variable overrides portable.flag and default', () {
      final bundleDir = Directory('${tempDir.path}/bundle')..createSync();
      File('${bundleDir.path}/portable.flag').createSync();

      final fakeExe = File('${bundleDir.path}/EmuleSion.exe');
      final envPath = '${tempDir.path}/from_env';

      final env = {
        WorkDirectoryResolver.envWorkdir: envPath,
        'LOCALAPPDATA': 'C:\\Users\\QA\\AppData\\Local',
      };

      final resolved = WorkDirectoryResolver.resolve(
        executableFile: fakeExe,
        environment: env,
      );

      expect(resolved.mode, WorkDirMode.environment);
      expect(resolved.directory.path, envPath);
    });

    test('Tier 2: legacy MULE_DATA_DIR fallback works when EMULE_SION_WORKDIR is absent', () {
      final bundleDir = Directory('${tempDir.path}/bundle')..createSync();
      File('${bundleDir.path}/portable.flag').createSync();

      final fakeExe = File('${bundleDir.path}/EmuleSion.exe');
      final legacyPath = '${tempDir.path}/from_legacy_env';

      final env = {
        WorkDirectoryResolver.envLegacyWorkdir: legacyPath,
        'LOCALAPPDATA': 'C:\\Users\\QA\\AppData\\Local',
      };

      final resolved = WorkDirectoryResolver.resolve(
        executableFile: fakeExe,
        environment: env,
      );

      expect(resolved.mode, WorkDirMode.environment);
      expect(resolved.directory.path, legacyPath);
    });

    test('Tier 3: portable.flag adjacent to executable resolves to <exe_dir>/data', () {
      final bundleDir = Directory('${tempDir.path}/bundle')..createSync();
      File('${bundleDir.path}/portable.flag').createSync();

      final fakeExe = File('${bundleDir.path}/EmuleSion.exe');

      final env = {
        'LOCALAPPDATA': 'C:\\Users\\QA\\AppData\\Local',
      };

      final resolved = WorkDirectoryResolver.resolve(
        executableFile: fakeExe,
        environment: env,
      );

      expect(resolved.mode, WorkDirMode.portable);
      expect(resolved.directory.path, '${bundleDir.path}/data');
    });

    test('Tier 3: portable.flag in bundle root when binary is inside bin/ resolves to <bundle>/data', () {
      final bundleDir = Directory('${tempDir.path}/bundle')..createSync();
      final binDir = Directory('${bundleDir.path}/bin')..createSync();
      File('${bundleDir.path}/portable.flag').createSync();

      final fakeExe = File('${binDir.path}/mule-daemon.exe');

      final env = {
        'LOCALAPPDATA': 'C:\\Users\\QA\\AppData\\Local',
      };

      final resolved = WorkDirectoryResolver.resolve(
        executableFile: fakeExe,
        environment: env,
      );

      expect(resolved.mode, WorkDirMode.portable);
      expect(resolved.directory.path, '${bundleDir.path}/data');
    });

    test('Tier 4: installed default %LOCALAPPDATA% is used when no flags or overrides exist', () {
      final appDir = Directory('${tempDir.path}/installed_app')..createSync();
      final fakeExe = File('${appDir.path}/EmuleSion.exe');

      final env = {
        'LOCALAPPDATA': 'C:\\Users\\QA\\AppData\\Local',
      };

      final resolved = WorkDirectoryResolver.resolve(
        executableFile: fakeExe,
        environment: env,
      );

      expect(resolved.mode, WorkDirMode.installedDefault);
      if (Platform.isWindows) {
        expect(resolved.directory.path, 'C:\\Users\\QA\\AppData\\Local\\EmuleSion');
      }
    });
  });
}
