use memmap2::Mmap;
use std::fs::File;
use std::path::PathBuf;

pub struct MappedFile {
    mmap: Mmap,
    path: PathBuf,
}

impl MappedFile {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let file = File::open(&path).map_err(|e| format!("Failed to open file: {e}"))?;
        let mmap = unsafe { Mmap::map(&file) }.map_err(|e| format!("Failed to mmap file: {e}"))?;
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
