// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use crate::error::DaemonError;

#[derive(Debug)]
pub struct DirectoryLock {
    file: File,
    lock_path: PathBuf,
}

impl DirectoryLock {
    pub fn acquire(data_dir: &Path) -> Result<Self, DaemonError> {
        std::fs::create_dir_all(data_dir)?;
        let lock_path = data_dir.join("mule.lock");

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;

        match file.try_lock() {
            Ok(()) => Ok(Self { file, lock_path }),
            Err(_) => Err(DaemonError::AlreadyRunning(data_dir.to_path_buf())),
        }
    }

    pub fn path(&self) -> &Path {
        &self.lock_path
    }
}

impl Drop for DirectoryLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
