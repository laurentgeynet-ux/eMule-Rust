// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::CoreError;

/// Strategy to follow if target destination file already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollisionStrategy {
    #[default]
    FailIfExists,
    Overwrite,
    /// eMule `CPartFile::PerformFileComplete` semantics: never overwrite an
    /// existing destination — generate `name(1).ext`, `name(2).ext`, …
    /// continuing numbering from an existing `(N)` suffix when present.
    Rename,
}

/// Atomically finalizes a completed download from its `.part` file to a
/// destination path. Returns the path actually written (identical to
/// `target_path` unless `strategy` is [`CollisionStrategy::Rename`] and a
/// collision had to be resolved).
pub fn finalize_file(
    temp_path: &Path,
    target_path: &Path,
    strategy: CollisionStrategy,
) -> Result<PathBuf, CoreError> {
    let resolved = match strategy {
        CollisionStrategy::FailIfExists if target_path.exists() => {
            return Err(CoreError::TargetFileExists(target_path.to_path_buf()));
        }
        CollisionStrategy::Rename => resolve_collision_path(target_path),
        _ => target_path.to_path_buf(),
    };

    if let Some(parent) = resolved.parent() {
        fs::create_dir_all(parent)?;
    }

    // Try atomic rename first
    if fs::rename(temp_path, &resolved).is_err() {
        // Fallback to copy + remove across filesystem boundaries
        fs::copy(temp_path, &resolved)?;
        let _ = fs::remove_file(temp_path);
    }

    Ok(resolved)
}

/// eMule `PerformFileComplete` collision naming: if `target` exists, try
/// `stem(1).ext`, `stem(2).ext`, … When the file stem already ends with
/// `(N)` (all-digits or empty, e.g. `file(3).mkv`), numbering continues at
/// N+1 on the stripped stem. Extensionless files yield `stem(N)` (eMule
/// emits `stem(N).` — a trailing dot Windows folds away anyway).
fn resolve_collision_path(target: &Path) -> PathBuf {
    if !target.exists() {
        return target.to_path_buf();
    }

    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let (base, ext) = match file_name.rfind('.') {
        Some(i) => (&file_name[..i], &file_name[i + 1..]),
        None => (file_name, ""),
    };

    // Existing `(N)` suffix: `file(3).mkv` colliding restarts at `file(4).mkv`.
    let mut stem = base;
    let mut namecount: u64 = 0;
    if base.ends_with(')') {
        if let Some(open) = base.rfind('(') {
            let digits = &base[open + 1..base.len() - 1];
            if digits.bytes().all(|b| b.is_ascii_digit()) {
                namecount = digits.parse().unwrap_or(0);
                stem = &base[..open];
            }
        }
    }

    loop {
        namecount += 1;
        let candidate_name = if ext.is_empty() {
            format!("{stem}({namecount})")
        } else {
            format!("{stem}({namecount}).{ext}")
        };
        let candidate = target.with_file_name(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
    }
}
