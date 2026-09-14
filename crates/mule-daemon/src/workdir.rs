use std::path::{Path, PathBuf};

pub const PORTABLE_FLAG_FILENAME: &str = "portable.flag";
pub const ENV_WORKDIR: &str = "EMULE_SION_WORKDIR";
pub const ENV_LEGACY_WORKDIR: &str = "MULE_DATA_DIR";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkDirMode {
    ExplicitCli,
    Environment,
    Portable,
    InstalledDefault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedWorkDir {
    pub path: PathBuf,
    pub mode: WorkDirMode,
}

pub struct WorkDirectoryResolver;

impl WorkDirectoryResolver {
    /// Resolves the workdir using the deterministic priority cascade:
    /// 1. `--workdir <path>` (explicit CLI argument)
    /// 2. `EMULE_SION_WORKDIR` (env var, with legacy `MULE_DATA_DIR` fallback)
    /// 3. `portable.flag` adjacent to executable or bundle root (parent of `bin/`) -> `<bundle_root>/data`
    /// 4. System default (%LOCALAPPDATA%\EmuleSion on Windows, ~/.emule_sion on Unix)
    pub fn resolve(cli_arg: Option<PathBuf>) -> ResolvedWorkDir {
        Self::resolve_with_context(
            cli_arg,
            std::env::current_exe().ok().as_deref(),
            |k| std::env::var(k).ok(),
        )
    }

    /// Pure resolver method with dependency injection for environment and exe location,
    /// enabling 100% deterministic unit testing of all 4 cascade tiers.
    pub fn resolve_with_context<F>(
        cli_arg: Option<PathBuf>,
        exe_path: Option<&Path>,
        get_env: F,
    ) -> ResolvedWorkDir
    where
        F: Fn(&str) -> Option<String>,
    {
        // 1. Explicit CLI argument
        if let Some(arg) = cli_arg {
            if !arg.as_os_str().is_empty() {
                return ResolvedWorkDir {
                    path: arg,
                    mode: WorkDirMode::ExplicitCli,
                };
            }
        }

        // 2. Environment variable: EMULE_SION_WORKDIR (or legacy MULE_DATA_DIR)
        if let Some(env_val) = get_env(ENV_WORKDIR) {
            let trimmed = env_val.trim();
            if !trimmed.is_empty() {
                return ResolvedWorkDir {
                    path: PathBuf::from(trimmed),
                    mode: WorkDirMode::Environment,
                };
            }
        }
        if let Some(legacy_val) = get_env(ENV_LEGACY_WORKDIR) {
            let trimmed = legacy_val.trim();
            if !trimmed.is_empty() {
                return ResolvedWorkDir {
                    path: PathBuf::from(trimmed),
                    mode: WorkDirMode::Environment,
                };
            }
        }

        // 3. Portable mode: check if portable.flag exists
        if let Some(exe) = exe_path {
            if let Some(exe_dir) = exe.parent() {
                // Check adjacent to exe
                let adjacent_flag = exe_dir.join(PORTABLE_FLAG_FILENAME);
                if adjacent_flag.exists() {
                    return ResolvedWorkDir {
                        path: exe_dir.join("data"),
                        mode: WorkDirMode::Portable,
                    };
                }

                // Check in parent directory (if binary is in bin/)
                if let Some(parent_dir) = exe_dir.parent() {
                    let parent_flag = parent_dir.join(PORTABLE_FLAG_FILENAME);
                    if parent_flag.exists() {
                        return ResolvedWorkDir {
                            path: parent_dir.join("data"),
                            mode: WorkDirMode::Portable,
                        };
                    }
                }
            }
        }

        // 4. Installed default: %LOCALAPPDATA%\EmuleSion (or %APPDATA%\EmuleSion), ~/.emule_sion on Unix
        let default_path = Self::resolve_system_default(&get_env);
        ResolvedWorkDir {
            path: default_path,
            mode: WorkDirMode::InstalledDefault,
        }
    }

    fn resolve_system_default<F>(get_env: &F) -> PathBuf
    where
        F: Fn(&str) -> Option<String>,
    {
        #[cfg(target_os = "windows")]
        {
            if let Some(local_appdata) = get_env("LOCALAPPDATA") {
                let trimmed = local_appdata.trim();
                if !trimmed.is_empty() {
                    return PathBuf::from(trimmed).join("EmuleSion");
                }
            }
            if let Some(appdata) = get_env("APPDATA") {
                let trimmed = appdata.trim();
                if !trimmed.is_empty() {
                    return PathBuf::from(trimmed).join("EmuleSion");
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Some(home) = get_env("HOME") {
                let trimmed = home.trim();
                if !trimmed.is_empty() {
                    return PathBuf::from(trimmed).join(".emule_sion");
                }
            }
        }

        PathBuf::from(".").join(".mule_data")
    }
}
