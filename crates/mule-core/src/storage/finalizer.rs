use std::fs;
use std::path::Path;

use crate::error::CoreError;

/// Strategy to follow if target destination file already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollisionStrategy {
    #[default]
    FailIfExists,
    Overwrite,
}

/// Atomically finalizes a completed download from its `.part` file to destination path.
pub fn finalize_file(
    temp_path: &Path,
    target_path: &Path,
    strategy: CollisionStrategy,
) -> Result<(), CoreError> {
    if target_path.exists() && strategy == CollisionStrategy::FailIfExists {
        return Err(CoreError::TargetFileExists(target_path.to_path_buf()));
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Try atomic rename first
    if fs::rename(temp_path, target_path).is_err() {
        // Fallback to copy + remove across filesystem boundaries
        fs::copy(temp_path, target_path)?;
        let _ = fs::remove_file(temp_path);
    }

    Ok(())
}
