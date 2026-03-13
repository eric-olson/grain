use std::collections::HashMap;

/// Chunk size for cached output data (aligned with checkpoint interval).
const CHUNK_SIZE: usize = 64 * 1024;

/// Maximum bytes to keep in the output cache.
const MAX_CACHE_BYTES: usize = 128 * 1024 * 1024;

/// Chunked output cache with fixed memory budget.
///
/// Caches processor output in fixed-size chunks. Evicts chunks farthest
/// from the current viewport position when the budget is exceeded.
pub struct OutputCache {
    chunks: HashMap<usize, Vec<u8>>,
    total_bytes: usize,
}

impl OutputCache {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            total_bytes: 0,
        }
    }

    /// Returns the fixed chunk size.
    pub fn chunk_size() -> usize {
        CHUNK_SIZE
    }

    /// Try to read cached data for the given output range.
    /// Returns None if any part of the range is not cached.
    pub fn get(&self, output_offset: usize, len: usize) -> Option<Vec<u8>> {
        if len == 0 {
            return Some(Vec::new());
        }

        let start_chunk = output_offset / CHUNK_SIZE;
        let end_chunk = (output_offset + len - 1) / CHUNK_SIZE;

        // Check all needed chunks exist
        for chunk_idx in start_chunk..=end_chunk {
            if !self.chunks.contains_key(&chunk_idx) {
                return None;
            }
        }

        // Assemble output from chunks
        let mut result = Vec::with_capacity(len);
        let mut remaining = len;
        let mut pos = output_offset;

        while remaining > 0 {
            let chunk_idx = pos / CHUNK_SIZE;
            let offset_in_chunk = pos % CHUNK_SIZE;
            let chunk = &self.chunks[&chunk_idx];
            let available = chunk.len().saturating_sub(offset_in_chunk);
            let take = remaining.min(available);
            if take == 0 {
                return None;
            }
            result.extend_from_slice(&chunk[offset_in_chunk..offset_in_chunk + take]);
            pos += take;
            remaining -= take;
        }

        Some(result)
    }

    /// Store a chunk of output data. `chunk_offset` must be chunk-aligned.
    pub fn put(&mut self, chunk_offset: usize, data: Vec<u8>) {
        let chunk_idx = chunk_offset / CHUNK_SIZE;
        let data_len = data.len();

        if let Some(old) = self.chunks.insert(chunk_idx, data) {
            self.total_bytes -= old.len();
        }
        self.total_bytes += data_len;
    }

    /// Evict chunks farthest from `viewport_center` until under budget.
    pub fn evict(&mut self, viewport_center: usize) {
        let center_chunk = viewport_center / CHUNK_SIZE;

        while self.total_bytes > MAX_CACHE_BYTES && !self.chunks.is_empty() {
            // Find the chunk farthest from viewport
            let farthest = self
                .chunks
                .keys()
                .max_by_key(|&&idx| idx.abs_diff(center_chunk))
                .copied();

            if let Some(idx) = farthest {
                if let Some(removed) = self.chunks.remove(&idx) {
                    self.total_bytes -= removed.len();
                }
            } else {
                break;
            }
        }
    }

    /// Clear all cached data.
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.total_bytes = 0;
    }
}
