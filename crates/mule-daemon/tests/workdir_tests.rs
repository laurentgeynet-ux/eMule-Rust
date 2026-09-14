use mule_daemon::{
    WorkDirMode, WorkDirectoryResolver, ENV_DOWNLOADS_DIR, ENV_LEGACY_WORKDIR, ENV_WORKDIR,
    PORTABLE_FLAG_FILENAME,
};
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_workdir_priority_tier1_explicit_cli_wins_over_all() {
    let temp = tempdir().unwrap();
    let bundle_dir = temp.path().join("bundle");
    std::fs::create_dir_all(&bundle_dir).unwrap();

    // Create portable.flag in bundle
    File::create(bundle_dir.join(PORTABLE_FLAG_FILENAME)).unwrap();

    let exe_path = bundle_dir.join("mule-daemon.exe");
    let cli_override = PathBuf::from("C:\\Explicit\\CustomWorkDir");

    let mut env = HashMap::new();
    env.insert(ENV_WORKDIR.to_string(), "C:\\Env\\WorkDir".to_string());
    env.insert("LOCALAPPDATA".to_string(), "C:\\AppData\\Local".to_string());

    let resolved = WorkDirectoryResolver::resolve_with_context(
        Some(cli_override.clone()),
        None,
        Some(&exe_path),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::ExplicitCli);
    assert_eq!(resolved.workdir, cli_override);
    assert_eq!(
        resolved.default_download_dir,
        cli_override.join("downloads")
    );
}

#[test]
fn test_workdir_priority_tier1_explicit_downloads_dir_wins() {
    let temp = tempdir().unwrap();
    let bundle_dir = temp.path().join("bundle");
    std::fs::create_dir_all(&bundle_dir).unwrap();

    let exe_path = bundle_dir.join("mule-daemon.exe");
    let cli_workdir = PathBuf::from("C:\\Explicit\\CustomWorkDir");
    let cli_downloads = PathBuf::from("D:\\CustomDownloads");

    let mut env = HashMap::new();
    env.insert(
        ENV_DOWNLOADS_DIR.to_string(),
        "E:\\EnvDownloads".to_string(),
    );

    let resolved = WorkDirectoryResolver::resolve_with_context(
        Some(cli_workdir.clone()),
        Some(cli_downloads.clone()),
        Some(&exe_path),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::ExplicitCli);
    assert_eq!(resolved.workdir, cli_workdir);
    assert_eq!(resolved.default_download_dir, cli_downloads);
}

#[test]
fn test_workdir_priority_tier2_env_var_wins_over_portable_and_default() {
    let temp = tempdir().unwrap();
    let bundle_dir = temp.path().join("bundle");
    std::fs::create_dir_all(&bundle_dir).unwrap();

    // Create portable.flag in bundle
    File::create(bundle_dir.join(PORTABLE_FLAG_FILENAME)).unwrap();

    let exe_path = bundle_dir.join("mule-daemon.exe");

    let mut env = HashMap::new();
    env.insert(ENV_WORKDIR.to_string(), "D:\\Test\\EnvDir".to_string());
    env.insert("LOCALAPPDATA".to_string(), "C:\\AppData\\Local".to_string());

    let resolved = WorkDirectoryResolver::resolve_with_context(
        None,
        None,
        Some(&exe_path),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::Environment);
    assert_eq!(resolved.workdir, PathBuf::from("D:\\Test\\EnvDir"));
    assert_eq!(
        resolved.default_download_dir,
        PathBuf::from("D:\\Test\\EnvDir\\downloads")
    );
}

#[test]
fn test_workdir_priority_tier2_legacy_mule_data_dir_fallback() {
    let temp = tempdir().unwrap();
    let bundle_dir = temp.path().join("bundle");
    std::fs::create_dir_all(&bundle_dir).unwrap();

    let exe_path = bundle_dir.join("mule-daemon.exe");

    let mut env = HashMap::new();
    env.insert(
        ENV_LEGACY_WORKDIR.to_string(),
        "D:\\Test\\LegacyDir".to_string(),
    );
    env.insert("LOCALAPPDATA".to_string(), "C:\\AppData\\Local".to_string());

    let resolved = WorkDirectoryResolver::resolve_with_context(
        None,
        None,
        Some(&exe_path),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::Environment);
    assert_eq!(resolved.workdir, PathBuf::from("D:\\Test\\LegacyDir"));
    assert_eq!(
        resolved.default_download_dir,
        PathBuf::from("D:\\Test\\LegacyDir\\downloads")
    );
}

#[test]
fn test_workdir_priority_tier3_portable_flag_adjacent_to_exe() {
    let temp = tempdir().unwrap();
    let bundle_dir = temp.path().join("bundle");
    std::fs::create_dir_all(&bundle_dir).unwrap();

    // Create portable.flag adjacent to exe
    File::create(bundle_dir.join(PORTABLE_FLAG_FILENAME)).unwrap();

    let exe_path = bundle_dir.join("EmuleSion.exe");

    let mut env = HashMap::new();
    env.insert("LOCALAPPDATA".to_string(), "C:\\AppData\\Local".to_string());

    let resolved = WorkDirectoryResolver::resolve_with_context(
        None,
        None,
        Some(&exe_path),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::Portable);
    assert_eq!(resolved.workdir, bundle_dir.join("data"));
    assert_eq!(resolved.default_download_dir, bundle_dir.join("downloads"));
    assert_eq!(resolved.bundle_root, Some(bundle_dir));
}

#[test]
fn test_workdir_priority_tier3_portable_flag_in_bundle_root_parent_of_bin() {
    let temp = tempdir().unwrap();
    let bundle_root = temp.path().join("EmuleSion-Portable");
    let bin_dir = bundle_root.join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();

    // portable.flag in bundle_root, while daemon exe is in bundle_root/bin/
    File::create(bundle_root.join(PORTABLE_FLAG_FILENAME)).unwrap();

    let daemon_exe = bin_dir.join("mule-daemon.exe");

    let mut env = HashMap::new();
    env.insert("LOCALAPPDATA".to_string(), "C:\\AppData\\Local".to_string());

    let resolved = WorkDirectoryResolver::resolve_with_context(
        None,
        None,
        Some(&daemon_exe),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::Portable);
    assert_eq!(resolved.workdir, bundle_root.join("data"));
    assert_eq!(resolved.default_download_dir, bundle_root.join("downloads"));
    assert_eq!(resolved.bundle_root, Some(bundle_root));
}

#[test]
fn test_workdir_priority_tier4_installed_default_when_no_flags_or_overrides() {
    let temp = tempdir().unwrap();
    let app_dir = temp.path().join("program_files");
    std::fs::create_dir_all(&app_dir).unwrap();

    // NO portable.flag
    let exe_path = app_dir.join("EmuleSion.exe");

    let mut env = HashMap::new();
    env.insert(
        "LOCALAPPDATA".to_string(),
        "C:\\Users\\QA\\AppData\\Local".to_string(),
    );
    env.insert("USERPROFILE".to_string(), "C:\\Users\\QA".to_string());

    let resolved = WorkDirectoryResolver::resolve_with_context(
        None,
        None,
        Some(&exe_path),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::InstalledDefault);
    #[cfg(target_os = "windows")]
    {
        assert_eq!(
            resolved.workdir,
            PathBuf::from("C:\\Users\\QA\\AppData\\Local\\EmuleSion")
        );
        assert_eq!(
            resolved.default_download_dir,
            PathBuf::from("C:\\Users\\QA\\Downloads\\EmuleSion")
        );
    }
}

#[test]
fn test_workdir_non_bin_subfolder_does_not_traverse_parent_for_portable_flag() {
    let temp = tempdir().unwrap();
    let root_dir = temp.path().join("SomeApp");
    let other_sub_dir = root_dir.join("other_folder");
    std::fs::create_dir_all(&other_sub_dir).unwrap();

    // portable.flag in root_dir, but exe is in other_folder (which is not "bin")
    File::create(root_dir.join(PORTABLE_FLAG_FILENAME)).unwrap();

    let exe = other_sub_dir.join("tool.exe");

    let mut env = HashMap::new();
    env.insert(
        "LOCALAPPDATA".to_string(),
        "C:\\Users\\QA\\AppData\\Local".to_string(),
    );

    let resolved = WorkDirectoryResolver::resolve_with_context(
        None,
        None,
        Some(&exe),
        |k| env.get(k).cloned(),
    );

    // Because other_folder != "bin", it must NOT pick up parent's portable.flag
    assert_eq!(resolved.mode, WorkDirMode::InstalledDefault);
}
