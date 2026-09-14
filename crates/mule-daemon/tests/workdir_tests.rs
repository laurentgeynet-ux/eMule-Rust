use mule_daemon::{
    WorkDirMode, WorkDirectoryResolver, ENV_LEGACY_WORKDIR, ENV_WORKDIR, PORTABLE_FLAG_FILENAME,
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
        Some(&exe_path),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::ExplicitCli);
    assert_eq!(resolved.path, cli_override);
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
        Some(&exe_path),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::Environment);
    assert_eq!(resolved.path, PathBuf::from("D:\\Test\\EnvDir"));
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
        Some(&exe_path),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::Environment);
    assert_eq!(resolved.path, PathBuf::from("D:\\Test\\LegacyDir"));
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
        Some(&exe_path),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::Portable);
    assert_eq!(resolved.path, bundle_dir.join("data"));
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
        Some(&daemon_exe),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::Portable);
    assert_eq!(resolved.path, bundle_root.join("data"));
}

#[test]
fn test_workdir_priority_tier4_installed_default_when_no_flags_or_overrides() {
    let temp = tempdir().unwrap();
    let app_dir = temp.path().join("program_files");
    std::fs::create_dir_all(&app_dir).unwrap();

    // NO portable.flag
    let exe_path = app_dir.join("EmuleSion.exe");

    let mut env = HashMap::new();
    env.insert("LOCALAPPDATA".to_string(), "C:\\Users\\QA\\AppData\\Local".to_string());

    let resolved = WorkDirectoryResolver::resolve_with_context(
        None,
        Some(&exe_path),
        |k| env.get(k).cloned(),
    );

    assert_eq!(resolved.mode, WorkDirMode::InstalledDefault);
    #[cfg(target_os = "windows")]
    {
        assert_eq!(
            resolved.path,
            PathBuf::from("C:\\Users\\QA\\AppData\\Local\\EmuleSion")
        );
    }
}
