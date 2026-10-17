// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use std::path::{Path, PathBuf};

pub const PORTABLE_FLAG_FILENAME: &str = "portable.flag";
pub const ENV_WORKDIR: &str = "EMULERUST_WORKDIR";
/// Alias déprécié (avant le renommage eMuleRust), honoré avant `MULE_DATA_DIR`.
pub const ENV_SION_WORKDIR: &str = "EMULE_SION_WORKDIR";
pub const ENV_LEGACY_WORKDIR: &str = "MULE_DATA_DIR";
pub const ENV_DOWNLOADS_DIR: &str = "EMULERUST_DOWNLOADS_DIR";
/// Alias déprécié (avant le renommage eMuleRust).
pub const ENV_SION_DOWNLOADS_DIR: &str = "EMULE_SION_DOWNLOADS_DIR";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkDirMode {
    ExplicitCli,
    Environment,
    Portable,
    InstalledDefault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDirectories {
    pub bundle_root: Option<PathBuf>,
    pub workdir: PathBuf,
    pub default_download_dir: PathBuf,
    pub mode: WorkDirMode,
    /// Compatibility field identical to `workdir`.
    pub path: PathBuf,
}

pub type ResolvedWorkDir = ResolvedDirectories;

pub struct WorkDirectoryResolver;

impl WorkDirectoryResolver {
    /// Resolves the runtime directories using the deterministic priority cascade:
    /// 1. `--workdir <path>` (explicit CLI argument)
    /// 2. `EMULERUST_WORKDIR` (env var, with deprecated `EMULE_SION_WORKDIR`
    ///    and legacy `MULE_DATA_DIR` fallbacks)
    /// 3. `portable.flag` adjacent to executable or bundle root -> `<bundle_root>/data` & `<bundle_root>/downloads`
    /// 4. System default (%LOCALAPPDATA%\eMuleRust on Windows, ~/.emulerust on Unix; downloads in %USERPROFILE%\Downloads\eMuleRust)
    pub fn resolve(
        cli_workdir: Option<PathBuf>,
        cli_downloads_dir: Option<PathBuf>,
    ) -> ResolvedDirectories {
        Self::resolve_with_context(
            cli_workdir,
            cli_downloads_dir,
            std::env::current_exe().ok().as_deref(),
            |k| std::env::var(k).ok(),
        )
    }

    /// Pure resolver method with dependency injection for environment and exe location,
    /// enabling 100% deterministic unit testing of all cascade tiers.
    pub fn resolve_with_context<F>(
        cli_workdir: Option<PathBuf>,
        cli_downloads_dir: Option<PathBuf>,
        exe_path: Option<&Path>,
        get_env: F,
    ) -> ResolvedDirectories
    where
        F: Fn(&str) -> Option<String>,
    {
        let explicit_download_dir = cli_downloads_dir
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| {
                get_env(ENV_DOWNLOADS_DIR)
                    .or_else(|| get_env(ENV_SION_DOWNLOADS_DIR))
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
            });

        // 1. Explicit CLI argument
        if let Some(arg) = cli_workdir {
            if !arg.as_os_str().is_empty() {
                let bundle_root = arg.parent().and_then(|parent| {
                    if parent.join(PORTABLE_FLAG_FILENAME).exists() {
                        Some(parent.to_path_buf())
                    } else {
                        None
                    }
                });

                let default_download_dir = explicit_download_dir.clone().unwrap_or_else(|| {
                    if let Some(root) = &bundle_root {
                        root.join("downloads")
                    } else {
                        arg.join("downloads")
                    }
                });

                return ResolvedDirectories {
                    bundle_root,
                    workdir: arg.clone(),
                    default_download_dir,
                    mode: WorkDirMode::ExplicitCli,
                    path: arg,
                };
            }
        }

        // 2. Environment variable: EMULERUST_WORKDIR (or deprecated
        //    EMULE_SION_WORKDIR, or legacy MULE_DATA_DIR)
        let env_workdir = get_env(ENV_WORKDIR)
            .or_else(|| get_env(ENV_SION_WORKDIR))
            .or_else(|| get_env(ENV_LEGACY_WORKDIR))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);

        if let Some(env_dir) = env_workdir {
            let bundle_root = env_dir.parent().and_then(|parent| {
                if parent.join(PORTABLE_FLAG_FILENAME).exists() {
                    Some(parent.to_path_buf())
                } else {
                    None
                }
            });

            let default_download_dir = explicit_download_dir.clone().unwrap_or_else(|| {
                if let Some(root) = &bundle_root {
                    root.join("downloads")
                } else {
                    env_dir.join("downloads")
                }
            });

            return ResolvedDirectories {
                bundle_root,
                workdir: env_dir.clone(),
                default_download_dir,
                mode: WorkDirMode::Environment,
                path: env_dir,
            };
        }

        // 3. Portable mode: strictly bounded bundle root detection
        if let Some(exe) = exe_path {
            if let Some(exe_dir) = exe.parent() {
                let is_bin = exe_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.eq_ignore_ascii_case("bin"))
                    .unwrap_or(false);

                let bundle_root = if is_bin {
                    exe_dir.parent().unwrap_or(exe_dir)
                } else {
                    exe_dir
                };

                let flag_path = bundle_root.join(PORTABLE_FLAG_FILENAME);
                if flag_path.exists() {
                    let workdir = bundle_root.join("data");
                    let default_download_dir = explicit_download_dir
                        .clone()
                        .unwrap_or_else(|| bundle_root.join("downloads"));

                    return ResolvedDirectories {
                        bundle_root: Some(bundle_root.to_path_buf()),
                        workdir: workdir.clone(),
                        default_download_dir,
                        mode: WorkDirMode::Portable,
                        path: workdir,
                    };
                }
            }
        }

        // 4. Installed default
        let default_workdir = Self::resolve_system_default_workdir(&get_env);
        let default_download_dir = explicit_download_dir.unwrap_or_else(|| {
            Self::resolve_system_default_downloads_dir(&get_env, &default_workdir)
        });

        ResolvedDirectories {
            bundle_root: None,
            workdir: default_workdir.clone(),
            default_download_dir,
            mode: WorkDirMode::InstalledDefault,
            path: default_workdir,
        }
    }

    fn resolve_system_default_workdir<F>(get_env: &F) -> PathBuf
    where
        F: Fn(&str) -> Option<String>,
    {
        #[cfg(target_os = "windows")]
        {
            if let Some(local_appdata) = get_env("LOCALAPPDATA") {
                let trimmed = local_appdata.trim();
                if !trimmed.is_empty() {
                    return PathBuf::from(trimmed).join("eMuleRust");
                }
            }
            if let Some(appdata) = get_env("APPDATA") {
                let trimmed = appdata.trim();
                if !trimmed.is_empty() {
                    return PathBuf::from(trimmed).join("eMuleRust");
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Some(home) = get_env("HOME") {
                let trimmed = home.trim();
                if !trimmed.is_empty() {
                    return PathBuf::from(trimmed).join(".emulerust");
                }
            }
        }

        PathBuf::from(".").join(".mule_data")
    }

    fn resolve_system_default_downloads_dir<F>(get_env: &F, fallback_workdir: &Path) -> PathBuf
    where
        F: Fn(&str) -> Option<String>,
    {
        #[cfg(target_os = "windows")]
        {
            if let Some(userprofile) = get_env("USERPROFILE") {
                let trimmed = userprofile.trim();
                if !trimmed.is_empty() {
                    return PathBuf::from(trimmed).join("Downloads").join("eMuleRust");
                }
            }
            if let Some(home) = get_env("HOME") {
                let trimmed = home.trim();
                if !trimmed.is_empty() {
                    return PathBuf::from(trimmed).join("Downloads").join("eMuleRust");
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Some(home) = get_env("HOME") {
                let trimmed = home.trim();
                if !trimmed.is_empty() {
                    return PathBuf::from(trimmed).join("Downloads").join("eMuleRust");
                }
            }
        }

        fallback_workdir.join("downloads")
    }
}
