// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(windows)]
use std::os::windows::fs::FileExt;

use crate::error::CoreError;

/// Storage allocation policy for temporary download files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AllocationPolicy {
    /// Laisse le fichier croître à l'écriture (croissance dynamique —
    /// le nom « sparse » décrit l'intention : sur NTFS les plages non
    /// écrites ne consomment pas de clusters tant que le fichier n'est
    /// pas étendu par `set_len`; aucun marquage `FSCTL_SET_SPARSE` n'est
    /// réalisé).
    #[default]
    SparseIfSupported,
    Preallocate,
    None,
}

/// Compteurs d'E/S positionnées d'un [`PartFile`] — instrumentation du
/// pipeline disque (invariant cible : `pread_bytes ≈ 0` en phase de
/// téléchargement, `pwrite_bytes == total_size`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartFileIoStats {
    /// Nombre d'appels de lecture positionnée (`pread`/`seek_read`).
    pub read_ops: u64,
    /// Octets relus depuis le disque.
    pub read_bytes: u64,
    /// Nombre d'appels d'écriture positionnée (`pwrite`/`seek_write`).
    pub write_ops: u64,
    /// Octets écrits.
    pub write_bytes: u64,
    /// Nombre de `sync_data` réels (fsync batché).
    pub sync_ops: u64,
}

/// Handle to a physical temporary download file (`.part`).
pub struct PartFile {
    file: File,
    path: PathBuf,
    total_size: u64,
    io_stats: PartFileIoStats,
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
            io_stats: PartFileIoStats::default(),
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

    /// Compteurs d'E/S cumulés depuis l'ouverture du handle.
    pub fn io_stats(&self) -> PartFileIoStats {
        self.io_stats
    }

    /// Écriture positionnée (`pwrite`/`seek_write`, un seul appel système)
    /// — pas de `seek` mutable sur le descripteur partagé.
    pub fn write_segment(&mut self, offset: u64, data: &[u8]) -> Result<(), CoreError> {
        if offset + data.len() as u64 > self.total_size {
            return Err(CoreError::InvalidRange {
                start: offset,
                end: offset + data.len() as u64,
                reason: "segment exceeds file total size boundary",
            });
        }

        positioned_write(&self.file, offset, data)?;
        self.io_stats.write_ops += 1;
        self.io_stats.write_bytes += data.len() as u64;
        Ok(())
    }

    /// Flushes unwritten buffers to physical disk.
    pub fn flush(&mut self) -> Result<(), CoreError> {
        self.file.sync_data()?;
        self.io_stats.sync_ops += 1;
        Ok(())
    }

    /// Lecture positionnée (`pread`/`seek_read`) d'une plage (vérification
    /// de hash, récupération AICH) — réservée aux chemins froids : la
    /// vérification de part en ligne utilise le buffer stagé en RAM.
    pub fn read_range(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, CoreError> {
        let mut buf = vec![0u8; len];
        positioned_read(&self.file, offset, &mut buf)?;
        self.io_stats.read_ops += 1;
        self.io_stats.read_bytes += len as u64;
        Ok(buf)
    }
}

/// Écriture positionnée complète — les API `seek_write`/`write_at` sont
/// autorisées à écrire moins que demandé : on boucle (équivalent
/// `write_all`) sans déplacer le curseur du descripteur.
#[cfg(windows)]
fn positioned_write(file: &File, offset: u64, data: &[u8]) -> Result<(), CoreError> {
    let mut written = 0usize;
    while written < data.len() {
        let n = file.seek_write(&data[written..], offset + written as u64)?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::WriteZero, "seek_write").into());
        }
        written += n;
    }
    Ok(())
}

#[cfg(unix)]
fn positioned_write(file: &File, offset: u64, data: &[u8]) -> Result<(), CoreError> {
    let mut written = 0usize;
    while written < data.len() {
        let n = file.write_at(&data[written..], offset + written as u64)?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::WriteZero, "write_at").into());
        }
        written += n;
    }
    Ok(())
}

/// Lecture positionnée complète (`read_exact`) — échec explicite si le
/// fichier ne contient pas assez de données (évite un buffer zero-paddé
/// silencieux qui fausserait une vérification de hash).
#[cfg(windows)]
fn positioned_read(file: &File, offset: u64, buf: &mut [u8]) -> Result<(), CoreError> {
    let mut read = 0usize;
    while read < buf.len() {
        let n = file.seek_read(&mut buf[read..], offset + read as u64)?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "seek_read").into());
        }
        read += n;
    }
    Ok(())
}

#[cfg(unix)]
fn positioned_read(file: &File, offset: u64, buf: &mut [u8]) -> Result<(), CoreError> {
    file.read_exact_at(buf, offset)?;
    Ok(())
}
