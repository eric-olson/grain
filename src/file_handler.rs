use std::fs::File;
use std::path::PathBuf;

use memmap2::Mmap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FileError {
    #[error("Failed to open file: {0}")]
    Open(#[from] std::io::Error),

    #[error("Failed to mmap file: {0}")]
    Mmap(std::io::Error),
}

pub struct MappedFile {
    mmap: Mmap,
    path: PathBuf,
}

impl MappedFile {
    pub fn open(path: PathBuf) -> Result<Self, FileError> {
        let file = File::open(&path)?;
        // SAFETY: The file is opened read-only and we hold no mutable references.
        // The mapping may become invalid if the file is truncated externally,
        // but that is an accepted risk for this use case.
        let mmap = unsafe { Mmap::map(&file) }.map_err(FileError::Mmap)?;
        Ok(Self { mmap, path })
    }

    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    pub fn get_range(&self, offset: usize, len: usize) -> &[u8] {
        let end = (offset + len).min(self.mmap.len());
        let start = offset.min(end);
        &self.mmap[start..end]
    }

    pub fn name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    }

    pub fn data(&self) -> &[u8] {
        &self.mmap
    }
}
