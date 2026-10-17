// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:emulerust/services/work_directory_resolver.dart';

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

      final fakeExe = File('${bundleDir.path}/eMuleRust.exe');
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

    test('Tier 2: EMULERUST_WORKDIR environment variable overrides portable.flag and default', () {
      final bundleDir = Directory('${tempDir.path}/bundle')..createSync();
      File('${bundleDir.path}/portable.flag').createSync();

      final fakeExe = File('${bundleDir.path}/eMuleRust.exe');
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

    test('Tier 2: deprecated EMULE_SION_WORKDIR alias still resolves', () {
      final bundleDir = Directory('${tempDir.path}/bundle')..createSync();
      File('${bundleDir.path}/portable.flag').createSync();

      final fakeExe = File('${bundleDir.path}/eMuleRust.exe');
      final sionPath = '${tempDir.path}/from_sion_env';

      final env = {
        WorkDirectoryResolver.envSionWorkdir: sionPath,
        'LOCALAPPDATA': 'C:\\Users\\QA\\AppData\\Local',
      };

      final resolved = WorkDirectoryResolver.resolve(
        executableFile: fakeExe,
        environment: env,
      );

      expect(resolved.mode, WorkDirMode.environment);
      expect(resolved.directory.path, sionPath);
    });

    test('Tier 2: EMULERUST_WORKDIR takes precedence over deprecated EMULE_SION_WORKDIR', () {
      final bundleDir = Directory('${tempDir.path}/bundle')..createSync();
      File('${bundleDir.path}/portable.flag').createSync();

      final fakeExe = File('${bundleDir.path}/eMuleRust.exe');
      final envPath = '${tempDir.path}/from_env';

      final env = {
        WorkDirectoryResolver.envWorkdir: envPath,
        WorkDirectoryResolver.envSionWorkdir: '${tempDir.path}/from_sion_env',
        'LOCALAPPDATA': 'C:\\Users\\QA\\AppData\\Local',
      };

      final resolved = WorkDirectoryResolver.resolve(
        executableFile: fakeExe,
        environment: env,
      );

      expect(resolved.mode, WorkDirMode.environment);
      expect(resolved.directory.path, envPath);
    });

    test('Tier 2: legacy MULE_DATA_DIR fallback works when EMULERUST_WORKDIR is absent', () {
      final bundleDir = Directory('${tempDir.path}/bundle')..createSync();
      File('${bundleDir.path}/portable.flag').createSync();

      final fakeExe = File('${bundleDir.path}/eMuleRust.exe');
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

      final fakeExe = File('${bundleDir.path}/eMuleRust.exe');

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
      final fakeExe = File('${appDir.path}/eMuleRust.exe');

      final env = {
        'LOCALAPPDATA': 'C:\\Users\\QA\\AppData\\Local',
      };

      final resolved = WorkDirectoryResolver.resolve(
        executableFile: fakeExe,
        environment: env,
      );

      expect(resolved.mode, WorkDirMode.installedDefault);
      if (Platform.isWindows) {
        expect(resolved.directory.path, 'C:\\Users\\QA\\AppData\\Local\\eMuleRust');
      }
    });

    test('Strict bounding: non-bin subfolder does not traverse parent for portable.flag', () {
      final rootDir = Directory('${tempDir.path}/app_root')..createSync();
      final otherDir = Directory('${rootDir.path}/sub_tools')..createSync();
      File('${rootDir.path}/portable.flag').createSync();

      final fakeExe = File('${otherDir.path}/tool.exe');

      final env = {
        'LOCALAPPDATA': 'C:\\Users\\QA\\AppData\\Local',
      };

      final resolved = WorkDirectoryResolver.resolve(
        executableFile: fakeExe,
        environment: env,
      );

      // Should NOT pick up parent because sub_tools != bin
      expect(resolved.mode, WorkDirMode.installedDefault);
    });

    test('Portable mode resolves defaultDownloadDirectory to <bundleRoot>/downloads', () {
      final bundleDir = Directory('${tempDir.path}/bundle_downloads')..createSync();
      File('${bundleDir.path}/portable.flag').createSync();
      final fakeExe = File('${bundleDir.path}/eMuleRust.exe');

      final resolved = WorkDirectoryResolver.resolve(
        executableFile: fakeExe,
        environment: {},
      );

      expect(resolved.mode, WorkDirMode.portable);
      expect(resolved.workdir.path, '${bundleDir.path}/data');
      expect(resolved.defaultDownloadDirectory.path, '${bundleDir.path}/downloads');
      expect(resolved.bundleRoot?.path, bundleDir.path);
    });

    test('Explicit customDownloadsDir overrides defaultDownloadDirectory', () {
      final bundleDir = Directory('${tempDir.path}/bundle_custom_dl')..createSync();
      File('${bundleDir.path}/portable.flag').createSync();
      final fakeExe = File('${bundleDir.path}/eMuleRust.exe');
      final customDl = Directory('${tempDir.path}/my_special_downloads');

      final resolved = WorkDirectoryResolver.resolve(
        customDownloadsDir: customDl,
        executableFile: fakeExe,
        environment: {},
      );

      expect(resolved.defaultDownloadDirectory.path, customDl.path);
    });

    test('InstalledDefault Windows resolves defaultDownloadDirectory to %USERPROFILE%\\Downloads\\eMuleRust', () {
      final appDir = Directory('${tempDir.path}/installed_windows_dl')..createSync();
      final fakeExe = File('${appDir.path}/eMuleRust.exe');

      final env = {
        'LOCALAPPDATA': 'C:\\Users\\QA\\AppData\\Local',
        'USERPROFILE': 'C:\\Users\\QA',
      };

      final resolved = WorkDirectoryResolver.resolve(
        executableFile: fakeExe,
        environment: env,
      );

      expect(resolved.mode, WorkDirMode.installedDefault);
      if (Platform.isWindows) {
        expect(resolved.workdir.path, 'C:\\Users\\QA\\AppData\\Local\\eMuleRust');
        expect(resolved.defaultDownloadDirectory.path, 'C:\\Users\\QA\\Downloads\\eMuleRust');
      }
    });
  });
}
