// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::error::CoreError;

/// Storage allocation policy for temporary download files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AllocationPolicy {
    #[default]
    SparseIfSupported,
    Preallocate,
    None,
}

/// Handle to a physical temporary download file (`.part`).
pub struct PartFile {
    file: File,
    path: PathBuf,
    total_size: u64,
}

impl PartFile {
    /// Opens or creates a `.part` file with the specified allocation policy.
    pub fn open_or_create<P: AsRef<Path>>(
        path: P,
        total_size: u64,
        policy: AllocationPolicy,
    ) -> Result<Self, CoreError> {
        let path_buf = path.as_ref().to_path_buf();
        if let Some(parent) = path_buf.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path_buf)?;

        let metadata = file.metadata()?;
        if metadata.len() > total_size {
            return Err(CoreError::InvalidRange {
                start: 0,
                end: metadata.len(),
                reason: "part file on disk exceeds total file size boundary",
            });
        }

        let part = Self {
            file,
            path: path_buf,
            total_size,
        };

        match policy {
            AllocationPolicy::Preallocate => {
                part.file.set_len(total_size)?;
            }
            AllocationPolicy::SparseIfSupported | AllocationPolicy::None => {
                // Keep file size as-is or grow dynamically on write
            }
        }

        Ok(part)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    /// Writes a data chunk at an arbitrary byte offset.
    pub fn write_segment(&mut self, offset: u64, data: &[u8]) -> Result<(), CoreError> {
        if offset + data.len() as u64 > self.total_size {
            return Err(CoreError::InvalidRange {
                start: offset,
                end: offset + data.len() as u64,
                reason: "segment exceeds file total size boundary",
            });
        }

        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(data)?;
        Ok(())
    }

    /// Flushes unwritten buffers to physical disk.
    pub fn flush(&mut self) -> Result<(), CoreError> {
        self.file.sync_data()?;
        Ok(())
    }

    /// Reads a byte range from the temporary file (e.g. for hash verification).
    pub fn read_range(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, CoreError> {
        self.file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; len];
        self.file.read_exact(&mut buf)?;
        Ok(buf)
    }
}
